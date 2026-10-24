//! Modulo di riferimento: esercita tutte le funzioni del sistema dichiarativo
//! (CRUD generato, PATCH tri-stato, enum, rate limit, configurazione, handler
//! `custom`, eventi via outbox, job). Serve da modello per i nuovi moduli e da
//! banco di prova per il codegen; non fa parte delle funzioni di prodotto.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

pub mod events;
pub mod handlers;
pub mod jobs;
