//! Runtime condiviso: errori HTTP, configurazione, stato dell'applicazione,
//! autorizzazione, rate limit e registro dei moduli. Il codice generato da
//! `noct-codegen` per ogni modulo si appoggia solo a questo crate.

mod authz;
mod config;
mod db;
mod error;
mod extension;
mod hooks;
mod patch;
mod rate_limit;
mod registry;
pub mod revalidate;
mod route;
mod runtime;
mod services;
mod state;

pub use authz::{
    Access, Actor, BlogRef, Identity, PEER_IP_HEADER, RequestMeta, TokenOwner, TokenPrincipal, authenticate,
    can_view_blog, can_write_blog, client_ip, enforce, load_blog, membership_role, request_meta,
};
pub use config::{Config, StorageBackend, StorageConfig, is_placeholder_secret};
pub use db::map_db_error;
pub use error::ApiError;
pub use extension::{Entity, EntityExtension, ExtCtx, Extensions, JsonMap, Op};
pub mod catalog;
pub use catalog::ModuleInfo;
pub mod schedule;
pub use hooks::{
    AdminSection, AdminSections, LoginGate, LoginGates, PersonalData, PersonalDataProviders, StoredObject,
};
pub use patch::Patch;
pub use rate_limit::RateLimiter;
pub use registry::{ModuleDef, Registry};
pub use route::{
    Policy, RateKey, RateSpec, RouteCtx, RouteSpec, body_map, empty_response, json_response, no_content, parse_json,
    parse_uuid, path_map,
};
pub use runtime::{
    Background, ConsumerDef, HandlerFn, HandlerFuture, StartupFuture, consumer, emit_event, event_envelope,
};
pub use schedule::Schedule;
pub use services::Services;
pub use state::AppState;

/// Ri-esportati per il codice generato.
pub use noct_migrate::Migration;

/// Le librerie di infrastruttura, raggiungibili da un solo crate.
pub use noct_bus as bus;
pub use noct_cache as cache;
pub use noct_http as http;
pub use noct_mail as mail;
pub use noct_markdown as markdown;
pub use noct_security as security;
pub use noct_storage as storage;
