//! Identità e SEO del portale: icona e favicon, immagine per le condivisioni, meta tag per lingua e verifiche dei
//! motori di ricerca. La validazione sta in `rules`, la generazione delle icone in `icons`.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

mod handlers_impl;
pub mod icons;
pub mod rules;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}
