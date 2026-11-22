//! Spazi wiki e documentazione: albero delle pagine, link interni con backlink e cronologia delle
//! revisioni. I campi `parent_id`, `position` e `revision_summary` dei post arrivano
//! dall'estensione (`extensions.rs`); la logica non-CRUD è in `handlers_impl.rs`, le regole pure
//! (link, differenze, validazione dell'albero) in `rules.rs`.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

pub mod extensions;
mod handlers_impl;
pub mod personal_data;
pub mod rules;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}
