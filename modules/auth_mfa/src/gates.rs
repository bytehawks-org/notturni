//! Gate del login: se l'utente ha l'MFA attiva, il login si ferma con una sfida.

use crate::{CHALLENGE_TTL_SECS, CHALLENGE_TYPE, EMAIL_CODE_TTL_MINUTES};
use async_trait::async_trait;
use chrono::Utc;
use noct_core::{
    ApiError, AppState, LoginGate,
    bus::{Kind, outbox},
    security::jwt,
};
use serde_json::{Value, json};
use sqlx::PgConnection;
use std::sync::Arc;
use uuid::Uuid;

pub fn all() -> Vec<Arc<dyn LoginGate>> {
    vec![Arc::new(MfaGate)]
}

/// Crea un codice via email per `user` e accoda il job che lo spedisce (stessa transazione:
/// se il bus è giù il messaggio resta nell'outbox). Vale solo l'ultimo codice richiesto.
pub async fn issue_email_code(conn: &mut PgConnection, user: Uuid) -> Result<Uuid, ApiError> {
    sqlx::query("UPDATE mfa_email_codes SET consumed_at = now() WHERE user_id = $1 AND consumed_at IS NULL")
        .bind(user)
        .execute(&mut *conn)
        .await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO mfa_email_codes (user_id, expires_at) VALUES ($1, now() + make_interval(mins => $2)) RETURNING id",
    )
    .bind(user)
    .bind(EMAIL_CODE_TTL_MINUTES)
    .fetch_one(&mut *conn)
    .await?;
    outbox::enqueue(conn, Kind::Job, "auth_mfa.email.send", &json!({ "code_id": id }))
        .await
        .map_err(ApiError::internal)?;
    Ok(id)
}

struct MfaGate;

#[async_trait]
impl LoginGate for MfaGate {
    fn module(&self) -> &'static str {
        "auth_mfa"
    }

    async fn challenge(&self, st: &AppState, user_id: Uuid, extra: Option<Value>) -> Result<Option<Value>, ApiError> {
        let method: Option<String> = sqlx::query_scalar(
            "SELECT s.method::text FROM mfa_settings s JOIN users u ON u.id = s.user_id
             WHERE s.user_id = $1 AND u.mfa_enabled AND s.method IS NOT NULL",
        )
        .bind(user_id)
        .fetch_optional(&st.db)
        .await?
        .flatten();
        let Some(method) = method else { return Ok(None) };

        if method == "email" {
            let mut tx = st.db.begin().await?;
            issue_email_code(&mut tx, user_id).await?;
            tx.commit().await?;
        }
        // il collegamento SSO in attesa viaggia nella sfida e si completa solo dopo la verifica
        let mut claims = serde_json::Map::new();
        claims.insert("method".into(), method.clone().into());
        if let Some(link) = extra {
            claims.insert("link".into(), link);
        }
        let challenge = jwt::encode_with(
            st.config.jwt_secret.as_bytes(),
            CHALLENGE_TYPE,
            user_id,
            Utc::now().timestamp(),
            CHALLENGE_TTL_SECS,
            claims,
        );
        Ok(Some(json!({ "mfa_required": true, "method": method, "challenge": challenge })))
    }
}
