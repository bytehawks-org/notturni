//! Handler `custom` del modulo.

use crate::{DomainRequest, dns, rules};
use noct_audit::{Entry, user_actor};
use noct_core::{ApiError, RouteCtx, map_db_error, security::token::random_secret};
use noct_manifest::ConfigValue;
use serde_json::{Value, json};
use sqlx::PgConnection;
use std::time::Duration;
use uuid::Uuid;

const MSG_TAKEN: &str = "Dominio già rivendicato da un altro account.";

fn user_id(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    ctx.actor.as_ref().map(|a| a.user_id).ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))
}

fn dns_server(ctx: &RouteCtx) -> String {
    match ctx.state.module_config.get("custom_domains").and_then(|c| c.get("dns_server")) {
        Some(ConfigValue::String(s)) => s.clone(),
        _ => String::new(),
    }
}

fn dns_timeout(ctx: &RouteCtx) -> Duration {
    match ctx.state.module_config.get("custom_domains").and_then(|c| c.get("dns_timeout_ms")) {
        Some(ConfigValue::Int(ms)) if *ms > 0 => Duration::from_millis(*ms as u64),
        _ => Duration::from_secs(5),
    }
}

fn domain_out(domain: &str, status: &str, token: &str) -> Value {
    json!({
        "domain": domain, "status": status,
        "txt_record_name": rules::txt_record_name(domain), "txt_record_value": rules::txt_record_value(token),
    })
}

async fn audit(
    conn: &mut PgConnection,
    ctx: &RouteCtx,
    action: &str,
    user: Uuid,
    payload: Value,
) -> Result<(), ApiError> {
    let actor = user_actor(&mut *conn, user).await.map_err(ApiError::internal)?;
    noct_audit::record(
        conn,
        Entry { action, actor, target: Some(("user", user)), blog_id: None, meta: &ctx.meta, payload },
    )
    .await
    .map_err(ApiError::internal)
}

/// Registra (o sostituisce) il dominio in attesa di verifica.
pub async fn set_domain(ctx: RouteCtx, body: DomainRequest) -> Result<Value, ApiError> {
    let me = user_id(&ctx)?;
    let domain =
        rules::validate_domain(&body.domain, &ctx.state.config.instance_fqdn).map_err(ApiError::bad_request)?;
    let token = random_secret(16);
    let mut tx = ctx.state.db.begin().await?;
    // solo un dominio già verificato da un altro account blocca: una richiesta non verificata non
    // prova alcun controllo sul dominio, altrimenti chiunque potrebbe «prenotarlo» all'infinito
    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM custom_domains WHERE domain = $1 AND user_id <> $2 AND status = 'verified')",
    )
    .bind(&domain)
    .bind(me)
    .fetch_one(&mut *tx)
    .await?;
    if taken {
        return Err(ApiError::conflict(MSG_TAKEN));
    }
    // le richieste non verificate di altri su questo dominio si riconquistano, non si ereditano
    sqlx::query("DELETE FROM custom_domains WHERE domain = $1 AND user_id <> $2 AND status <> 'verified'")
        .bind(&domain)
        .bind(me)
        .execute(&mut *tx)
        .await?;
    // sostituire un dominio verificato con uno nuovo non deve lasciare il profilo con il vecchio
    sqlx::query(
        "UPDATE users SET verified_domain = NULL, updated_at = now() WHERE id = $1 AND verified_domain IS NOT NULL",
    )
    .bind(me)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO custom_domains (user_id, domain, verification_token) VALUES ($1, $2, $3)
         ON CONFLICT ON CONSTRAINT uq_custom_domains_user DO UPDATE SET domain = EXCLUDED.domain,
            verification_token = EXCLUDED.verification_token, status = 'pending', verified_at = NULL, updated_at = now()",
    )
    .bind(me)
    .bind(&domain)
    .bind(&token)
    .execute(&mut *tx)
    .await
    // due utenti che rivendicano in parallelo lo stesso dominio: il secondo riceve un 409 pulito
    .map_err(|e| map_db_error(e, &[("uq_custom_domains_domain", 409, MSG_TAKEN)]))?;
    tx.commit().await?;
    Ok(domain_out(&domain, "pending", &token))
}

/// Interroga il DNS: se il record TXT combacia il dominio è verificato.
pub async fn verify_domain(ctx: RouteCtx) -> Result<Value, ApiError> {
    let me = user_id(&ctx)?;
    let row: Option<(String, String, String)> =
        sqlx::query_as("SELECT domain, verification_token, status::text FROM custom_domains WHERE user_id = $1")
            .bind(me)
            .fetch_optional(&ctx.state.db)
            .await?;
    let (domain, token, status) = row.ok_or_else(|| ApiError::not_found("Nessun dominio impostato."))?;
    if status == "verified" {
        return Ok(domain_out(&domain, &status, &token));
    }
    let found = dns::txt_matches(
        &dns_server(&ctx),
        dns_timeout(&ctx),
        &rules::txt_record_name(&domain),
        &rules::txt_record_value(&token),
    )
    .await;
    let mut tx = ctx.state.db.begin().await?;
    if !found {
        sqlx::query("UPDATE custom_domains SET status = 'failed', updated_at = now() WHERE user_id = $1 AND domain = $2 AND status <> 'verified'")
            .bind(me)
            .bind(&domain)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Err(ApiError::bad_request(
            "Record TXT non trovato o non corrispondente. Riprova dopo aver aggiornato il DNS.",
        ));
    }
    // un altro account può aver verificato lo stesso dominio nel frattempo: vince il primo
    let done = sqlx::query(
        "UPDATE custom_domains SET status = 'verified', verified_at = now(), updated_at = now()
         WHERE user_id = $1 AND domain = $2 AND NOT EXISTS (SELECT 1 FROM custom_domains o WHERE o.domain = $2 AND o.user_id <> $1 AND o.status = 'verified')",
    )
    .bind(me)
    .bind(&domain)
    .execute(&mut *tx)
    .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::conflict(MSG_TAKEN));
    }
    sqlx::query("UPDATE users SET verified_domain = $2, updated_at = now() WHERE id = $1")
        .bind(me)
        .bind(&domain)
        .execute(&mut *tx)
        .await
        .map_err(|e| map_db_error(e, &[]))?;
    audit(&mut tx, &ctx, "user.domain_verified", me, json!({"domain": domain})).await?;
    tx.commit().await?;
    Ok(domain_out(&domain, "verified", &token))
}

pub async fn delete_domain(ctx: RouteCtx) -> Result<(), ApiError> {
    let me = user_id(&ctx)?;
    let mut tx = ctx.state.db.begin().await?;
    let removed: Option<String> =
        sqlx::query_scalar("DELETE FROM custom_domains WHERE user_id = $1 RETURNING status::text")
            .bind(me)
            .fetch_optional(&mut *tx)
            .await?;
    if removed.as_deref() == Some("verified") {
        sqlx::query("UPDATE users SET verified_domain = NULL, updated_at = now() WHERE id = $1")
            .bind(me)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}
