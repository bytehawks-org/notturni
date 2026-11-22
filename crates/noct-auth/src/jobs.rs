//! Job del modulo (`[jobs.*]` in module.toml).

use noct_core::{
    AppState,
    bus::{HandlerError, Message},
    mail::Email,
    security::otp::derive_otp,
};
use sqlx::Row;
use uuid::Uuid;

pub const PURPOSE_PASSWORD_RESET: &str = "password_reset";

/// Spedisce il codice di reset. Il messaggio porta solo l'id del codice: il valore
/// si rigenera qui, quindi non passa mai da outbox, bus o dead letter.
pub async fn send_password_reset(st: AppState, msg: Message) -> Result<(), HandlerError> {
    let code_id = msg
        .payload
        .get("code_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| HandlerError::Permanent("payload senza `code_id` valido".into()))?;

    let row = sqlx::query(
        "SELECT u.email, u.is_active, (c.consumed_at IS NOT NULL OR c.expires_at <= now()) AS spent
         FROM password_reset_codes c JOIN users u ON u.id = c.user_id WHERE c.id = $1",
    )
    .bind(code_id)
    .fetch_optional(&st.db)
    .await
    .map_err(|e| HandlerError::Retry(e.to_string()))?;
    // codice già usato, scaduto o utente sparito: niente da spedire, e riprovare non serve
    let Some(row) = row else { return Ok(()) };
    if row.get::<bool, _>("spent") || !row.get::<bool, _>("is_active") {
        return Ok(());
    }

    let code = derive_otp(st.config.jwt_secret.as_bytes(), PURPOSE_PASSWORD_RESET, code_id);
    let body = format!(
        "Hai chiesto di reimpostare la password di Notturni.\n\nIl tuo codice è: {code}\n\n\
         Vale 10 minuti. Se non sei stato tu, ignora questa email: la tua password non cambia.\n"
    );
    st.services
        .mailer
        .send(&Email::text(row.get::<String, _>("email"), "Notturni — codice per reimpostare la password", body))
        .await
        .map_err(|e| HandlerError::Retry(e.to_string()))
}
