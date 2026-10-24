//! Regole del profilo, senza accesso al database.

use regex::Regex;
use std::sync::LazyLock;

/// Un solo cambio di username ogni 5 giorni: è un identificatore citabile (`@menzioni`, permalink).
pub const USERNAME_COOLDOWN_DAYS: i64 = 5;
pub const MAX_SOCIAL_LINKS: i64 = 5;
pub const MAX_FALLBACK_LANGUAGES: usize = 5;
pub const MAX_USER_INTERESTS: usize = 5;
pub const MAX_AVATAR_BYTES: usize = 2 * 1024 * 1024;
/// Come l'utente firma i propri testi.
pub const NAME_STYLES: &[&str] = &["username", "full_name", "display_name", "verified_domain"];
/// Lingue dell'interfaccia selezionabili nel profilo.
pub const UI_LOCALES: &[&str] = &["it", "en"];

static COUNTRY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Z]{2}$").expect("regex statica"));

/// `""` azzera (→ `None`); altrimenti ISO 3166-1 alpha-2 maiuscolo. Solo il formato: non si
/// mantiene un elenco dei paesi.
pub fn normalize_country(raw: &str) -> Result<Option<String>, String> {
    let c = raw.trim().to_uppercase();
    if c.is_empty() {
        return Ok(None);
    }
    if !COUNTRY_RE.is_match(&c) {
        return Err("Il codice paese deve essere ISO 3166-1 di 2 lettere maiuscole (es. IT, FR, DE).".into());
    }
    Ok(Some(c))
}

fn locale(raw: &str) -> Result<String, String> {
    let l = raw.trim().to_lowercase();
    noct_blogs::rules::validate_locale(&l)?;
    Ok(l)
}

/// `""` azzera (→ `None`); altrimenti ISO 639-1 minuscolo.
pub fn normalize_native_language(raw: &str) -> Result<Option<String>, String> {
    if raw.trim().is_empty() { Ok(None) } else { locale(raw).map(Some) }
}

/// Lingue di ripiego: formato valido, senza duplicati (l'ordine di invio è preservato), al più 5.
pub fn normalize_fallback_languages(list: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for raw in list {
        let l = locale(raw)?;
        if !out.contains(&l) {
            out.push(l);
        }
    }
    if out.len() > MAX_FALLBACK_LANGUAGES {
        return Err(format!("Massimo {MAX_FALLBACK_LANGUAGES} lingue di fallback."));
    }
    Ok(out)
}

/// `""` = torna al default di piattaforma (→ `None`).
pub fn normalize_ui_locale(raw: &str) -> Result<Option<String>, String> {
    let l = raw.trim().to_lowercase();
    if l.is_empty() {
        return Ok(None);
    }
    if !UI_LOCALES.contains(&l.as_str()) {
        return Err("Lingua dell'interfaccia non supportata.".into());
    }
    Ok(Some(l))
}

/// Scelta degli interessi: senza duplicati e vuoti (ordine di invio preservato), al più 5, tutte
/// chiavi presenti nell'elenco di piattaforma corrente.
pub fn normalize_interests(keys: &[String], available: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for raw in keys {
        let k = raw.trim().to_lowercase();
        if !k.is_empty() && !out.contains(&k) {
            out.push(k);
        }
    }
    if out.len() > MAX_USER_INTERESTS {
        return Err(format!("Al massimo {MAX_USER_INTERESTS} interessi."));
    }
    let unknown: Vec<&str> = out.iter().filter(|k| !available.contains(k)).map(String::as_str).collect();
    if !unknown.is_empty() {
        return Err(format!("Interessi sconosciuti: {}.", unknown.join(", ")));
    }
    Ok(out)
}

/// Un link social è sempre un URL assoluto http(s): l'URL finisce in un `href`, quindi `javascript:`,
/// `data:` e simili non sono ammessi.
pub fn validate_social_url(url: &str) -> Result<(), String> {
    let lower = url.trim().to_ascii_lowercase();
    let rest = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://"));
    match rest {
        Some(r)
            if !r.is_empty() && !r.starts_with('/') && !url.chars().any(|c| c.is_whitespace() || c.is_control()) =>
        {
            Ok(())
        }
        _ => Err("Il link deve essere un URL http:// o https:// valido.".into()),
    }
}

/// Liste di piattaforma per il sigillo di verifica.
#[derive(Debug, Clone, Default)]
pub struct VerificationLists {
    pub gold: Vec<String>,
    pub silver: Vec<String>,
    pub blue_domains: Vec<String>,
}

fn listed(list: &[String], email: &str, username: &str, domain: &str) -> bool {
    list.iter()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .any(|v| v == email.to_lowercase() || v == username.to_lowercase() || (!domain.is_empty() && v == domain))
}

/// Sigillo del profilo, ricalcolato da zero a ogni lettura. In ordine di priorità: oro (verificati
/// a mano dalla piattaforma) > argento (sostenitori) > blu (dominio email dell'istanza o di
/// un'organizzazione approvata) > bronzo (dominio personale verificato via DNS) > nessuno.
pub fn verification_tier(
    lists: &VerificationLists,
    instance_fqdn: &str,
    email: &str,
    username: &str,
    verified_domain: Option<&str>,
) -> &'static str {
    let domain = email.rsplit_once('@').map(|(_, d)| d.to_lowercase()).unwrap_or_default();
    if listed(&lists.gold, email, username, &domain) {
        "gold"
    } else if listed(&lists.silver, email, username, &domain) {
        "silver"
    } else if !domain.is_empty()
        && (domain == instance_fqdn.to_lowercase()
            || lists.blue_domains.iter().any(|d| d.trim().eq_ignore_ascii_case(&domain)))
    {
        "blue"
    } else if verified_domain.is_some_and(|d| !d.is_empty()) {
        "bronze"
    } else {
        "none"
    }
}

/// Identificatore `did:web` calcolato dall'id (stabile anche se lo username cambia).
pub fn atproto_did(instance_fqdn: &str, user: uuid::Uuid) -> String {
    format!("did:web:{instance_fqdn}:users:{user}")
}

pub fn activitypub_actor_id(instance_fqdn: &str, user: uuid::Uuid) -> String {
    format!("https://{instance_fqdn}/ap/actors/{user}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn country_and_languages_are_normalized() {
        assert_eq!(normalize_country(" it ").unwrap().as_deref(), Some("IT"));
        assert_eq!(normalize_country("  ").unwrap(), None);
        assert!(normalize_country("ITA").is_err() && normalize_country("1a").is_err());
        assert_eq!(normalize_native_language("EN").unwrap().as_deref(), Some("en"));
        assert!(normalize_native_language("eng").is_err());
        assert_eq!(normalize_fallback_languages(&s(&["IT", "en", "it"])).unwrap(), s(&["it", "en"]));
        assert!(normalize_fallback_languages(&s(&["a", "b"])).is_err());
        assert!(normalize_fallback_languages(&s(&["aa", "bb", "cc", "dd", "ee", "ff"])).is_err());
        assert_eq!(normalize_ui_locale("").unwrap(), None);
        assert!(normalize_ui_locale("de").is_err());
    }

    #[test]
    fn interests_are_deduplicated_limited_and_checked_against_the_catalog() {
        let catalog = s(&["music", "art", "food", "travel", "cinema", "gaming"]);
        assert_eq!(normalize_interests(&s(&["Music", "music", " ", "art"]), &catalog).unwrap(), s(&["music", "art"]));
        assert!(normalize_interests(&[], &catalog).unwrap().is_empty());
        let err = normalize_interests(&s(&["music", "nope"]), &catalog).unwrap_err();
        assert!(err.contains("nope"), "{err}");
        assert!(normalize_interests(&catalog, &catalog).is_err(), "6 > 5");
    }

    #[test]
    fn social_links_must_be_http_urls() {
        for ok in ["https://example.com/u/foo", "http://example.com", "HTTPS://Example.com"] {
            assert!(validate_social_url(ok).is_ok(), "{ok}");
        }
        for bad in [
            "javascript:alert(1)",
            "data:text/html,x",
            "//example.com",
            "https://",
            "https:///x",
            "example.com",
            "https://a b.com",
            "ftp://example.com",
        ] {
            assert!(validate_social_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn verification_tier_follows_the_priority_order() {
        let lists = VerificationLists {
            gold: s(&["gold@example.com"]),
            silver: s(&["silvia", "partner.example"]),
            blue_domains: s(&["Org.Example"]),
        };
        let tier =
            |email: &str, user: &str, dom: Option<&str>| verification_tier(&lists, "notturni.eu", email, user, dom);
        assert_eq!(tier("gold@example.com", "foo", Some("x.example")), "gold");
        assert_eq!(tier("a@example.com", "silvia", None), "silver");
        assert_eq!(tier("a@partner.example", "bar", None), "silver", "dominio nella lista argento");
        assert_eq!(tier("a@notturni.eu", "bar", None), "blue");
        assert_eq!(tier("a@org.example", "bar", Some("x.example")), "blue", "blu batte bronzo");
        assert_eq!(tier("a@example.com", "bar", Some("x.example")), "bronze");
        assert_eq!(tier("a@example.com", "bar", Some("")), "none");
        assert_eq!(tier("a@example.com", "bar", None), "none");
    }
}
