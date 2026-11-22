//! Token dei link nelle email, derivati dalla chiave del server (nessuno è salvato).
//!
//! * **Disiscrizione**: `<id iscritto>.<hex HMAC-SHA256(chiave, id)>`, senza scadenza e identico a
//!   quello della v1, così i link già spediti restano validi. Deve poter essere ricostruito a ogni
//!   invio di campagna, per questo non è un valore casuale con il solo hash in database.
//! * **Conferma**: stesso formato, ma la firma copre anche un nonce salvato nella riga: una nuova
//!   richiesta di iscrizione cambia il nonce e invalida il link precedente.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

const CONFIRM_DOMAIN: &[u8] = b"noct:newsletter:confirm:v1";

fn mac(secret: &[u8]) -> HmacSha256 {
    HmacSha256::new_from_slice(secret).expect("HMAC accetta qualunque lunghezza di chiave")
}

fn unsubscribe_mac(secret: &[u8], id: Uuid) -> HmacSha256 {
    let mut m = mac(secret);
    m.update(id.as_bytes());
    m
}

fn confirm_mac(secret: &[u8], id: Uuid, nonce: Uuid) -> HmacSha256 {
    let mut m = mac(secret);
    m.update(CONFIRM_DOMAIN);
    m.update(id.as_bytes());
    m.update(nonce.as_bytes());
    m
}

pub fn unsubscribe_token(secret: &[u8], id: Uuid) -> String {
    format!("{id}.{}", hex::encode(unsubscribe_mac(secret, id).finalize().into_bytes()))
}

pub fn confirm_token(secret: &[u8], id: Uuid, nonce: Uuid) -> String {
    format!("{id}.{}", hex::encode(confirm_mac(secret, id, nonce).finalize().into_bytes()))
}

fn split(token: &str) -> Option<(Uuid, Vec<u8>)> {
    let (raw_id, sig) = token.trim().split_once('.')?;
    Some((Uuid::parse_str(raw_id).ok()?, hex::decode(sig).ok()?))
}

/// L'id dell'iscritto se la firma è valida (confronto a tempo costante).
pub fn verify_unsubscribe_token(secret: &[u8], token: &str) -> Option<Uuid> {
    let (id, sig) = split(token)?;
    unsubscribe_mac(secret, id).verify_slice(&sig).ok().map(|()| id)
}

/// L'id dell'iscritto indicato dal token di conferma, senza averne ancora verificato la firma
/// (serve per leggere il nonce dalla riga).
pub fn confirm_token_subject(token: &str) -> Option<Uuid> {
    split(token).map(|(id, _)| id)
}

/// `true` se la firma corrisponde a `id` e `nonce`.
pub fn verify_confirm_token(secret: &[u8], token: &str, nonce: Uuid) -> bool {
    match split(token) {
        Some((id, sig)) => confirm_mac(secret, id, nonce).verify_slice(&sig).is_ok(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"un-segreto-di-test-lungo-almeno-32-byte!!";

    #[test]
    fn unsubscribe_tokens_match_the_v1_format_and_resist_tampering() {
        let id = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
        let token = unsubscribe_token(SECRET, id);
        // valore calcolato con la v1: `hmac.new(secret, subscriber_id.bytes, sha256).hexdigest()`
        assert_eq!(
            token,
            "11111111-2222-3333-4444-555555555555.069fe04f57d1859e9d05069ca76cd0c83175445f03ded012213d96d7009002f1"
        );
        assert_eq!(verify_unsubscribe_token(SECRET, &token), Some(id));
        assert_eq!(verify_unsubscribe_token(b"altra-chiave-altra-chiave-altra-chiave", &token), None);
        let other = Uuid::new_v4();
        let forged = format!("{other}.{}", token.split_once('.').unwrap().1);
        assert_eq!(verify_unsubscribe_token(SECRET, &forged), None, "la firma è legata all'id");
        for bad in ["", ".", "x.y", &format!("{id}."), &format!("{id}.zz"), &format!("{id}.00"), "no-uuid.00"] {
            assert_eq!(verify_unsubscribe_token(SECRET, bad), None, "{bad}");
        }
    }

    #[test]
    fn confirm_tokens_depend_on_the_nonce() {
        let (id, n1, n2) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let token = confirm_token(SECRET, id, n1);
        assert_eq!(confirm_token_subject(&token), Some(id));
        assert!(verify_confirm_token(SECRET, &token, n1));
        assert!(!verify_confirm_token(SECRET, &token, n2), "un nuovo nonce invalida il vecchio link");
        assert!(!verify_confirm_token(b"altra-chiave-altra-chiave-altra-chiave", &token, n1));
        // il token di conferma non vale come token di disiscrizione, e viceversa
        assert_eq!(verify_unsubscribe_token(SECRET, &token), None);
        assert!(!verify_confirm_token(SECRET, &unsubscribe_token(SECRET, id), n1));
    }
}
