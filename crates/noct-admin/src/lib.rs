//! Amministrazione di piattaforma. Il crate non conosce nessun modulo opzionale: contatori e
//! impostazioni che appartengono a un modulo (commenti anonimi, provider SSO, interessi…) li
//! dichiara il modulo stesso con [`noct_core::AdminSection`].

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

mod audit_log;
mod blogs;
mod config;
mod handlers_impl;
mod modules;
mod overview;
mod site_texts;
mod users;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}

/// Ruoli che amministrano la piattaforma.
pub const ADMIN_ROLES: &[&str] = &["super_admin", "amministratore"];
