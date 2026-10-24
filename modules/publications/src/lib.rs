//! Pubblicazioni: raccolte di post come capitoli. `publication_id` in ingresso e
//! `publication`/`chapter_order` in uscita dei post arrivano dall'estensione (`extensions.rs`).

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

pub mod extensions;
mod handlers_impl;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}
