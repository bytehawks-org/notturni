//! Dominio personale dell'utente, verificato con un record TXT sul DNS (come su Bluesky).
//! Verificato dà il sigillo «bronzo» (calcolato dal profilo) e rende il profilo raggiungibile
//! anche dal dominio; lo username resta sempre risolvibile.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

pub mod dns;
pub mod extensions;
mod handlers_impl;
pub mod personal_data;
pub mod rules;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}
