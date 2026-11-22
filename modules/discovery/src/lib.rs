//! Scoperta: feed e ricerca del portale, directory dei blog, follow, panoramica, letture, menzioni.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

mod handlers_impl;
pub mod personal_data;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}
