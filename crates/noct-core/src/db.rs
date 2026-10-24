use crate::ApiError;

/// Traduce un errore sqlx in risposta HTTP. `conflicts` mappa il nome di un
/// vincolo di unicità violato su `(status, messaggio)` dichiarati nel manifest.
pub fn map_db_error(err: sqlx::Error, conflicts: &[(&str, u16, &str)]) -> ApiError {
    if let sqlx::Error::Database(db) = &err {
        match db.code().as_deref() {
            Some("23505") => {
                let constraint = db.constraint().unwrap_or_default();
                return match conflicts.iter().find(|(c, _, _)| *c == constraint) {
                    Some((_, status, message)) => ApiError::new(*status, *message),
                    None => ApiError::conflict("Esiste già una risorsa con questi dati."),
                };
            }
            Some("23503") => return ApiError::bad_request("Riferimento a una risorsa inesistente o ancora in uso."),
            Some("22001") => return ApiError::bad_request("Un valore supera la lunghezza consentita."),
            _ => {}
        }
    }
    ApiError::internal(err)
}
