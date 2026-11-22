//! Hash delle password con Argon2id (formato PHC): gli hash prodotti dalla
//! versione Python (argon2-cffi) restano verificabili, perché i parametri
//! viaggiano dentro la stringa.

use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use std::sync::LazyLock;

#[derive(Debug, thiserror::Error)]
#[error("hash della password non riuscito: {0}")]
pub struct HashError(String);

pub fn hash_password(plain: &str) -> Result<String, HashError> {
    Argon2::default().hash_password(plain.as_bytes()).map(|h| h.to_string()).map_err(|e| HashError(e.to_string()))
}

/// `false` per password sbagliata *e* per hash illeggibile: chi chiama non deve
/// distinguere i due casi (nessuna informazione utile a un attaccante).
pub fn verify_password(plain: &str, hashed: &str) -> bool {
    Argon2::default().verify_password(plain.as_bytes(), hashed).is_ok()
}

static DUMMY_HASH: LazyLock<String> =
    LazyLock::new(|| hash_password("password-fittizia-per-equalizzare-i-tempi").expect("hash di una costante"));

/// Esegue una verifica "a vuoto" con lo stesso costo di una vera: da chiamare
/// quando l'utente non esiste, perché il tempo di risposta non riveli se
/// un'email è registrata.
pub fn verify_dummy(plain: &str) {
    let _ = verify_password(plain, &DUMMY_HASH);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_roundtrip() {
        let h = hash_password("correct horse battery").unwrap();
        assert!(h.starts_with("$argon2id$"));
        assert!(verify_password("correct horse battery", &h));
        assert!(!verify_password("correct horse batterz", &h));
        assert!(!verify_password("", &h));
    }

    #[test]
    fn salts_make_hashes_unique() {
        assert_ne!(hash_password("stessa-password").unwrap(), hash_password("stessa-password").unwrap());
    }

    #[test]
    fn garbage_hash_is_a_failed_verification_not_a_panic() {
        assert!(!verify_password("x", ""));
        assert!(!verify_password("x", "non-un-hash"));
        assert!(!verify_password("x", "$argon2id$v=19$m=1,t=1,p=1$AAAA$AAAA"));
    }

    #[test]
    fn hashes_made_by_argon2_cffi_still_verify() {
        // generato con argon2-cffi (PasswordHasher() di default: m=65536,t=3,p=4) per "legacy-password-1"
        let legacy =
            "$argon2id$v=19$m=65536,t=3,p=4$lsWEqgkUtZ2Qq8fdFLlAFQ$6303wkuvTgq0lQrl4pKKdyUrzJsA1l2mJhZ8jdAEpdc";
        assert!(verify_password("legacy-password-1", legacy));
        assert!(!verify_password("legacy-password-2", legacy));
    }

    #[test]
    fn dummy_verification_does_not_panic() {
        verify_dummy("qualsiasi");
    }
}
