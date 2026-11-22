//! Job del modulo (`[jobs.*]` in module.toml).

use crate::{PURPOSE_EMAIL_NEW, PURPOSE_EMAIL_OLD};
use noct_core::{
    AppState,
    bus::{HandlerError, Message},
    mail::Email,
    security::otp::derive_otp,
};
use sqlx::Row;
use uuid::Uuid;

/// Spedisce il codice di un passo del cambio email. Il messaggio porta solo l'id della richiesta
/// e il passo (`old` | `new`): il valore si rigenera qui.
pub async fn send_email_change_code(st: AppState, msg: Message) -> Result<(), HandlerError> {
    let request = msg
        .payload
        .get("request_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| HandlerError::Permanent("payload senza `request_id` valido".into()))?;
    let stage = msg.payload.get("stage").and_then(|v| v.as_str()).unwrap_or_default();
    let (purpose, spent_sql) = match stage {
        "old" => (PURPOSE_EMAIL_OLD, "(r.old_consumed_at IS NOT NULL OR r.old_expires_at <= now())"),
        "new" => (
            PURPOSE_EMAIL_NEW,
            "(r.new_consumed_at IS NOT NULL OR r.new_expires_at IS NULL OR r.new_expires_at <= now())",
        ),
        other => return Err(HandlerError::Permanent(format!("passo sconosciuto: `{other}`"))),
    };
    let row = sqlx::query(&format!(
        "SELECT u.email, r.new_email, u.is_active, (r.completed_at IS NOT NULL OR {spent_sql}) AS spent
         FROM email_change_requests r JOIN users u ON u.id = r.user_id WHERE r.id = $1"
    ))
    .bind(request)
    .fetch_optional(&st.db)
    .await
    .map_err(|e| HandlerError::Retry(e.to_string()))?;
    // richiesta annullata, scaduta o già usata: niente da spedire
    let Some(row) = row else { return Ok(()) };
    if row.get::<bool, _>("spent") || !row.get::<bool, _>("is_active") {
        return Ok(());
    }
    let to: String = if stage == "old" { row.get("email") } else { row.get("new_email") };
    let code = derive_otp(st.config.jwt_secret.as_bytes(), purpose, request);
    let body = if stage == "old" {
        format!(
            "Hai chiesto di cambiare l'indirizzo email del tuo account Notturni.\n\nCodice di conferma: {code}\n\n\
             Vale 10 minuti. Se non sei stato tu, ignora questo messaggio e cambia la password.\n"
        )
    } else {
        format!(
            "Conferma il nuovo indirizzo email del tuo account Notturni.\n\nCodice di conferma: {code}\n\n\
             Vale 10 minuti. Se non hai chiesto tu il cambio, ignora questo messaggio.\n"
        )
    };
    st.services
        .mailer
        .send(&Email::text(to, "Notturni — conferma il cambio email", body))
        .await
        .map_err(|e| HandlerError::Retry(e.to_string()))
}
