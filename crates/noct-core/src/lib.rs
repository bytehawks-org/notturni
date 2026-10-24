//! Runtime condiviso: errori HTTP, configurazione, stato dell'applicazione,
//! autorizzazione, rate limit e registro dei moduli. Il codice generato da
//! `noct-codegen` per ogni modulo si appoggia solo a questo crate.

mod authz;
mod config;
mod db;
mod error;
mod patch;
mod rate_limit;
mod registry;
mod route;
mod state;

pub use authz::{Access, Actor, BlogRef, Identity, authenticate, can_view_blog, can_write_blog, enforce};
pub use config::Config;
pub use db::map_db_error;
pub use error::ApiError;
pub use patch::Patch;
pub use rate_limit::RateLimiter;
pub use registry::{ModuleDef, Registry};
pub use route::{
    Policy, RateKey, RateSpec, RouteCtx, RouteSpec, json_response, no_content, parse_json, parse_uuid, path_map,
};
pub use state::AppState;

/// Ri-esportati per il codice generato.
pub use noct_migrate::Migration;
