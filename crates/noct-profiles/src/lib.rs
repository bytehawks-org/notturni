//! Profilo utente: anagrafica pubblica e privata, avatar, link social, cambio email in tre passi,
//! directory degli utenti e blog/post pubblici di un utente. Il sigillo di verifica si calcola
//! in lettura (`rules::verification_tier`), non è una colonna da tenere allineata.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

mod handlers_impl;
pub mod jobs;
pub mod personal_data;
pub mod rules;
mod view;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}

/// Scopi per derivare i codici del cambio email (vedi `noct_core::security::otp`): uno per passo,
/// così il codice della vecchia casella non vale per la nuova.
pub const PURPOSE_EMAIL_OLD: &str = "email_change_old";
pub const PURPOSE_EMAIL_NEW: &str = "email_change_new";
/// Validità di ciascun codice.
pub const EMAIL_CODE_TTL_MINUTES: i64 = 10;

pub mod admin_sections;
