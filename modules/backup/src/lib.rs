//! Backup. Due cose distinte: la copia del Markdown di ogni post pubblicato (veloce, nello stesso
//! storage, un fallback) e il backup infrastrutturale dell'intero database e di tutti i bucket verso
//! uno storage S3 **esterno**, che in produzione deve poter essere un altro provider.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

pub mod events;
pub mod infra;
pub mod jobs;
