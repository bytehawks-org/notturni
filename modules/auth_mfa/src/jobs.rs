//! Job del modulo (`[jobs.*]` in module.toml).

use crate::PURPOSE_MFA_EMAIL;
use noct_core::{
    AppState,
    bus::{HandlerError, Message},
    mail::Email,
    security::otp::derive_otp,
};
use sqlx::Row;
use uuid::Uuid;

/// Spedisce il codice MFA. Il messaggio porta solo l'id del codice: il valore si rigenera qui.
pub async fn send_mfa_email(st: AppState, msg: Message) -> Result<(), HandlerError> {
    let code_id = msg
        .payload
        .get("code_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| HandlerError::Permanent("payload senza `code_id` valido".into()))?;
    let row = sqlx::query(
        "SELECT u.email, u.is_active, (c.consumed_at IS NOT NULL OR c.expires_at <= now()) AS spent
         FROM mfa_email_codes c JOIN users u ON u.id = c.user_id WHERE c.id = $1",
    )
    .bind(code_id)
    .fetch_optional(&st.db)
    .await
    .map_err(|e| HandlerError::Retry(e.to_string()))?;
    let Some(row) = row else { return Ok(()) };
    if row.get::<bool, _>("spent") || !row.get::<bool, _>("is_active") {
        return Ok(());
    }
    let code = derive_otp(st.config.jwt_secret.as_bytes(), PURPOSE_MFA_EMAIL, code_id);
    let body = format!(
        "Il tuo codice di verifica per Notturni è: {code}\n\nVale 10 minuti. Se non sei stato tu, \
         cambia subito la password: qualcuno conosce quella attuale.\n"
    );
    st.services
        .mailer
        .send(&Email::text(row.get::<String, _>("email"), "Notturni — codice di verifica", body))
        .await
        .map_err(|e| HandlerError::Retry(e.to_string()))
}
