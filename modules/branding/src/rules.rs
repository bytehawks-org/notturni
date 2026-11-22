//! Validazione pura delle impostazioni di identità e SEO. Tutto ciò che finisce in un meta tag è testo semplice:
//! niente angolari, virgolette o caratteri di controllo (il frontend lo scrive comunque con escape, questa è
//! la seconda barriera).

use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

pub const MAX_EXTRA_META: usize = 10;
pub const MAX_LOCALES: usize = 20;

static TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_-]{10,200}$").expect("regex valida"));
static TWITTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^@?[A-Za-z0-9_]{1,15}$").expect("regex valida"));
static COLOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#[0-9a-fA-F]{6}$").expect("regex valida"));
static META_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z][A-Za-z0-9:._-]{1,59}$").expect("regex valida"));
static LOCALE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z]{2}$").expect("regex valida"));

/// Meta tag che il portale gestisce da sé o che cambierebbero il comportamento delle pagine: non si possono
/// aggiungere a mano.
const RESERVED_META: &[&str] = &[
    "viewport",
    "robots",
    "googlebot",
    "bingbot",
    "description",
    "keywords",
    "refresh",
    "theme-color",
    "referrer",
    "content-security-policy",
    "set-cookie",
    "generator",
    "author",
    "format-detection",
    "color-scheme",
];

/// Una riga di testo semplice: senza angolari, virgolette doppie né caratteri di controllo.
pub fn plain(field: &str, v: &str, max: usize) -> Result<Option<String>, String> {
    let v = v.trim();
    if v.is_empty() {
        return Ok(None);
    }
    if v.chars().count() > max || v.chars().any(|c| c.is_control() || matches!(c, '<' | '>' | '"')) {
        return Err(format!("`{field}`: testo su una riga, fino a {max} caratteri, senza < > né virgolette doppie."));
    }
    Ok(Some(v.to_string()))
}

pub fn verification(field: &str, v: Option<&str>) -> Result<Option<String>, String> {
    match v.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(t) if TOKEN.is_match(t) => Ok(Some(t.to_string())),
        Some(_) => {
            Err(format!("`{field}`: incolla solo il codice di verifica (lettere, cifre, - e _), non il tag intero."))
        }
    }
}

pub fn twitter_site(v: Option<&str>) -> Result<Option<String>, String> {
    match v.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(t) if TWITTER.is_match(t) => Ok(Some(format!("@{}", t.trim_start_matches('@')))),
        Some(_) => Err("`twitter_site`: un account come @nomeutente (fino a 15 caratteri).".into()),
    }
}

pub fn theme_color(v: Option<&str>) -> Result<Option<String>, String> {
    match v.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(t) if COLOR.is_match(t) => Ok(Some(t.to_ascii_lowercase())),
        Some(_) => Err("`theme_color`: un colore esadecimale come #3d6b5c.".into()),
    }
}

/// `[{"name": …, "content": …}]` → coppie validate (massimo 10, nomi non riservati e senza doppioni).
pub fn extra_meta(v: &Value) -> Result<Vec<(String, String)>, String> {
    if v.is_null() {
        return Ok(Vec::new());
    }
    let list = v.as_array().ok_or("`extra_meta` deve essere un elenco di {name, content}.")?;
    if list.len() > MAX_EXTRA_META {
        return Err(format!("`extra_meta`: al massimo {MAX_EXTRA_META} meta tag."));
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for item in list {
        let name = item.get("name").and_then(Value::as_str).unwrap_or_default().trim();
        let content = item.get("content").and_then(Value::as_str).unwrap_or_default();
        if !META_NAME.is_match(name) {
            return Err(format!("`extra_meta`: nome «{name}» non valido (lettere, cifre, : . _ -)."));
        }
        if RESERVED_META.contains(&name.to_ascii_lowercase().as_str())
            || name.to_ascii_lowercase().starts_with("http-equiv")
        {
            return Err(format!("`extra_meta`: «{name}» è gestito dal portale e non si può impostare qui."));
        }
        let content = plain("extra_meta.content", content, 300)?
            .ok_or_else(|| format!("`extra_meta`: «{name}» senza contenuto."))?;
        if out.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)) {
            return Err(format!("`extra_meta`: «{name}» è ripetuto."));
        }
        out.push((name.to_string(), content));
    }
    Ok(out)
}

pub struct SeoText {
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
}

/// `{"it": {"title", "description", "keywords"}, …}` → lingue con almeno un campo compilato.
pub fn locales(v: &Value) -> Result<Vec<(String, SeoText)>, String> {
    if v.is_null() {
        return Ok(Vec::new());
    }
    let map = v.as_object().ok_or("`locales` deve essere un oggetto lingua → testi.")?;
    if map.len() > MAX_LOCALES {
        return Err(format!("`locales`: al massimo {MAX_LOCALES} lingue."));
    }
    let mut out = Vec::new();
    for (code, entry) in map {
        if !LOCALE.is_match(code) {
            return Err(format!("`locales`: lingua «{code}» non valida (due lettere minuscole)."));
        }
        let text = |k: &str, max: usize| {
            plain(&format!("locales.{code}.{k}"), entry.get(k).and_then(Value::as_str).unwrap_or_default(), max)
        };
        let t = SeoText {
            title: text("title", 120)?,
            description: text("description", 400)?,
            keywords: text("keywords", 300)?,
        };
        if t.title.is_some() || t.description.is_some() || t.keywords.is_some() {
            out.push((code.clone(), t));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// La lingua da usare: quella richiesta, poi quella predefinita, `it`, `en`, infine la prima disponibile.
pub fn pick_locale(available: &[String], asked: Option<&str>, default: &str) -> Option<String> {
    [asked, Some(default), Some("it"), Some("en")]
        .into_iter()
        .flatten()
        .find(|l| available.iter().any(|a| a == l))
        .map(str::to_string)
        .or_else(|| available.first().cloned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plain_text_and_tokens_are_validated() {
        assert_eq!(plain("f", "  Notturni — scrivere  ", 50).unwrap().as_deref(), Some("Notturni — scrivere"));
        assert_eq!(plain("f", "   ", 50).unwrap(), None);
        for bad in ["<b>x</b>", "a\"b", "a\nb", &"x".repeat(51)] {
            assert!(plain("f", bad, 50).is_err(), "{bad:?}");
        }
        assert!(verification("g", Some("abcDEF_123-xyz")).unwrap().is_some());
        assert!(verification("g", Some("")).unwrap().is_none());
        for bad in ["corto", "<meta name=\"google-site-verification\" content=\"x\">", "a b c d e f g h i j"] {
            assert!(verification("g", Some(bad)).is_err(), "{bad}");
        }
        assert_eq!(twitter_site(Some("notturni")).unwrap().as_deref(), Some("@notturni"));
        assert_eq!(twitter_site(Some("@notturni")).unwrap().as_deref(), Some("@notturni"));
        assert!(twitter_site(Some("@troppo_lungo_per_twitter")).is_err() && twitter_site(Some("a b")).is_err());
        assert_eq!(theme_color(Some("#3D6B5C")).unwrap().as_deref(), Some("#3d6b5c"));
        assert!(theme_color(Some("verde")).is_err() && theme_color(Some("#fff")).is_err());
    }

    #[test]
    fn extra_meta_rejects_reserved_names_duplicates_and_markup() {
        let ok = extra_meta(&json!([{"name": "p:domain_verify", "content": "abc123"}, {"name": "facebook-domain-verification", "content": "xyz"}])).unwrap();
        assert_eq!(ok.len(), 2);
        assert!(extra_meta(&Value::Null).unwrap().is_empty());
        for bad in [
            json!([{"name": "robots", "content": "noindex"}]),
            json!([{"name": "Viewport", "content": "x"}]),
            json!([{"name": "http-equiv-refresh", "content": "0;url=x"}]),
            json!([{"name": "a b", "content": "x"}]),
            json!([{"name": "ok-name", "content": "<script>"}]),
            json!([{"name": "ok-name", "content": ""}]),
            json!([{"name": "ok-name", "content": "1"}, {"name": "OK-NAME", "content": "2"}]),
            json!({"name": "x"}),
        ] {
            assert!(extra_meta(&bad).is_err(), "{bad}");
        }
        let many: Vec<Value> = (0..11).map(|i| json!({"name": format!("meta-{i}"), "content": "x"})).collect();
        assert!(extra_meta(&Value::Array(many)).is_err());
    }

    #[test]
    fn locales_keep_only_filled_entries_and_pick_the_best_language() {
        let l = locales(
            &json!({"it": {"title": "Notturni", "description": "Scrivere"}, "en": {}, "de": {"keywords": "blog"}}),
        )
        .unwrap();
        assert_eq!(l.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(), ["de", "it"]);
        assert!(
            locales(&json!({"ita": {"title": "x"}})).is_err() && locales(&json!({"it": {"title": "<x>"}})).is_err()
        );
        let have = vec!["de".to_string(), "it".to_string()];
        assert_eq!(pick_locale(&have, Some("de"), "it").as_deref(), Some("de"));
        assert_eq!(pick_locale(&have, Some("fr"), "en").as_deref(), Some("it"));
        assert_eq!(pick_locale(&["de".to_string()], Some("fr"), "it").as_deref(), Some("de"));
        assert_eq!(pick_locale(&[], Some("it"), "it"), None);
    }
}
