//! Codici monouso a 6 cifre *derivati*, non memorizzati: il codice è
//! `HMAC(chiave del server, scopo ‖ id)` ridotto a 6 cifre. Il database tiene
//! solo l'id e la scadenza; un job può rigenerare il codice per spedirlo senza
//! che il valore in chiaro passi da outbox, bus o dead letter.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

pub const OTP_DIGITS: usize = 6;

pub fn derive_otp(server_secret: &[u8], purpose: &str, id: Uuid) -> String {
    // chiave dedicata: il segreto principale non firma mai direttamente gli OTP
    let mut kdf = HmacSha256::new_from_slice(server_secret).expect("HMAC accetta qualunque lunghezza di chiave");
    kdf.update(b"noct:otp:v1");
    let key = kdf.finalize().into_bytes();

    let mut m = HmacSha256::new_from_slice(&key).expect("HMAC accetta qualunque lunghezza di chiave");
    m.update(purpose.as_bytes());
    m.update(&[0]);
    m.update(id.as_bytes());
    let digest = m.finalize().into_bytes();
    let n = u64::from_be_bytes(digest[..8].try_into().expect("8 byte"));
    format!("{:0width$}", n % 10u64.pow(OTP_DIGITS as u32), width = OTP_DIGITS)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"un-segreto-di-test-lungo-almeno-32-byte!!";

    #[test]
    fn deterministic_six_digits() {
        let id = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
        let a = derive_otp(SECRET, "password_reset", id);
        assert_eq!(a, derive_otp(SECRET, "password_reset", id));
        assert_eq!(a.len(), 6);
        assert!(a.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn depends_on_secret_purpose_and_id() {
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let base = derive_otp(SECRET, "password_reset", a);
        assert_ne!(base, derive_otp(b"un-altro-segreto-un-altro-segreto-!!", "password_reset", a));
        assert_ne!(base, derive_otp(SECRET, "mfa_email", a));
        assert_ne!(base, derive_otp(SECRET, "password_reset", b));
    }

    #[test]
    fn digits_are_roughly_uniform() {
        // non è un test statistico serio: intercetta solo un errore grossolano (es. cifra fissa)
        let mut first_digits = std::collections::HashSet::new();
        for _ in 0..200 {
            first_digits.insert(derive_otp(SECRET, "t", Uuid::new_v4()).as_bytes()[0]);
        }
        assert!(first_digits.len() >= 8);
    }
}
