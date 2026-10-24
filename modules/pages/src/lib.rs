//! Pagine statiche del sito e dei blog. L'attivazione per blog (`static_pages_enabled`) è un
//! campo del blog aggiunto dall'estensione (`extensions.rs`).

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

pub mod extensions;
mod handlers_impl;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}
