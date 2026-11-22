//! Regole pure dei testi dell'interfaccia modificabili dagli amministratori.

use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

/// Gruppi di messaggi del frontend che gli amministratori possono riscrivere: la home (manifesto, pilastri,
/// pulsanti) e il modulo di iscrizione alla newsletter. Estendere l'elenco è una scelta di prodotto, non solo tecnica.
pub const EDITABLE_PREFIXES: &[&str] = &["Home.", "NewsletterSignup."];
pub const MAX_VALUE_CHARS: usize = 2000;
pub const MAX_CHANGES: usize = 200;

static KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z][A-Za-z0-9]*(\.[A-Za-z][A-Za-z0-9]*)+$").expect("regex valida"));
static LOCALE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z]{2}$").expect("regex valida"));

pub fn validate_text_locale(locale: &str) -> Result<(), String> {
    if LOCALE.is_match(locale) { Ok(()) } else { Err("Lingua non valida: due lettere minuscole (es. it, en).".into()) }
}

pub fn validate_key(key: &str) -> Result<(), String> {
    if key.len() > 120 || !KEY.is_match(key) || !EDITABLE_PREFIXES.iter().any(|p| key.starts_with(p)) {
        return Err(format!("Il testo `{key}` non è modificabile."));
    }
    Ok(())
}

/// Tag ammessi per chiave: il componente che mostra quel messaggio fornisce il renderer del tag (qui il link
/// all'informativa sulla privacy). Per tutte le altre chiavi nessun tag.
pub const RICH_TAGS: &[(&str, &[&str])] = &[("NewsletterSignup.consent", &["privacy"])];

/// Il testo viene mostrato da un renderer di messaggi ICU: graffe e angolari lo spezzerebbero (o iniettano
/// markup), quindi è testo semplice, una riga, senza caratteri di controllo. Sono ammessi solo i tag di
/// `RICH_TAGS` per quella chiave, ben formati (aperti e chiusi, mai annidati o ripetuti).
pub fn validate_value(key: &str, value: &str) -> Result<(), String> {
    if value.chars().count() > MAX_VALUE_CHARS {
        return Err(format!("`{key}`: al massimo {MAX_VALUE_CHARS} caratteri."));
    }
    // un apostrofo davanti a un angolare avvia una citazione nei messaggi ICU
    if value.contains("'<") || value.contains("'>") {
        return Err(format!("`{key}`: niente apostrofo subito prima di < o >."));
    }
    let tags = RICH_TAGS.iter().find(|(k, _)| *k == key).map_or(&[][..], |(_, t)| *t);
    let mut rest = value.to_string();
    for tag in tags {
        let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
        let (o, c) = (rest.matches(&open).count(), rest.matches(&close).count());
        let ordered = rest.find(&open).zip(rest.find(&close)).is_none_or(|(a, b)| a < b);
        if o != c || o > 1 || !ordered {
            return Err(format!("`{key}`: il tag <{tag}> va aperto e chiuso una sola volta."));
        }
        rest = rest.replace(&open, "").replace(&close, "");
    }
    if rest.chars().any(|c| c.is_control() || matches!(c, '{' | '}' | '<' | '>'))
        || rest.contains("'<")
        || rest.contains("'>")
    {
        return Err(format!("`{key}`: testo semplice su una riga, senza {{ }} < >."));
    }
    Ok(())
}

/// Le modifiche richieste: `(chiave, Some(testo))` da impostare, `(chiave, None)` da ripristinare (valore
/// `null` o stringa vuota/di soli spazi).
pub fn parse_changes(texts: &Value) -> Result<Vec<(String, Option<String>)>, String> {
    let map = texts.as_object().ok_or("`texts` deve essere un oggetto chiave → testo.")?;
    if map.is_empty() || map.len() > MAX_CHANGES {
        return Err(format!("Servono da 1 a {MAX_CHANGES} modifiche."));
    }
    let mut out = Vec::with_capacity(map.len());
    for (key, value) in map {
        validate_key(key)?;
        let value = match value {
            Value::Null => None,
            Value::String(s) if s.trim().is_empty() => None,
            Value::String(s) => {
                validate_value(key, s.trim())?;
                Some(s.trim().to_string())
            }
            _ => return Err(format!("`{key}`: serve un testo oppure null.")),
        };
        out.push((key.clone(), value));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_editable_groups_and_plain_text_are_accepted() {
        assert!(validate_key("Home.pillars.noAi.title").is_ok());
        assert!(validate_key("NewsletterSignup.title").is_ok());
        for bad in [
            "Home",
            "Home.",
            "Nav.overview",
            "Admin.title",
            "home.title",
            "Home..x",
            "Home.x y",
            "Home.pillars.",
            "../Home.x",
        ] {
            assert!(validate_key(bad).is_err(), "{bad}");
        }
        assert!(validate_value("Home.eyebrow", "Open source · multilingua").is_ok());
        for bad in ["{count} blog", "<b>x</b>", "a\nb", "x\u{0}y", &"x".repeat(2001)] {
            assert!(validate_value("Home.eyebrow", bad).is_err(), "{bad:?}");
        }
        // il tag del link alla privacy è ammesso solo sulla chiave che lo prevede, ben formato
        let consent = "NewsletterSignup.consent";
        assert!(validate_value(consent, "Accetto la <privacy>informativa</privacy> e l'email").is_ok());
        assert!(validate_value(consent, "Accetto l'informativa").is_ok(), "il tag si può anche togliere");
        for bad in [
            "<privacy>aperto",
            "chiuso</privacy>",
            "<privacy>a</privacy><privacy>b</privacy>",
            "</privacy>x<privacy>",
            "<b>x</b>",
            "l'<privacy>x</privacy>",
        ] {
            assert!(validate_value(consent, bad).is_err(), "{bad}");
        }
        assert!(validate_value("Home.eyebrow", "<privacy>x</privacy>").is_err(), "tag non ammesso su altre chiavi");
        assert!(validate_text_locale("it").is_ok());
        assert!(["", "ita", "IT", "i1"].iter().all(|l| validate_text_locale(l).is_err()));
    }

    #[test]
    fn changes_set_or_restore_and_reject_anything_odd() {
        let ok =
            parse_changes(&json!({"Home.eyebrow": "  Ciao ", "Home.limits": null, "Home.startBlog": "  "})).unwrap();
        assert!(ok.contains(&("Home.eyebrow".to_string(), Some("Ciao".to_string()))));
        assert!(ok.contains(&("Home.limits".to_string(), None)) && ok.contains(&("Home.startBlog".to_string(), None)));
        for bad in [
            json!({}),
            json!([]),
            json!("x"),
            json!({"Home.eyebrow": 3}),
            json!({"Nav.home": "x"}),
            json!({"Home.x": "{y}"}),
        ] {
            assert!(parse_changes(&bad).is_err(), "{bad}");
        }
    }
}
