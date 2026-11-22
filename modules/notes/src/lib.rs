//! Note a piè di pagina dei post e libreria note del blog. Il campo `notes` di create, update e
//! traduzione dei post, e `notes` delle risposte, arrivano dall'estensione (`extensions.rs`).

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

pub mod extensions;
mod handlers_impl;
pub mod rules;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}

pub mod admin_sections;
