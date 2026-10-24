use crate::{Actor, ApiError, AppState, BlogRef, RequestMeta, TokenPrincipal};
use axum::{
    Json,
    body::Bytes,
    extract::RawPathParams,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use noct_manifest::ConfigValue;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use uuid::Uuid;

/// Policy di accesso di una rotta (vedi `auth` in api.toml).
#[derive(Debug, Clone, Copy)]
pub enum Policy {
    Public,
    /// Sessione utente (JWT).
    User,
    /// API token (core o utente).
    Token,
    /// Sessione utente oppure API token.
    UserOrToken,
    BlogWrite,
    PlatformRole(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateKey {
    Ip,
    User,
}

#[derive(Debug, Clone, Copy)]
pub struct RateSpec {
    pub key: RateKey,
    pub max: u32,
    pub window_secs: u64,
}

/// Metadati statici di una rotta, emessi dal codegen.
#[derive(Debug, Clone, Copy)]
pub struct RouteSpec {
    pub module: &'static str,
    pub id: &'static str,
    pub policy: Policy,
    pub rate: Option<RateSpec>,
}

/// Contesto passato agli handler `custom` dopo autenticazione, autorizzazione
/// e rate limit già applicati.
pub struct RouteCtx {
    pub state: AppState,
    /// Utente con sessione valida (JWT).
    pub actor: Option<Actor>,
    /// API token valido.
    pub token: Option<TokenPrincipal>,
    /// Presente se la rotta dichiara `scope = { blog = ... }`.
    pub blog: Option<BlogRef>,
    pub path: HashMap<String, String>,
    pub query: HashMap<String, String>,
    pub headers: HeaderMap,
    /// Corpo grezzo della richiesta: presente per le rotte con `request` (già
    /// interpretato in `body`, qui serve a leggere i campi delle estensioni) e per
    /// quelle con `upload`; vuoto altrimenti.
    pub body: Bytes,
    pub meta: RequestMeta,
    pub module: &'static str,
}

impl RouteCtx {
    fn config_value(&self, key: &str) -> Option<&ConfigValue> {
        self.state.module_config.get(self.module)?.get(key)
    }

    pub fn config_str(&self, key: &str) -> Option<&str> {
        match self.config_value(key)? {
            ConfigValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn config_int(&self, key: &str) -> Option<i64> {
        match self.config_value(key)? {
            ConfigValue::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn config_bool(&self, key: &str) -> Option<bool> {
        match self.config_value(key)? {
            ConfigValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

/// Il corpo JSON come oggetto (vuoto se il corpo manca o non è un oggetto): serve a leggere i
/// campi delle estensioni, che il DTO tipizzato della rotta non conosce.
pub fn body_map(body: &Bytes) -> crate::JsonMap {
    match serde_json::from_slice(body) {
        Ok(serde_json::Value::Object(m)) => m,
        _ => crate::JsonMap::new(),
    }
}

pub fn path_map(raw: &RawPathParams) -> HashMap<String, String> {
    raw.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

pub fn parse_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ApiError> {
    // un corpo assente equivale a `{}`: i campi obbligatori mancanti falliscono comunque la validazione
    let bytes: &[u8] = if bytes.iter().all(u8::is_ascii_whitespace) { b"{}" } else { bytes };
    serde_json::from_slice(bytes).map_err(|e| ApiError::unprocessable(format!("Corpo della richiesta non valido: {e}")))
}

pub fn parse_uuid(raw: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw).map_err(|_| ApiError::unprocessable("Identificativo non valido."))
}

pub fn json_response<T: Serialize>(status: u16, value: &T) -> Response {
    (StatusCode::from_u16(status).unwrap_or(StatusCode::OK), Json(value)).into_response()
}

pub fn no_content() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

/// Risposta senza corpo con lo status indicato (es. 202 Accepted).
pub fn empty_response(status: u16) -> Response {
    StatusCode::from_u16(status).unwrap_or(StatusCode::NO_CONTENT).into_response()
}
