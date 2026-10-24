//! Autenticazione a due fattori. Al login, dopo la password, il [`gates::MfaGate`] risponde con
//! una sfida se l'utente ha l'MFA attiva; `POST /auth/mfa/verify` la completa ed emette la sessione.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

pub mod gates;
mod handlers_impl;
pub mod jobs;
pub mod totp;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}

/// Scopo per derivare il codice via email (vedi `noct_core::security::otp`).
pub const PURPOSE_MFA_EMAIL: &str = "mfa_email";
/// Validità del codice via email.
pub const EMAIL_CODE_TTL_MINUTES: i32 = 10;
/// Validità della sfida emessa al login.
pub const CHALLENGE_TTL_SECS: i64 = 300;
pub const CHALLENGE_TYPE: &str = "mfa_challenge";
