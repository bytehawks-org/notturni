//! Commenti ai post. I campi di modalità (`comments_mode`, `effective_comments_mode`,
//! `comments_auto_close_days`) di blog e post arrivano dalle estensioni (`extensions.rs`).

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

pub mod extensions;
mod handlers_impl;
pub mod personal_data;
pub mod rules;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}

pub mod admin_sections;
