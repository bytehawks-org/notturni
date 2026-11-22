//! Regole di validazione di username, password ed email (stesse della v1).

use regex::Regex;
use std::sync::LazyLock;

pub const PASSWORD_MIN_LENGTH: usize = 10;
/// Oltre questa lunghezza l'hashing diventa un vettore di DoS, non una sicurezza in più.
pub const PASSWORD_MAX_BYTES: usize = 1024;
pub const USERNAME_MIN_LENGTH: usize = 3;
pub const USERNAME_MAX_LENGTH: usize = 32;
pub const EMAIL_MAX_LENGTH: usize = 255;

/// Riservati per evitare ambiguità con segmenti di path fissi (es. `/users/me`)
/// o con termini di sistema.
pub const RESERVED_USERNAMES: &[&str] = &[
    "me",
    "admin",
    "api",
    "www",
    "root",
    "system",
    "notturni",
    "support",
    "help",
    "moderator",
    "moderatore",
    "null",
    "undefined",
    "search",
    "users",
];

static USERNAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9]+(?:[-_][a-z0-9]+)*$").expect("regex statica"));

pub fn validate_username(username: &str) -> Result<(), String> {
    let len = username.chars().count();
    if !(USERNAME_MIN_LENGTH..=USERNAME_MAX_LENGTH).contains(&len) {
        return Err(format!("Lo username deve avere da {USERNAME_MIN_LENGTH} a {USERNAME_MAX_LENGTH} caratteri."));
    }
    if !USERNAME_RE.is_match(username) {
        return Err(
            "Lo username può contenere solo lettere minuscole, cifre, '-' e '_' (non a inizio/fine e non ripetuti)."
                .into(),
        );
    }
    if RESERVED_USERNAMES.contains(&username) {
        return Err("Questo username è riservato.".into());
    }
    Ok(())
}

pub fn validate_password_policy(password: &str) -> Result<(), String> {
    if password.chars().count() < PASSWORD_MIN_LENGTH {
        return Err(format!("La password deve avere almeno {PASSWORD_MIN_LENGTH} caratteri."));
    }
    if password.len() > PASSWORD_MAX_BYTES {
        return Err(format!("La password può avere al massimo {PASSWORD_MAX_BYTES} byte."));
    }
    Ok(())
}

/// Minuscolo e senza spazi ai bordi: l'email è un identificatore, non un testo.
pub fn normalize_email(raw: &str) -> String {
    raw.trim().to_lowercase()
}

/// Controllo di forma (non di esistenza: quella la prova solo l'invio di un codice).
pub fn validate_email(email: &str) -> Result<(), String> {
    let err = || "Indirizzo email non valido.".to_string();
    if email.len() > EMAIL_MAX_LENGTH || email.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(err());
    }
    let (local, domain) = email.rsplit_once('@').ok_or_else(err)?;
    let local_ok = !local.is_empty()
        && local.len() <= 64
        && !local.starts_with('.')
        && !local.ends_with('.')
        && !local.contains("..");
    let labels: Vec<&str> = domain.split('.').collect();
    let domain_ok = labels.len() >= 2
        && labels.iter().all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.chars().all(|c| c.is_alphanumeric() || c == '-')
        });
    if local_ok && domain_ok && !local.contains('@') { Ok(()) } else { Err(err()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames() {
        for ok in ["anna", "mario-rossi", "a_b_c", "u123", "abc"] {
            assert!(validate_username(ok).is_ok(), "{ok}");
        }
        for bad in ["ab", "Anna", "-anna", "anna-", "an--na", "an na", "anna!", "admin", "me", &"x".repeat(33)] {
            assert!(validate_username(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn password_policy_bounds() {
        assert!(validate_password_policy("0123456789").is_ok());
        assert!(validate_password_policy("012345678").is_err());
        assert!(validate_password_policy(&"x".repeat(1024)).is_ok());
        assert!(validate_password_policy(&"x".repeat(1025)).is_err());
        assert!(validate_password_policy("àèìòùàèìòù").is_ok(), "si contano i caratteri, non i byte");
    }

    #[test]
    fn emails() {
        for ok in ["a@example.com", "mario.rossi+tag@sub.example.co.uk", "u_1@exam-ple.eu"] {
            assert!(validate_email(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "a",
            "a@",
            "@example.com",
            "a@example",
            "a b@example.com",
            "a@@example.com",
            ".a@example.com",
            "a..b@example.com",
            "a@-x.com",
            "a@x..com",
        ] {
            assert!(validate_email(bad).is_err(), "{bad}");
        }
        assert_eq!(normalize_email("  Anna@Example.COM "), "anna@example.com");
    }
}
