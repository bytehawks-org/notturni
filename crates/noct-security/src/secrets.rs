//! Cifratura a riposo dei segreti che il server deve poter rileggere (es. il segreto TOTP, che serve
//! in chiaro per verificare un codice e quindi non si può salvare come hash).
//!
//! ChaCha20-Poly1305 con nonce casuale per ogni valore; il valore salvato è `enc:v1:<base64url(nonce||cifrato)>`.
//! Un valore senza prefisso è considerato in chiaro (righe scritte prima dell'introduzione della
//! cifratura): si legge ancora e viene riscritto cifrato alla prima occasione.
//!
//! La chiave è dedicata (`NOCT_SECRETS_KEY`) o, in sua assenza, derivata dal segreto JWT con una
//! separazione di dominio: ruotare il segreto JWT senza una chiave dedicata rende illeggibili i valori
//! cifrati, per questo in produzione la chiave dedicata è raccomandata.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit as AeadKeyInit},
};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

const PREFIX: &str = "enc:v1:";
const NONCE_LEN: usize = 12;

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("valore cifrato non valido")]
    Malformed,
    #[error("decifratura non riuscita: chiave diversa da quella usata per cifrare")]
    Decrypt,
}

/// Chiave di 32 byte. Da una chiave dedicata se presente, altrimenti derivata dal segreto JWT.
pub fn derive_key(dedicated: Option<&str>, jwt_secret: &str) -> [u8; 32] {
    let (ikm, info): (&[u8], &[u8]) = match dedicated {
        Some(k) if !k.is_empty() => (k.as_bytes(), b"noct:secrets:v1:dedicated"),
        _ => (jwt_secret.as_bytes(), b"noct:secrets:v1:from-jwt"),
    };
    let mut mac = Hmac::<Sha256>::new_from_slice(ikm).expect("HMAC accetta chiavi di qualsiasi lunghezza");
    mac.update(info);
    mac.finalize().into_bytes().into()
}

pub fn is_sealed(stored: &str) -> bool {
    stored.starts_with(PREFIX)
}

/// Cifra `plaintext`; il risultato è diverso a ogni chiamata (nonce casuale).
pub fn seal(key: &[u8; 32], plaintext: &str) -> String {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).expect("generatore casuale del sistema operativo non disponibile");
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let sealed = cipher.encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes()).expect("cifratura in memoria");
    let mut blob = nonce.to_vec();
    blob.extend(sealed);
    format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(blob))
}

/// Il valore in chiaro. Un valore senza prefisso (scritto prima della cifratura) si restituisce com'è.
pub fn open(key: &[u8; 32], stored: &str) -> Result<String, SecretError> {
    let Some(encoded) = stored.strip_prefix(PREFIX) else { return Ok(stored.to_string()) };
    let blob = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| SecretError::Malformed)?;
    if blob.len() <= NONCE_LEN {
        return Err(SecretError::Malformed);
    }
    let (nonce, cipher_text) = blob.split_at(NONCE_LEN);
    let plain = ChaCha20Poly1305::new(Key::from_slice(key))
        .decrypt(Nonce::from_slice(nonce), cipher_text)
        .map_err(|_| SecretError::Decrypt)?;
    String::from_utf8(plain).map_err(|_| SecretError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_values_round_trip_and_differ_each_time() {
        let key = derive_key(None, "un-segreto-jwt-di-prova-abbastanza-lungo");
        let a = seal(&key, "JBSWY3DPEHPK3PXP");
        let b = seal(&key, "JBSWY3DPEHPK3PXP");
        assert_ne!(a, b, "nonce diverso a ogni cifratura");
        assert!(is_sealed(&a) && !a.contains("JBSWY3DP"));
        assert_eq!(open(&key, &a).unwrap(), "JBSWY3DPEHPK3PXP");
        assert_eq!(open(&key, &b).unwrap(), "JBSWY3DPEHPK3PXP");
    }

    #[test]
    fn plaintext_values_from_before_the_encryption_still_open() {
        let key = derive_key(None, "x");
        assert_eq!(open(&key, "JBSWY3DPEHPK3PXP").unwrap(), "JBSWY3DPEHPK3PXP");
        assert!(!is_sealed("JBSWY3DPEHPK3PXP"));
    }

    #[test]
    fn a_wrong_key_or_a_tampered_value_is_rejected() {
        let key = derive_key(Some("chiave-dedicata"), "jwt");
        let other = derive_key(Some("altra-chiave"), "jwt");
        let sealed = seal(&key, "segreto");
        assert!(matches!(open(&other, &sealed), Err(SecretError::Decrypt)));
        let mut tampered = sealed.clone();
        tampered.pop();
        tampered.push('A');
        assert!(open(&key, &tampered).is_err());
        assert!(matches!(open(&key, "enc:v1:@@@"), Err(SecretError::Malformed)));
        assert!(matches!(open(&key, "enc:v1:AAAA"), Err(SecretError::Malformed)));
    }

    #[test]
    fn dedicated_and_derived_keys_are_distinct_and_stable() {
        assert_eq!(derive_key(None, "jwt"), derive_key(None, "jwt"));
        assert_ne!(derive_key(None, "jwt"), derive_key(None, "jwt2"));
        assert_ne!(derive_key(Some("k"), "jwt"), derive_key(None, "jwt"));
        assert_eq!(derive_key(Some(""), "jwt"), derive_key(None, "jwt"));
    }
}
