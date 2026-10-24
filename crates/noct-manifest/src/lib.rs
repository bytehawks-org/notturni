//! Parsing, normalizzazione e validazione dei manifest dichiarativi di un
//! modulo (`module.toml`, `api.toml`, `db.toml`).
//!
//! Il crate è puro (nessun I/O, nessun database): lo usano sia `build.rs` dei
//! moduli (via `noct-codegen`) sia il runtime e `noct-ctl`.

mod api;
mod config;
mod db;
mod graph;
mod issue;
mod manifest;
mod module;
mod openapi;
mod registry;

pub use api::{
    ApiFile, Auth, Conflict, Field, FieldType, FieldTypeName, Key, Method, RateKey, RateLimit, Route, RouteKind, Scope,
    parse_duration, type_ref,
};
pub use config::{ConfigValue, resolve_config};
pub use db::{ColType, Column, EnumDef, Index, OnDelete, Reference, Schema, Table, Unique};
pub use graph::topological_order;
pub use issue::{Issue, Issues};
pub use manifest::{Manifest, Sources};
pub use module::{ConfigField, ConfigType, Events, JobSpec, Kind, Lifecycle, ModuleFile, ModuleMeta};
pub use openapi::openapi;
pub use registry::validate_registry;

/// Identificatore SQL/modulo ammesso: minuscole, cifre, `_`.
pub(crate) fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some('a'..='z')) && s.len() <= 63 && chars.all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_'))
}
