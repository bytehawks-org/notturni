//! Newsletter: iscrizione con doppio consenso a un blog o al digest di piattaforma, disiscrizione
//! e cancellazione senza login, notifica automatica dei nuovi post e campagne manuali.
//!
//! Nessun segreto passa da outbox o bus: i link di conferma e di disiscrizione si derivano con la
//! chiave del server (`tokens`) nel job che spedisce l'email.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

pub mod content;
mod events;
mod handlers_impl;
pub mod jobs;
pub mod personal_data;
pub mod tokens;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}

/// Validità del link di conferma.
pub const CONFIRM_TTL_HOURS: i64 = 48;

/// Chiave della lista: `blog:<id>` oppure `platform` (digest).
pub fn list_key(blog: Option<uuid::Uuid>) -> String {
    match blog {
        Some(id) => format!("blog:{id}"),
        None => "platform".to_string(),
    }
}
