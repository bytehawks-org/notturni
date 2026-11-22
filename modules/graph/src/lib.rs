//! Mappa dei collegamenti: grafo di post, Spazi, tag, categorie e link tra post. Nessuna tabella propria,
//! tutto calcolato da Postgres alla richiesta (vedi `rules::build` per la parte pura).

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

mod handlers_impl;
pub mod rules;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}
