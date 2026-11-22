use crate::BusError;

fn token_ok(t: &str) -> bool {
    !t.is_empty() && t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Subject concreto di pubblicazione: token `[a-z0-9_-]` separati da punti, niente wildcard.
pub fn validate_subject(s: &str) -> Result<(), BusError> {
    if s.len() <= 200 && !s.is_empty() && s.split('.').all(token_ok) {
        Ok(())
    } else {
        Err(BusError::InvalidSubject(s.to_string()))
    }
}

/// Filtro di sottoscrizione: come un subject, ma con `*` (un token) e `>` (il resto, solo in coda).
pub fn validate_filter(s: &str) -> Result<(), BusError> {
    let tokens: Vec<&str> = s.split('.').collect();
    let ok = s.len() <= 200
        && !s.is_empty()
        && tokens.iter().enumerate().all(|(i, t)| token_ok(t) || *t == "*" || (*t == ">" && i == tokens.len() - 1));
    if ok { Ok(()) } else { Err(BusError::InvalidSubject(s.to_string())) }
}

/// Semantica dei subject NATS.
pub fn subject_matches(filter: &str, subject: &str) -> bool {
    let mut f = filter.split('.');
    let mut s = subject.split('.');
    loop {
        match (f.next(), s.next()) {
            (Some(">"), Some(_)) => return true,
            (Some("*"), Some(_)) => {}
            (Some(a), Some(b)) if a == b => {}
            (None, None) => return true,
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_follows_nats_rules() {
        assert!(subject_matches("post.published", "post.published"));
        assert!(!subject_matches("post.published", "post.updated"));
        assert!(subject_matches("post.*", "post.published"));
        assert!(!subject_matches("post.*", "post.published.x"));
        assert!(subject_matches("post.>", "post.published.x"));
        assert!(!subject_matches("post.>", "post"));
        assert!(subject_matches("*.published", "post.published"));
        assert!(!subject_matches("a.b", "a"));
    }

    #[test]
    fn validation() {
        assert!(validate_subject("sandbox.note.created").is_ok());
        for bad in ["", "a..b", "a.*", "a.>", "A.b", "a b", ".a"] {
            assert!(validate_subject(bad).is_err(), "{bad:?}");
        }
        assert!(validate_filter("a.*.c").is_ok() && validate_filter("a.>").is_ok());
        assert!(validate_filter("a.>.c").is_err() && validate_filter("").is_err());
    }
}
