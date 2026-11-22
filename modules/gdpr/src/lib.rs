//! Diritti GDPR. L'esportazione e la cancellazione dei dati personali non conoscono i moduli: ogni
//! modulo attivo li espone con [`noct_core::PersonalData`] e questo modulo li raccoglie.
//!
//! **Cancellazione = anonimizzazione**, non `DELETE` della riga utente. Un utente può essere
//! proprietario di blog con altri collaboratori o aver scritto su blog altrui: cancellare la riga
//! costringerebbe a bloccare l'operazione o a cancellare a cascata contenuti di terzi. L'art. 17.3
//! consente di conservare ciò che serve a tutelare i diritti di altri: i dati personali spariscono,
//! blog, post e commenti restano con l'autore «Utente eliminato».

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

mod handlers_impl;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}

pub use handlers_impl::{anonymize_account, build_export};

/// Art. 12: risposta entro un mese dalla ricezione.
pub const DEADLINE_DAYS: i64 = 30;
pub const ANONYMIZED_NAME: &str = "Utente eliminato";
