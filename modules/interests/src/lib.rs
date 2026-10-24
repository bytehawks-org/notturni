//! Interessi di piattaforma. La scelta degli interessi da parte dell'utente appartiene al profilo.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

mod handlers_impl;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}
