//! Immagini dei blog: upload per l'editor con libreria media, copertina e favicon.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

mod handlers_impl;
pub mod upload;
pub mod variants;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}
