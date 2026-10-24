//! Blog: creazione, dettaglio, modifica, collaboratori, inviti, ciclo di vita
//! (pausa, cancellazione con tolleranza, trasferimento) e configurazione di presentazione.
//!
//! I campi che appartengono a moduli opzionali arrivano dalle estensioni
//! (`noct_core::EntityExtension`); copertina, favicon e libreria media sono del crate `media`.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

mod handlers_impl;
pub mod rules;
pub mod view;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}

/// Giorni in cui un blog cancellato resta ripristinabile prima della cancellazione definitiva.
pub const BLOG_DELETE_GRACE_DAYS: i64 = 30;
/// Ruoli assegnabili da questa interfaccia (autore e revisore solo per altre vie).
pub const INVITABLE_ROLES: &[&str] = &["co_autore", "mediatore"];
