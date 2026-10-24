//! Parti comuni agli handler: cookie di sessione, CSRF, hashing fuori dal thread
//! async, risposta di sessione.

use crate::AuthSettings;
use axum::{
    http::{HeaderMap, HeaderValue, header},
    response::Response,
};
use noct_core::ApiError;
use noct_security::{
    constant_time_eq,
    cookie::{Cookie, read_cookie},
    password,
    token::random_secret,
};

pub const REFRESH_COOKIE: &str = "noct_refresh_token";
/// Il cookie di refresh viaggia solo verso gli endpoint di autenticazione.
pub const REFRESH_COOKIE_PATH: &str = "/api/v1/auth";
pub const CSRF_COOKIE: &str = "noct_csrf_token";
pub const CSRF_HEADER: &str = "x-csrf-token";

pub const MSG_BAD_CREDENTIALS: &str = "Email o password non corrette.";
pub const MSG_INACTIVE: &str = "Utente disattivato.";
pub const MSG_BAD_CODE: &str = "Codice non valido o scaduto.";

/// Argon2 è CPU-bound (decine di ms): fuori dal thread del runtime, altrimenti
/// poche richieste di login concorrenti bloccherebbero l'intera API.
pub async fn hash_password(plain: String) -> Result<String, ApiError> {
    tokio::task::spawn_blocking(move || password::hash_password(&plain))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::internal)
}

pub async fn verify_password(plain: String, hashed: String) -> Result<bool, ApiError> {
    tokio::task::spawn_blocking(move || password::verify_password(&plain, &hashed)).await.map_err(ApiError::internal)
}

pub async fn verify_dummy(plain: String) -> Result<(), ApiError> {
    tokio::task::spawn_blocking(move || password::verify_dummy(&plain)).await.map_err(ApiError::internal)
}

fn refresh_cookie<'a>(s: &'a AuthSettings, value: &'a str, max_age: i64) -> Cookie<'a> {
    Cookie {
        name: REFRESH_COOKIE,
        value,
        path: REFRESH_COOKIE_PATH,
        domain: s.cookie_domain.as_deref(),
        max_age_secs: max_age,
        http_only: true,
        secure: s.cookie_secure,
        same_site: s.same_site,
    }
}

/// Leggibile da JavaScript (serve al double-submit) e valido su tutto il sito.
fn csrf_cookie<'a>(s: &'a AuthSettings, value: &'a str, max_age: i64) -> Cookie<'a> {
    Cookie {
        name: CSRF_COOKIE,
        value,
        path: "/",
        domain: s.cookie_domain.as_deref(),
        max_age_secs: max_age,
        http_only: false,
        secure: s.cookie_secure,
        same_site: s.same_site,
    }
}

fn append(res: &mut Response, header_value: String) -> Result<(), ApiError> {
    let v = HeaderValue::from_str(&header_value).map_err(ApiError::internal)?;
    res.headers_mut().append(header::SET_COOKIE, v);
    Ok(())
}

/// Imposta refresh e CSRF sulla risposta.
pub fn set_session_cookies(res: &mut Response, s: &AuthSettings, refresh_token: &str) -> Result<(), ApiError> {
    let csrf = random_secret(32);
    append(res, refresh_cookie(s, refresh_token, s.refresh_ttl_secs()).to_header().map_err(ApiError::internal)?)?;
    append(res, csrf_cookie(s, &csrf, s.refresh_ttl_secs()).to_header().map_err(ApiError::internal)?)
}

pub fn clear_session_cookies(res: &mut Response, s: &AuthSettings) -> Result<(), ApiError> {
    append(res, refresh_cookie(s, "", 0).to_clearing_header().map_err(ApiError::internal)?)?;
    append(res, csrf_cookie(s, "", 0).to_clearing_header().map_err(ApiError::internal)?)
}

/// Solo il cookie di refresh: dopo un refresh fallito il CSRF non ha valore di
/// confidenzialità e lasciarlo evita di rompere una richiesta legittima concorrente.
pub fn clear_refresh_cookie(res: &mut Response, s: &AuthSettings) -> Result<(), ApiError> {
    append(res, refresh_cookie(s, "", 0).to_clearing_header().map_err(ApiError::internal)?)
}

/// Double-submit: il cookie e l'header devono coincidere. Richiesto sui soli endpoint
/// autenticati da cookie (refresh, logout): le altre scritture usano `Authorization`,
/// che un'origine estranea non può impostare.
pub fn verify_csrf(headers: &HeaderMap) -> Result<(), ApiError> {
    let cookie = cookie_value(headers, CSRF_COOKIE);
    let header = headers.get(CSRF_HEADER).and_then(|v| v.to_str().ok());
    match (cookie, header) {
        (Some(c), Some(h)) if !c.is_empty() && constant_time_eq(c, h) => Ok(()),
        _ => Err(ApiError::forbidden("CSRF token mancante o non valido.")),
    }
}

pub fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(header::COOKIE).and_then(|v| v.to_str().ok()).and_then(|h| read_cookie(h, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(cookie: Option<&str>, csrf: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(c) = cookie {
            h.insert(header::COOKIE, c.parse().unwrap());
        }
        if let Some(c) = csrf {
            h.insert(CSRF_HEADER, c.parse().unwrap());
        }
        h
    }

    #[test]
    fn csrf_requires_matching_cookie_and_header() {
        assert!(verify_csrf(&headers(Some("noct_csrf_token=abc"), Some("abc"))).is_ok());
        assert!(verify_csrf(&headers(Some("x=1; noct_csrf_token=abc"), Some("abc"))).is_ok());
        assert!(verify_csrf(&headers(Some("noct_csrf_token=abc"), Some("abd"))).is_err());
        assert!(verify_csrf(&headers(Some("noct_csrf_token=abc"), None)).is_err());
        assert!(verify_csrf(&headers(None, Some("abc"))).is_err());
        assert!(verify_csrf(&headers(Some("noct_csrf_token="), Some(""))).is_err(), "vuoti non valgono");
        assert!(verify_csrf(&headers(None, None)).is_err());
    }
}
