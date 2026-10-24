//! JWT HS256 minimale, solo ciò che serve alla piattaforma: access token e
//! challenge. Implementazione volutamente piccola e rigida:
//! * accetta **solo** `alg = HS256` (mai `none`, mai algoritmi asimmetrici);
//! * firma verificata a tempo costante prima di leggere i claim;
//! * `exp` obbligatorio e `type` verificato, così un token di un tipo non è
//!   usabile come un altro.
//!
//! Compatibile con i token emessi da PyJWT (`sub`, `type`, `iat`, `exp`).

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Tolleranza sull'orologio per `iat` nel futuro (secondi).
const LEEWAY_SECS: i64 = 30;
const HEADER: &str = r#"{"alg":"HS256","typ":"JWT"}"#;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JwtError {
    #[error("token non valido o scaduto")]
    Invalid,
    #[error("token di tipo inatteso")]
    WrongType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    #[serde(rename = "type")]
    pub typ: String,
    /// Emissione e scadenza, in secondi Unix.
    pub iat: i64,
    pub exp: i64,
}

fn mac(secret: &[u8]) -> HmacSha256 {
    // HMAC accetta chiavi di qualsiasi lunghezza
    HmacSha256::new_from_slice(secret).expect("HMAC accetta qualunque lunghezza di chiave")
}

/// Emette un token di tipo `typ` per `sub`, valido `ttl_secs` da `now`.
pub fn encode(secret: &[u8], typ: &str, sub: Uuid, now: i64, ttl_secs: i64) -> String {
    let claims = Claims { sub, typ: typ.to_string(), iat: now, exp: now + ttl_secs };
    let payload = serde_json::to_vec(&claims).expect("claims serializzabili");
    let signing_input = format!("{}.{}", URL_SAFE_NO_PAD.encode(HEADER), URL_SAFE_NO_PAD.encode(payload));
    let mut m = mac(secret);
    m.update(signing_input.as_bytes());
    format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(m.finalize().into_bytes()))
}

/// Verifica firma, algoritmo, scadenza e tipo; restituisce i claim.
pub fn decode(secret: &[u8], token: &str, expected_type: &str, now: i64) -> Result<Claims, JwtError> {
    let mut parts = token.split('.');
    let (Some(h), Some(p), Some(s), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err(JwtError::Invalid);
    };
    let header: serde_json::Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(h).map_err(|_| JwtError::Invalid)?)
        .map_err(|_| JwtError::Invalid)?;
    if header.get("alg").and_then(|a| a.as_str()) != Some("HS256") {
        return Err(JwtError::Invalid);
    }
    let signature = URL_SAFE_NO_PAD.decode(s).map_err(|_| JwtError::Invalid)?;
    let mut m = mac(secret);
    m.update(format!("{h}.{p}").as_bytes());
    m.verify_slice(&signature).map_err(|_| JwtError::Invalid)?;

    let claims: Claims = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(p).map_err(|_| JwtError::Invalid)?)
        .map_err(|_| JwtError::Invalid)?;
    if claims.exp <= now || claims.iat > now + LEEWAY_SECS {
        return Err(JwtError::Invalid);
    }
    if claims.typ != expected_type {
        return Err(JwtError::WrongType);
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"un-segreto-di-test-lungo-almeno-32-byte!!";

    fn user() -> Uuid {
        Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap()
    }

    #[test]
    fn roundtrip() {
        let t = encode(SECRET, "access", user(), 1_000, 900);
        let c = decode(SECRET, &t, "access", 1_500).unwrap();
        assert_eq!((c.sub, c.typ.as_str(), c.iat, c.exp), (user(), "access", 1_000, 1_900));
    }

    #[test]
    fn expiry_is_enforced() {
        let t = encode(SECRET, "access", user(), 1_000, 900);
        assert!(decode(SECRET, &t, "access", 1_899).is_ok());
        assert_eq!(decode(SECRET, &t, "access", 1_900), Err(JwtError::Invalid));
        assert_eq!(decode(SECRET, &t, "access", 5_000), Err(JwtError::Invalid));
    }

    #[test]
    fn tokens_from_the_future_are_rejected_beyond_leeway() {
        let t = encode(SECRET, "access", user(), 10_000, 900);
        assert!(decode(SECRET, &t, "access", 9_980).is_ok(), "entro la tolleranza");
        assert_eq!(decode(SECRET, &t, "access", 9_000), Err(JwtError::Invalid));
    }

    #[test]
    fn wrong_secret_and_wrong_type_are_rejected() {
        let t = encode(SECRET, "access", user(), 1_000, 900);
        assert_eq!(decode(b"un-altro-segreto-un-altro-segreto-!!", &t, "access", 1_100), Err(JwtError::Invalid));
        assert_eq!(decode(SECRET, &t, "mfa_challenge", 1_100), Err(JwtError::WrongType));
    }

    #[test]
    fn tampering_is_detected() {
        let t = encode(SECRET, "access", user(), 1_000, 900);
        let parts: Vec<&str> = t.split('.').collect();
        // payload alterato (stesso sub, exp molto più lontana) con la firma originale
        let forged = Claims { sub: user(), typ: "access".into(), iat: 1_000, exp: 99_999_999 };
        let forged_p = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&forged).unwrap());
        assert_eq!(
            decode(SECRET, &format!("{}.{forged_p}.{}", parts[0], parts[2]), "access", 1_100),
            Err(JwtError::Invalid)
        );
        // firma troncata o vuota
        assert_eq!(decode(SECRET, &format!("{}.{}.", parts[0], parts[1]), "access", 1_100), Err(JwtError::Invalid));
        assert_eq!(decode(SECRET, &format!("{}.{}.AAAA", parts[0], parts[1]), "access", 1_100), Err(JwtError::Invalid));
    }

    #[test]
    fn alg_none_and_other_algorithms_are_rejected() {
        let payload = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&Claims { sub: user(), typ: "access".into(), iat: 1_000, exp: 99_999 }).unwrap(),
        );
        for alg in ["none", "HS512", "RS256", "hs256"] {
            let h = URL_SAFE_NO_PAD.encode(format!(r#"{{"alg":"{alg}","typ":"JWT"}}"#));
            assert_eq!(decode(SECRET, &format!("{h}.{payload}."), "access", 1_100), Err(JwtError::Invalid), "{alg}");
            assert_eq!(
                decode(SECRET, &format!("{h}.{payload}.AAAA"), "access", 1_100),
                Err(JwtError::Invalid),
                "{alg}"
            );
        }
    }

    #[test]
    fn malformed_input_never_panics() {
        for t in ["", ".", "..", "a.b", "a.b.c.d", "!!!.???.***", "eyJhbGciOiJIUzI1NiJ9.e30.x"] {
            assert!(decode(SECRET, t, "access", 1).is_err(), "{t:?}");
        }
    }

    #[test]
    fn tokens_issued_by_pyjwt_decode() {
        // generato con PyJWT: jwt.encode({"sub": ..., "type": "access", "iat": 1000, "exp": 1900}, SECRET, "HS256")
        let t = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMTExMTExMS0yMjIyLTMzMzMtNDQ0NC01NTU1NTU1NTU1NTUiLCJ0eXBlIjoiYWNjZXNzIiwiaWF0IjoxMDAwLCJleHAiOjE5MDB9.mcwdDJ8UU3wtIOb0nCWJfzBc3MujictmCDt1oCGmOGM";
        let c = decode(SECRET, t, "access", 1_500).unwrap();
        assert_eq!((c.sub, c.iat, c.exp), (user(), 1_000, 1_900));
        // e viceversa: il nostro token ha esattamente la stessa firma di PyJWT a parità di header e claim
        assert_eq!(encode(SECRET, "access", user(), 1_000, 900).rsplit('.').next(), t.rsplit('.').next());
    }
}
