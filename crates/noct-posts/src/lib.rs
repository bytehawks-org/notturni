//! Post: bozze, revisione, pubblicazione e pianificazione, traduzioni, tag, media e link
//! citati, permalink. Campi e logica che appartengono a moduli opzionali (categoria,
//! pubblicazione, note, commenti) arrivano dalle estensioni (`noct_core::EntityExtension`);
//! backup su storage e notifica alla newsletter sono consumer degli eventi pubblicati qui.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

mod handlers_impl;
mod out;
pub mod personal_data;
pub mod rules;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}

pub use out::{PostRow, VISIBLE_SQL, render};
