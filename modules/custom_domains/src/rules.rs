//! Regole sui domini e sul record di verifica.

use regex::Regex;
use std::sync::LazyLock;

pub const CHALLENGE_PREFIX: &str = "_notturni-challenge";

static HOSTNAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([a-z0-9]([a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,}$").expect("regex statica"));

/// Normalizza (minuscolo, senza punto finale) e valida un dominio. Non ammessi: l'istanza stessa e
/// i suoi sottodomini, gli indirizzi IP e i nomi senza TLD.
pub fn validate_domain(raw: &str, instance_fqdn: &str) -> Result<String, String> {
    let domain = raw.trim().to_lowercase().trim_end_matches('.').to_string();
    if !(4..=255).contains(&domain.len()) || !HOSTNAME_RE.is_match(&domain) || domain.split('.').any(|l| l.len() > 63) {
        return Err("Formato dominio non valido.".into());
    }
    let own = instance_fqdn.to_lowercase();
    if domain == own || domain.ends_with(&format!(".{own}")) {
        return Err("Non puoi usare un (sotto)dominio della piattaforma stessa.".into());
    }
    Ok(domain)
}

pub fn txt_record_name(domain: &str) -> String {
    format!("{CHALLENGE_PREFIX}.{domain}")
}

pub fn txt_record_value(token: &str) -> String {
    format!("notturni-verify={token}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains_are_normalized_and_the_instance_is_reserved() {
        assert_eq!(validate_domain("  Anna.Example.ORG. ", "notturni.eu").unwrap(), "anna.example.org");
        for bad in [
            "",
            "localhost",
            "anna",
            "-a.example.org",
            "a_b.example.org",
            "192.168.1.1",
            "a..example.org",
            "https://anna.example.org",
            "anna.example.org/x",
            "a.b",
            "exa mple.org",
            "anna.example.123",
        ] {
            assert!(validate_domain(bad, "notturni.eu").is_err(), "{bad}");
        }
        assert!(
            validate_domain(&format!("{}.example.org", "a".repeat(64)), "notturni.eu").is_err(),
            "etichetta oltre 63 caratteri"
        );
        for own in ["notturni.eu", "blog.notturni.eu", "NOTTURNI.EU."] {
            let err = validate_domain(own, "notturni.eu").unwrap_err();
            assert!(err.contains("piattaforma"), "{own}: {err}");
        }
        assert!(
            validate_domain("notturni.eu.example.org", "notturni.eu").is_ok(),
            "solo il suffisso esatto è riservato"
        );
        assert!(validate_domain("anotturni.eu", "notturni.eu").is_ok());
    }

    #[test]
    fn challenge_record_has_the_v1_format() {
        assert_eq!(txt_record_name("anna.example.org"), "_notturni-challenge.anna.example.org");
        assert_eq!(txt_record_value("abc"), "notturni-verify=abc");
    }
}
