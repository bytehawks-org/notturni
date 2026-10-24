//! Handler `custom` del modulo.

use noct_core::{ApiError, RouteCtx};
use serde_json::Value;

/// Sorgente di verità dell'elenco: `interests_settings.items`. Senza riga (installazione nuova)
/// l'elenco è vuoto.
pub async fn list_interests(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let items: Option<Value> =
        sqlx::query_scalar("SELECT items FROM interests_settings WHERE id = 1").fetch_optional(&ctx.state.db).await?;
    Ok(match items {
        Some(Value::Array(list)) => list,
        _ => Vec::new(),
    })
}
