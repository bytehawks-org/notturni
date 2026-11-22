//! TOTP (RFC 6238): HMAC-SHA1, 6 cifre, passo di 30 secondi, come le app di autenticazione comuni.

use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use subtle::ConstantTimeEq;

type HmacSha1 = Hmac<Sha1>;

pub const STEP_SECS: i64 = 30;
pub const DIGITS: u32 = 6;
const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Segreto casuale di 160 bit, in base32 senza padding (32 caratteri), come `pyotp.random_base32`.
pub fn generate_secret() -> Result<String, getrandom::Error> {
    let mut bytes = [0u8; 20];
    getrandom::fill(&mut bytes)?;
    Ok(base32_encode(&bytes))
}

pub fn base32_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &b in data {
        buffer = (buffer << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buffer >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// Decodifica base32 (maiuscole o minuscole, con o senza padding e spazi); `None` se non valido.
pub fn base32_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0u32);
    for c in text.chars().filter(|c| !c.is_whitespace() && *c != '=') {
        let v = ALPHABET.iter().position(|&a| a as char == c.to_ascii_uppercase())? as u32;
        buffer = (buffer << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

/// Codice HOTP (RFC 4226) per `counter`.
pub fn hotp(key: &[u8], counter: u64) -> String {
    let mut mac = HmacSha1::new_from_slice(key).expect("HMAC accetta qualunque lunghezza di chiave");
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let bin = u32::from_be_bytes([digest[offset] & 0x7f, digest[offset + 1], digest[offset + 2], digest[offset + 3]]);
    format!("{:0width$}", bin % 10u32.pow(DIGITS), width = DIGITS as usize)
}

pub fn step_at(unix_secs: i64) -> u64 {
    (unix_secs.max(0) / STEP_SECS) as u64
}

/// Verifica `code` contro il segreto base32 con una tolleranza di un passo per lato. Restituisce
/// l'intervallo che ha corrisposto (serve a non accettare lo stesso codice due volte).
pub fn verify(secret_b32: &str, code: &str, now_unix: i64) -> Option<u64> {
    let code = code.trim();
    if code.len() != DIGITS as usize || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let key = base32_decode(secret_b32)?;
    let current = step_at(now_unix);
    let mut matched = None;
    // nessuna uscita anticipata: il tempo non dipende da quale intervallo corrisponde
    for step in [current.saturating_sub(1), current, current + 1] {
        if bool::from(hotp(&key, step).as_bytes().ct_eq(code.as_bytes())) {
            matched = Some(step);
        }
    }
    matched
}

/// `otpauth://` per le app di autenticazione.
pub fn provisioning_uri(secret_b32: &str, account: &str, issuer: &str) -> String {
    format!(
        "otpauth://totp/{}:{}?secret={secret_b32}&issuer={}",
        percent_encode(issuer),
        percent_encode(account),
        percent_encode(issuer)
    )
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 6238, appendice B: segreto ASCII "12345678901234567890" (SHA-1), qui a 6 cifre
    const RFC_KEY: &[u8] = b"12345678901234567890";

    #[test]
    fn matches_the_rfc_6238_vectors() {
        for (time, expected) in [
            (59i64, "287082"),
            (1_111_111_109, "081804"),
            (1_111_111_111, "050471"),
            (1_234_567_890, "005924"),
            (2_000_000_000, "279037"),
        ] {
            assert_eq!(hotp(RFC_KEY, step_at(time)), expected, "t={time}");
        }
    }

    #[test]
    fn base32_round_trips_and_is_tolerant() {
        assert_eq!(base32_encode(RFC_KEY), "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
        assert_eq!(base32_decode("gezd gnbv GY3TQOJQ GEZDGNBVGY3TQOJQ====").unwrap(), RFC_KEY);
        assert!(base32_decode("not base32!").is_none());
        for len in 0..40 {
            let bytes: Vec<u8> = (0..len).map(|i| (i * 7 + 3) as u8).collect();
            assert_eq!(base32_decode(&base32_encode(&bytes)).unwrap(), bytes);
        }
        let s = generate_secret().unwrap();
        assert_eq!((s.len(), base32_decode(&s).unwrap().len()), (32, 20));
    }

    #[test]
    fn verification_accepts_one_step_of_drift_only() {
        let secret = base32_encode(RFC_KEY);
        let now = 1_111_111_111;
        let code = hotp(RFC_KEY, step_at(now));
        assert_eq!(verify(&secret, &code, now), Some(step_at(now)));
        assert_eq!(verify(&secret, &code, now + 30), Some(step_at(now)), "un passo dopo");
        assert_eq!(verify(&secret, &code, now - 30), Some(step_at(now)), "un passo prima");
        assert_eq!(verify(&secret, &code, now + 90), None, "tre passi dopo");
        for bad in ["", "12345", "1234567", "abcdef", "12 456"] {
            assert_eq!(verify(&secret, bad, now), None, "{bad}");
        }
        assert_eq!(verify("!!!", &code, now), None);
    }

    #[test]
    fn provisioning_uri_is_percent_encoded() {
        assert_eq!(
            provisioning_uri("ABC", "anna@example.com", "Notturni"),
            "otpauth://totp/Notturni:anna%40example.com?secret=ABC&issuer=Notturni"
        );
    }
}
