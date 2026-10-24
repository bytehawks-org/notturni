//! Hook di avvio: valida la configurazione e crea il Super Admin se richiesto.

use crate::{AuthSettings, util::hash_password};
use noct_audit::{AuditActor, Entry};
use noct_core::{AppState, RequestMeta};
use noct_manifest::ConfigValue;
use noct_security::validate::{normalize_email, validate_email, validate_password_policy, validate_username};
use serde_json::json;

pub async fn run(st: AppState) -> Result<(), String> {
    // una configurazione sbagliata deve fermare l'avvio, non manifestarsi alla prima richiesta
    let settings = AuthSettings::from_state(&st)?;
    probe_cookies(&settings)?;

    let cfg = st.module_config.get("auth").cloned().unwrap_or_default();
    let get = |k: &str| match cfg.get(k) {
        Some(ConfigValue::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    };
    if let (Some(username), Some(email), Some(password)) =
        (get("super_admin_username"), get("super_admin_email"), get("super_admin_password"))
    {
        // mai un errore fatale: un conflitto di provisioning (redeploy con un .env cambiato
        // dopo che l'account esiste o è stato rinominato a mano) non deve impedire l'avvio
        if let Err(e) = bootstrap_super_admin(&st, &username, &email, &password).await {
            tracing::error!(error = %e, "creazione del Super Admin non riuscita");
        }
    }
    Ok(())
}

/// Costruisce i cookie con la configurazione corrente: se dominio o attributi sono
/// invalidi lo si scopre qui.
fn probe_cookies(s: &AuthSettings) -> Result<(), String> {
    let mut res = axum::response::Response::new(axum::body::Body::empty());
    crate::util::set_session_cookies(&mut res, s, "noct_sess_probe")
        .map_err(|e| format!("configurazione dei cookie di sessione: {}", e.detail))
}

async fn bootstrap_super_admin(st: &AppState, username: &str, email: &str, password: &str) -> Result<(), String> {
    let email = normalize_email(email);
    validate_username(username)?;
    validate_email(&email)?;
    validate_password_policy(password)?;

    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE username = $1 OR email = $2)")
        .bind(username)
        .bind(&email)
        .fetch_one(&st.db)
        .await
        .map_err(|e| e.to_string())?;
    if exists {
        return Ok(());
    }
    let hash = hash_password(password.to_string()).await.map_err(|e| e.detail)?;
    let mut tx = st.db.begin().await.map_err(|e| e.to_string())?;
    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (username, email, hashed_password, platform_role) VALUES ($1, $2, $3, 'super_admin') ON CONFLICT DO NOTHING RETURNING id",
    )
    .bind(username)
    .bind(&email)
    .bind(&hash)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("creato nel frattempo da un altro processo")?;
    noct_audit::record(
        &mut tx,
        Entry {
            action: "auth.super_admin_bootstrap",
            actor: AuditActor::System,
            target: Some(("user", id)),
            blog_id: None,
            meta: &RequestMeta::default(),
            payload: json!({ "username": username }),
        },
    )
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    tracing::info!(%username, "Super Admin creato da NOCT_SUPER_ADMIN_*");
    Ok(())
}
