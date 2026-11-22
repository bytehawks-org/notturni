use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::json;

/// Errore restituito al client come `{"detail": "..."}` (stessa forma dell'API v1).
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub detail: String,
    pub retry_after: Option<u64>,
}

impl ApiError {
    pub fn new(status: u16, detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            detail: detail.into(),
            retry_after: None,
        }
    }
    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(400, detail)
    }
    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self::new(401, detail)
    }
    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::new(403, detail)
    }
    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(404, detail)
    }
    pub fn conflict(detail: impl Into<String>) -> Self {
        Self::new(409, detail)
    }
    pub fn unprocessable(detail: impl Into<String>) -> Self {
        Self::new(422, detail)
    }
    pub fn too_many_requests(retry_after: u64) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            detail: "Troppe richieste, riprova più tardi.".into(),
            retry_after: Some(retry_after),
        }
    }
    /// Il dettaglio reale va nei log, mai al client.
    pub fn internal(err: impl std::fmt::Display) -> Self {
        tracing::error!(error = %err, "errore interno");
        Self::new(500, "Errore interno del server.")
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        Self::internal(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut res = (self.status, Json(json!({ "detail": self.detail }))).into_response();
        if let Some(secs) = self.retry_after
            && let Ok(v) = HeaderValue::from_str(&secs.to_string())
        {
            res.headers_mut().insert(header::RETRY_AFTER, v);
        }
        res
    }
}
