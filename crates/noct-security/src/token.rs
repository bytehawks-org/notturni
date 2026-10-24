//! Token opachi: generazione casuale e hash. Solo l'hash sha256 viene
//! persistito; il valore in chiaro è mostrato una volta sola.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};

pub const API_TOKEN_PREFIX: &str = "noct_";
pub const REFRESH_TOKEN_PREFIX: &str = "noct_sess_";
/// Caratteri del token in chiaro conservati per riconoscerlo nelle liste.
pub const PREFIX_DISPLAY_LENGTH: usize = 16;

/// SHA-256 esadecimale. Basta per valori già ad alta entropia (token generati
/// qui); non va usato per password o codici a bassa entropia.
pub fn sha256_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

/// `num_bytes` byte dal generatore del sistema operativo, in base64 url-safe.
/// Il generatore del sistema non fallisce in pratica; se succedesse, meglio un
/// panico che un token prevedibile.
pub fn random_secret(num_bytes: usize) -> String {
    let mut buf = vec![0u8; num_bytes];
    getrandom::fill(&mut buf).expect("generatore casuale del sistema operativo non disponibile");
    URL_SAFE_NO_PAD.encode(buf)
}

/// Un API token: `(valore in chiaro, prefisso identificativo, hash)`.
pub fn generate_api_token() -> (String, String, String) {
    let plaintext = format!("{API_TOKEN_PREFIX}{}", random_secret(32));
    let prefix = plaintext[..PREFIX_DISPLAY_LENGTH].to_string();
    let hash = sha256_hex(&plaintext);
    (plaintext, prefix, hash)
}

/// Un refresh token di sessione: `(valore in chiaro, hash)`.
pub fn generate_refresh_token() -> (String, String) {
    let plaintext = format!("{REFRESH_TOKEN_PREFIX}{}", random_secret(32));
    let hash = sha256_hex(&plaintext);
    (plaintext, hash)
}

/// I token di sessione condividono il prefisso `noct_` con gli API token:
/// un bearer `noct_sess_…` non è mai un API token.
pub fn is_api_token(bearer: &str) -> bool {
    bearer.starts_with(API_TOKEN_PREFIX) && !bearer.starts_with(REFRESH_TOKEN_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_random_url_safe_and_long_enough() {
        let a = random_secret(32);
        let b = random_secret(32);
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn api_token_shape_and_hash() {
        let (plain, prefix, hash) = generate_api_token();
        assert!(plain.starts_with("noct_") && plain.len() > 40);
        assert_eq!(prefix, &plain[..16]);
        assert_eq!(hash, sha256_hex(&plain));
        assert_eq!(hash.len(), 64);
        assert!(is_api_token(&plain));
    }

    #[test]
    fn refresh_tokens_are_not_api_tokens() {
        let (plain, hash) = generate_refresh_token();
        assert!(plain.starts_with("noct_sess_"));
        assert_eq!(hash, sha256_hex(&plain));
        assert!(!is_api_token(&plain));
        assert!(!is_api_token("eyJhbGciOi..."));
    }

    #[test]
    fn sha256_known_vector() {
        assert_eq!(sha256_hex("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
