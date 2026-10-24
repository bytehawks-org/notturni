//! Attivazione e disattivazione dei moduli opzionali (`/admin/modules`).
//!
//! Qui si scrive solo lo **stato desiderato** (`noct_modules`) e si incrementa la versione: ogni
//! replica, API o worker, la controlla a intervalli (e questa subito, tramite `modules_changed`),
//! applica le migrazioni mancanti, ricostruisce router e consumer e scambia i vecchi. Per questo
//! la risposta distingue `enabled` (voluto) da `active` (in esecuzione in questa replica).

use crate::handlers_impl::{Target, audit, path_uuid_free};
use noct_core::{ApiError, AppState, ModuleInfo, RouteCtx};
use serde_json::{Value, json};
use sqlx::PgConnection;
use std::collections::BTreeMap;

/// Stato desiderato di ogni modulo: i core sempre; gli altri dalla tabella, in mancanza di una riga
/// (installazione senza supervisore, test) da ciò che è attivo in questo processo.
async fn desired(conn: &mut PgConnection, st: &AppState) -> Result<BTreeMap<String, bool>, ApiError> {
    let rows: Vec<(String, bool)> = sqlx::query_as("SELECT name, enabled FROM noct_modules").fetch_all(conn).await?;
    let rows: BTreeMap<String, bool> = rows.into_iter().collect();
    Ok(st
        .catalog
        .iter()
        .map(|m| {
            let on = m.is_core() || rows.get(&m.name).copied().unwrap_or_else(|| st.module_enabled(&m.name));
            (m.name.clone(), on)
        })
        .collect())
}

fn item(st: &AppState, m: &ModuleInfo, want: &BTreeMap<String, bool>, error: Option<&str>) -> Value {
    let enabled = want.get(&m.name).copied().unwrap_or(false);
    let active = st.module_enabled(&m.name);
    let required_by: Vec<&str> = st
        .catalog
        .iter()
        .filter(|o| o.requires.contains(&m.name) && want.get(&o.name).copied().unwrap_or(false))
        .map(|o| o.name.as_str())
        .collect();
    json!({
        "name": m.name, "version": m.version, "kind": m.kind, "description": m.description,
        "requires": m.requires, "default_enabled": m.default_enabled, "enabled": enabled, "active": active,
        "pending": enabled != active, "error": if enabled != active { error } else { None },
        "required_by": required_by,
    })
}

async fn last_error(conn: &mut PgConnection) -> Result<Option<String>, ApiError> {
    Ok(sqlx::query_scalar("SELECT last_error FROM noct_module_state WHERE id = 1")
        .fetch_optional(conn)
        .await?
        .flatten())
}

pub async fn list_modules(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let mut conn = ctx.state.db.acquire().await?;
    let want = desired(&mut conn, &ctx.state).await?;
    let error = last_error(&mut conn).await?;
    Ok(ctx.state.catalog.iter().map(|m| item(&ctx.state, m, &want, error.as_deref())).collect())
}

async fn set_module(ctx: RouteCtx, enable: bool) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let name = path_uuid_free(&ctx, "name");
    let info = st.catalog.iter().find(|m| m.name == name).ok_or_else(|| ApiError::not_found("Modulo non trovato."))?;
    if info.is_core() {
        return Err(ApiError::bad_request("Un modulo core è sempre attivo e non si può disattivare."));
    }
    let mut tx = st.db.begin().await?;
    sqlx::query("INSERT INTO noct_module_state (id) VALUES (1) ON CONFLICT (id) DO NOTHING").execute(&mut *tx).await?;
    // serializza i cambi concorrenti: i controlli sulle dipendenze valgono per lo stato che si scrive
    sqlx::query("SELECT version FROM noct_module_state WHERE id = 1 FOR UPDATE").execute(&mut *tx).await?;
    let mut want = desired(&mut tx, st).await?;
    if want[&info.name] != enable {
        if enable {
            let missing: Vec<&str> =
                info.requires.iter().filter(|r| !want.get(*r).copied().unwrap_or(false)).map(String::as_str).collect();
            if !missing.is_empty() {
                return Err(ApiError::bad_request(format!(
                    "`{}` richiede i moduli {}: attivali prima.",
                    info.name,
                    missing.join(", ")
                )));
            }
        } else {
            let users: Vec<&str> = st
                .catalog
                .iter()
                .filter(|o| o.requires.contains(&info.name) && want.get(&o.name).copied().unwrap_or(false))
                .map(|o| o.name.as_str())
                .collect();
            if !users.is_empty() {
                return Err(ApiError::bad_request(format!(
                    "`{}` serve ai moduli attivi {}: disattivali prima.",
                    info.name,
                    users.join(", ")
                )));
            }
        }
        let admin = crate::handlers_impl::actor_id(&ctx)?;
        sqlx::query(
            "INSERT INTO noct_modules (name, enabled, updated_by_id) VALUES ($1, $2, $3)
             ON CONFLICT (name) DO UPDATE SET enabled = EXCLUDED.enabled, updated_at = now(), updated_by_id = EXCLUDED.updated_by_id",
        )
        .bind(&info.name)
        .bind(enable)
        .bind(admin)
        .execute(&mut *tx)
        .await?;
        // un cambio nuovo rende superato l'errore del precedente
        sqlx::query("UPDATE noct_module_state SET version = version + 1, last_error = NULL WHERE id = 1")
            .execute(&mut *tx)
            .await?;
        audit(
            &mut tx,
            &ctx,
            if enable { "module.enabled" } else { "module.disabled" },
            Some(Target { kind: "module", id: uuid::Uuid::nil(), blog: None }),
            json!({"module": info.name}),
        )
        .await?;
        want.insert(info.name.clone(), enable);
        tx.commit().await?;
        st.modules_changed.notify_one();
    } else {
        tx.rollback().await?;
    }
    let mut conn = st.db.acquire().await?;
    let error = last_error(&mut conn).await?;
    Ok(item(st, info, &want, error.as_deref()))
}

pub async fn enable_module(ctx: RouteCtx) -> Result<Value, ApiError> {
    set_module(ctx, true).await
}

pub async fn disable_module(ctx: RouteCtx) -> Result<Value, ApiError> {
    set_module(ctx, false).await
}
