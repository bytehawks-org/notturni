use axum::{Json, Router, extract::State, routing::get};
use noct_core::{ApiError, AppState, Registry};
use serde_json::{Value, json};
use std::sync::Arc;

/// Router completo: rotte di piattaforma (`/health`, `/openapi.json`) più quelle
/// dei moduli attivi, tutte sotto `/api/v1`.
pub fn app(registry: &Registry, enabled: &[usize], state: AppState) -> Router {
    let openapi = Arc::new(registry.openapi(enabled));
    let platform = Router::new()
        .route("/health", get(health))
        .route("/openapi.json", get(move || async move { Json(Value::clone(&openapi)) }));
    Router::new().nest("/api/v1", platform.merge(registry.router(enabled))).with_state(state)
}

async fn health(State(st): State<AppState>) -> Result<Json<Value>, ApiError> {
    sqlx::query("SELECT 1").execute(&st.db).await?;
    Ok(Json(json!({ "status": "ok" })))
}
