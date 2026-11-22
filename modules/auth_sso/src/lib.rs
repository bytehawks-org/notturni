//! Accesso con provider esterni (OAuth2/OIDC).
//!
//! L'identità arriva solo da uno scambio diretto del codice con il provider (state e PKCE legati
//! al browser da un cookie httpOnly) e dal suo userinfo. Un'identità si collega a un account
//! esistente solo se il provider garantisce l'email, e dopo la verifica MFA se l'account ce l'ha.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

pub mod gates;
mod handlers_impl;
pub mod personal_data;
pub mod providers;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}

pub mod admin_sections;
