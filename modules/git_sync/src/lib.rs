//! Sincronizzazione Git: esporta i testi di uno Spazio in un branch di un repository remoto.
//!
//! * `rules` — validazione di URL e branch, generazione dei file (Markdown con front matter);
//! * `git` — esecuzione del binario `git` con ambiente isolato (HTTPS con token o SSH con chiave pinnata);
//! * `ssh` — chiave di deploy e host key tramite OpenSSH;
//! * `sync` — un giro completo: raccolta dei contenuti, clone del branch, commit, push;
//! * `handlers_impl` / `jobs` — API di configurazione e consumer dei job.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

mod git;
mod handlers_impl;
pub mod jobs;
pub mod personal_data;
pub mod rules;
mod scratch;
mod ssh;
pub mod sync;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}

/// Soggetto del job che esegue una sincronizzazione.
pub const SUBJECT_RUN: &str = "git_sync.run";
