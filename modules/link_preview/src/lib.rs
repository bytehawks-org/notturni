//! Anteprima Open Graph di un URL esterno, con cache a due livelli (Redis + tabella).

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

pub mod fetch;
mod handlers_impl;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}
