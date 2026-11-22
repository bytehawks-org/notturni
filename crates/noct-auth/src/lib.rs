//! Autenticazione: registrazione, sessioni, password, API token.
//!
//! * **Access token**: JWT HS256 di breve durata, nel corpo della risposta (il
//!   frontend lo tiene in memoria).
//! * **Refresh token**: opaco, solo l'hash è in DB, in un cookie `httpOnly` con
//!   path ristretto; rotazione a ogni uso; richieste protette da CSRF double-submit.
//! * **Password**: Argon2id; cambio o reset revocano ogni sessione e invalidano
//!   gli access token già emessi (`users.credentials_changed_at`).
//! * **OTP di reset**: derivato da HMAC, mai memorizzato né messo in coda.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

mod handlers_impl;
pub mod personal_data;
mod settings;
mod util;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}

/// Emissione della sessione, per i moduli che completano un login (MFA, SSO).
pub mod session {
    pub use crate::handlers_impl::{issue_session, session_response};
    pub use crate::util::{clear_refresh_cookie, cookie_value, set_session_cookies};
}
pub use handlers_impl::enqueue_password_reset;
pub mod jobs;
pub mod startup;

pub use settings::AuthSettings;
