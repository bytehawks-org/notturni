//! Regole di dominio dei commenti: modalità effettiva e hash delle email.

use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};

pub const MODES: &[&str] = &["everyone", "members", "closed"];
pub const DEFAULT_MODE: &str = "members";

/// Modalità effettiva: l'override del post, altrimenti quella del blog; un post diventa `closed`
/// quando sono passati `auto_close_days` dalla pubblicazione.
pub fn effective_mode(
    post_override: Option<&str>,
    blog_mode: Option<&str>,
    auto_close_days: Option<i32>,
    published_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> String {
    let mode = post_override.or(blog_mode).unwrap_or(DEFAULT_MODE);
    if mode != "closed"
        && let (Some(days), Some(published)) = (auto_close_days.filter(|d| *d > 0), published_at)
        && published + Duration::days(i64::from(days)) <= now
    {
        return "closed".into();
    }
    mode.to_string()
}

/// Hash sha256 dell'email normalizzata: identifica un anonimo bloccato senza conservare l'email.
pub fn email_hash(email: &str) -> String {
    hex::encode(Sha256::digest(email.trim().to_lowercase().as_bytes()))
}

/// Controllo volutamente semplice: una sola chiocciola, dominio con un punto, nessuno spazio.
pub fn valid_email(email: &str) -> bool {
    let e = email.trim();
    let Some((local, domain)) = e.split_once('@') else { return false };
    e.len() <= 255
        && !local.is_empty()
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !e.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_post_overrides_the_blog_and_old_posts_close_automatically() {
        let now = Utc::now();
        let old = Some(now - Duration::days(40));
        assert_eq!(effective_mode(None, None, None, old, now), "members");
        assert_eq!(effective_mode(None, Some("everyone"), None, old, now), "everyone");
        assert_eq!(effective_mode(Some("closed"), Some("everyone"), None, None, now), "closed");
        assert_eq!(effective_mode(Some("everyone"), Some("closed"), None, None, now), "everyone");
        assert_eq!(effective_mode(None, Some("everyone"), Some(30), old, now), "closed");
        assert_eq!(effective_mode(None, Some("everyone"), Some(60), old, now), "everyone");
        assert_eq!(effective_mode(None, Some("everyone"), Some(30), None, now), "everyone", "non pubblicato");
        assert_eq!(effective_mode(None, Some("everyone"), Some(0), old, now), "everyone", "0 = mai");
    }

    #[test]
    fn emails_are_hashed_case_insensitively_and_checked_simply() {
        assert_eq!(email_hash(" Anna@Example.com "), email_hash("anna@example.com"));
        assert_ne!(email_hash("a@example.com"), email_hash("b@example.com"));
        assert_eq!(email_hash("a@b.it").len(), 64);
        for ok in ["a@example.com", "a.b+c@sub.example.org"] {
            assert!(valid_email(ok), "{ok}");
        }
        for bad in ["", "a", "a@", "@example.com", "a@b", "a b@example.com", "a@@example.com", "a@example.", "a@.com"] {
            assert!(!valid_email(bad), "{bad}");
        }
    }
}
