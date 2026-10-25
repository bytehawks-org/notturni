//! Informativa privacy e cookie policy. I testi sono modelli (Markdown con variabili e condizioni sulle funzioni
//! attive) che si adattano all'installazione; l'amministratore completa i dati del titolare, li modifica per
//! lingua e li pubblica come pagine del sito (`/p/privacy`, `/p/cookie-policy`).

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

pub mod engine;
pub mod facts;
mod handlers_impl;
pub mod templates;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}
