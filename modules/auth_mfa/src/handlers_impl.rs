//! Handler `custom` del modulo.

use crate::{
    CHALLENGE_TYPE, MfaCode, MfaVerify, PURPOSE_MFA_EMAIL, TotpSetup,
    gates::issue_email_code,
    totp::{self, provisioning_uri},
};
use axum::response::Response;
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
use noct_audit::{AuditActor, Entry, user_actor};
use noct_auth::{AuthSettings, session};
use noct_core::{
    ApiError, AppState, RouteCtx,
    security::{constant_time_eq, jwt, otp::derive_otp},
};
use serde_json::{Value, json};
use sqlx::PgConnection;
use std::time::Duration;
use uuid::Uuid;

const MSG_BAD_CHALLENGE: &str = "Sfida MFA non valida o scaduta.";
const MSG_BAD_CODE: &str = "Codice MFA non valido o scaduto.";
const SUBJECT_LIMIT: u32 = 8;
const WINDOW: Duration = Duration::from_secs(300);
const ISSUER: &str = "Notturni";

fn user_id(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    ctx.actor.as_ref().map(|a| a.user_id).ok_or_else(|| ApiError::unauthorized("Not authenticated"))
}

/// Limite sui tentativi per account, oltre a quello per IP dichiarato sulla rotta: un codice a
/// 6 cifre è indovinabile in un numero gestibile di tentativi senza un tetto.
async fn subject_limit(st: &AppState, user: Uuid) -> Result<(), ApiError> {
    st.limiter.check(&format!("mfa:subject:{user}"), SUBJECT_LIMIT, WINDOW).await.map_err(ApiError::too_many_requests)
}

async fn audit(
    conn: &mut PgConnection,
    ctx: &RouteCtx,
    action: &str,
    user: Uuid,
    payload: Value,
) -> Result<(), ApiError> {
    let actor = user_actor(&mut *conn, user).await.map_err(ApiError::internal)?;
    noct_audit::record(conn, Entry { action, actor, target: None, blog_id: None, meta: &ctx.meta, payload })
        .await
        .map_err(ApiError::internal)
}

/// Verifica un codice via email contro l'ultimo non consumato; lo consuma se corretto.
async fn consume_email_code(conn: &mut PgConnection, st: &AppState, user: Uuid, code: &str) -> Result<bool, ApiError> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM mfa_email_codes WHERE user_id = $1 AND consumed_at IS NULL AND expires_at > now()
         ORDER BY created_at DESC, id LIMIT 1 FOR UPDATE",
    )
    .bind(user)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((id,)) = row else { return Ok(false) };
    let expected = derive_otp(st.config.jwt_secret.as_bytes(), PURPOSE_MFA_EMAIL, id);
    if !constant_time_eq(&expected, code.trim()) {
        return Ok(false);
    }
    let done = sqlx::query("UPDATE mfa_email_codes SET consumed_at = now() WHERE id = $1 AND consumed_at IS NULL")
        .bind(id)
        .execute(conn)
        .await?;
    Ok(done.rows_affected() == 1)
}

fn qr_data_uri(uri: &str) -> Result<String, ApiError> {
    let code = qrcode::QrCode::new(uri.as_bytes()).map_err(ApiError::internal)?;
    let svg = code.render::<qrcode::render::svg::Color>().min_dimensions(200, 200).quiet_zone(true).build();
    Ok(format!("data:image/svg+xml;base64,{}", STANDARD.encode(svg)))
}

// ---- configurazione ---------------------------------------------------------------------------------------------

pub async fn setup_totp(ctx: RouteCtx) -> Result<TotpSetup, ApiError> {
    let user = user_id(&ctx)?;
    let secret = totp::generate_secret().map_err(ApiError::internal)?;
    // il segreto resta "in attesa" finché non si conferma: rigenerarlo non rompe l'MFA già attiva
    sqlx::query(
        "INSERT INTO mfa_settings (user_id, pending_totp_secret) VALUES ($1, $2)
         ON CONFLICT (user_id) DO UPDATE SET pending_totp_secret = EXCLUDED.pending_totp_secret, updated_at = now()",
    )
    .bind(user)
    .bind(&secret)
    .execute(&ctx.state.db)
    .await?;
    let email: String =
        sqlx::query_scalar("SELECT email FROM users WHERE id = $1").bind(user).fetch_one(&ctx.state.db).await?;
    let uri = provisioning_uri(&secret, &email, ISSUER);
    Ok(TotpSetup { qr_code_data_uri: qr_data_uri(&uri)?, provisioning_uri: uri, secret })
}

pub async fn confirm_totp(ctx: RouteCtx, body: MfaCode) -> Result<(), ApiError> {
    let st = &ctx.state;
    let user = user_id(&ctx)?;
    subject_limit(st, user).await?;
    let pending: Option<String> = sqlx::query_scalar("SELECT pending_totp_secret FROM mfa_settings WHERE user_id = $1")
        .bind(user)
        .fetch_optional(&st.db)
        .await?
        .flatten();
    let step = pending
        .as_deref()
        .and_then(|s| totp::verify(s, &body.code, Utc::now().timestamp()))
        .ok_or_else(|| ApiError::bad_request("Codice non valido."))?;
    let mut tx = st.db.begin().await?;
    sqlx::query(
        "UPDATE mfa_settings SET method = 'totp', totp_secret = pending_totp_secret, pending_totp_secret = NULL,
                totp_last_step = $2, updated_at = now() WHERE user_id = $1",
    )
    .bind(user)
    .bind(step as i64)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE users SET mfa_enabled = true WHERE id = $1").bind(user).execute(&mut *tx).await?;
    audit(&mut tx, &ctx, "auth.mfa_enabled", user, json!({"method": "totp"})).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn setup_email(ctx: RouteCtx) -> Result<(), ApiError> {
    let user = user_id(&ctx)?;
    let mut tx = ctx.state.db.begin().await?;
    issue_email_code(&mut tx, user).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn confirm_email(ctx: RouteCtx, body: MfaCode) -> Result<(), ApiError> {
    let st = &ctx.state;
    let user = user_id(&ctx)?;
    subject_limit(st, user).await?;
    let mut tx = st.db.begin().await?;
    if !consume_email_code(&mut tx, st, user, &body.code).await? {
        return Err(ApiError::bad_request("Codice non valido o scaduto."));
    }
    sqlx::query(
        "INSERT INTO mfa_settings (user_id, method) VALUES ($1, 'email')
         ON CONFLICT (user_id) DO UPDATE SET method = 'email', totp_secret = NULL, pending_totp_secret = NULL,
                totp_last_step = NULL, updated_at = now()",
    )
    .bind(user)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE users SET mfa_enabled = true WHERE id = $1").bind(user).execute(&mut *tx).await?;
    audit(&mut tx, &ctx, "auth.mfa_enabled", user, json!({"method": "email"})).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn disable_mfa(ctx: RouteCtx) -> Result<(), ApiError> {
    let user = user_id(&ctx)?;
    let mut tx = ctx.state.db.begin().await?;
    sqlx::query("DELETE FROM mfa_settings WHERE user_id = $1").bind(user).execute(&mut *tx).await?;
    sqlx::query("UPDATE mfa_email_codes SET consumed_at = now() WHERE user_id = $1 AND consumed_at IS NULL")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE users SET mfa_enabled = false WHERE id = $1").bind(user).execute(&mut *tx).await?;
    audit(&mut tx, &ctx, "auth.mfa_disabled", user, json!({})).await?;
    tx.commit().await?;
    Ok(())
}

// ---- completamento del login --------------------------------------------------------------------------------------

pub async fn verify_mfa(ctx: RouteCtx, body: MfaVerify) -> Result<Response, ApiError> {
    let st = &ctx.state;
    let claims = jwt::decode(st.config.jwt_secret.as_bytes(), &body.challenge, CHALLENGE_TYPE, Utc::now().timestamp())
        .map_err(|_| ApiError::unauthorized(MSG_BAD_CHALLENGE))?;
    let user = claims.sub;
    subject_limit(st, user).await?;

    let account: Option<(String, String, bool)> =
        sqlx::query_as("SELECT username, email, is_active FROM users WHERE id = $1")
            .bind(user)
            .fetch_optional(&st.db)
            .await?;
    let Some((username, email, true)) = account else { return Err(ApiError::unauthorized("Utente non valido.")) };

    // il metodo è quello attuale dell'utente: una sfida emessa prima di un cambio non vale
    let (method, secret): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT method::text, totp_secret FROM mfa_settings WHERE user_id = $1")
            .bind(user)
            .fetch_optional(&st.db)
            .await?
            .unwrap_or((None, None));
    let claimed = claims.extra.get("method").and_then(Value::as_str);
    if method.as_deref() != claimed {
        return Err(ApiError::unauthorized(MSG_BAD_CHALLENGE));
    }

    let mut tx = st.db.begin().await?;
    let ok = match (method.as_deref(), secret.as_deref()) {
        (Some("totp"), Some(secret)) => match totp::verify(secret, &body.code, Utc::now().timestamp()) {
            // atomico: due richieste con lo stesso codice non passano entrambe
            Some(step) => {
                sqlx::query(
                    "UPDATE mfa_settings SET totp_last_step = $2 WHERE user_id = $1 AND (totp_last_step IS NULL OR totp_last_step < $2)",
                )
                .bind(user)
                .bind(step as i64)
                .execute(&mut *tx)
                .await?
                .rows_affected()
                    == 1
            }
            None => false,
        },
        (Some("email"), _) => consume_email_code(&mut tx, st, user, &body.code).await?,
        _ => false,
    };
    if !ok {
        return Err(ApiError::unauthorized(MSG_BAD_CODE));
    }

    // ciò che il login aspettava (es. il collegamento SSO) si completa solo ora
    if let Some(link) = claims.extra.get("link") {
        st.login_gates.completed(st, user, link).await?;
    }
    noct_audit::record(
        &mut tx,
        Entry {
            action: "auth.login",
            actor: AuditActor::user(user, &username, &email),
            target: None,
            blog_id: None,
            meta: &ctx.meta,
            payload: json!({ "method": format!("mfa_{}", method.as_deref().unwrap_or("?")) }),
        },
    )
    .await
    .map_err(ApiError::internal)?;
    let settings = AuthSettings::from_state(st).map_err(ApiError::internal)?;
    let (access, refresh) = session::issue_session(&mut tx, st, &settings, user).await?;
    tx.commit().await?;
    session::session_response(access, &refresh, &settings)
}
