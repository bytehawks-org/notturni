//! Regole di dominio dei blog: slug, lingue, limiti di testo, configurazione di
//! presentazione. Tutto puro (nessun I/O), testato qui.

use regex::Regex;
use serde_json::{Map, Value};
use std::sync::LazyLock;

pub const MIN_SLUG_LENGTH: usize = 4;
pub const MAX_SUBTITLE_LENGTH: usize = 64;
pub const MAX_DESCRIPTION_LENGTH: usize = 256;
/// Limite del footer per blog (stesso del footer di piattaforma).
pub const MAX_FOOTER_MARKDOWN_LENGTH: usize = 5000;

/// Nomi che la piattaforma tiene per sé (sottodomini e servizi); i nomi di 3 caratteri o
/// meno sono riservati a prescindere. Il Super Admin può aggiungerne altri.
pub const RESERVED_BLOG_SLUGS: &[&str] =
    &["blog", "www", "mail", "journal", "api", "admin", "monitor", "stats", "status", "search", "users", "s3"];

static SLUG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$").expect("regex statica"));
static LOCALE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z]{2}$").expect("regex statica"));

pub fn validate_slug(slug: &str, extra_reserved: &[String]) -> Result<(), String> {
    if slug.chars().count() < MIN_SLUG_LENGTH {
        return Err(format!("Lo slug del blog deve avere almeno {MIN_SLUG_LENGTH} caratteri."));
    }
    if !SLUG_RE.is_match(slug) {
        return Err("Lo slug può contenere solo lettere minuscole, numeri e trattini.".into());
    }
    if RESERVED_BLOG_SLUGS.contains(&slug) || extra_reserved.iter().any(|r| r == slug) {
        return Err(format!("'{slug}' è un nome riservato alla piattaforma."));
    }
    Ok(())
}

pub fn validate_locale(locale: &str) -> Result<(), String> {
    if LOCALE_RE.is_match(locale) {
        Ok(())
    } else {
        Err("Il codice lingua deve essere ISO 639-1 di 2 lettere minuscole (es. it, en, de).".into())
    }
}

pub fn validate_subtitle(s: &str) -> Result<(), String> {
    if s.chars().count() > MAX_SUBTITLE_LENGTH {
        return Err(format!("Il sottotitolo può avere al massimo {MAX_SUBTITLE_LENGTH} caratteri."));
    }
    Ok(())
}

pub fn validate_description(s: &str) -> Result<(), String> {
    if s.chars().count() > MAX_DESCRIPTION_LENGTH {
        return Err(format!("La descrizione può avere al massimo {MAX_DESCRIPTION_LENGTH} caratteri."));
    }
    Ok(())
}

/// Lingue secondarie: validate, senza duplicati e senza la lingua principale.
pub fn normalize_extra_locales(extra: &[String], default_locale: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for code in extra {
        validate_locale(code)?;
        if code != default_locale && !out.contains(code) {
            out.push(code.clone());
        }
    }
    Ok(out)
}

// ---- configurazione di presentazione -------------------------------------------------------------

pub const MAX_PALETTE_COLORS: usize = 5;
pub const MAX_FONTS: usize = 3;
/// Sopra questa saturazione HLS un colore è troppo acceso per una palette "calma".
pub const MAX_SATURATION: f64 = 0.9;

const SERIF_FONTS: &[&str] = &["Lora", "Merriweather", "Playfair Display", "Source Serif 4", "Crimson Pro"];
const SANS_SERIF_FONTS: &[&str] = &["Inter", "Nunito Sans", "Work Sans", "Source Sans 3", "Karla"];
const MONOSPACE_FONTS: &[&str] = &["JetBrains Mono", "Fira Code", "IBM Plex Mono", "Source Code Pro", "Space Mono"];
/// Chiavi di `typography` che sono stringhe ma non font.
const NON_FONT_TYPOGRAPHY_KEYS: &[&str] = &["body_size", "measure"];

/// Configurazione applicata a un blog che non ne ha salvata una (identica allo shell di base).
pub fn default_config() -> Value {
    serde_json::json!({
        "palette": {
            "background": "#fbf9f6",
            "foreground": "#2b2a28",
            "primary": "#3e6259",
            "muted": "#a8a29a",
            "border": "#e7e2da",
        },
        "typography": { "heading_font": "Lora", "body_font": "Source Sans 3", "monospace_font": "JetBrains Mono" },
        "layout": "standard",
    })
}

/// Saturazione HLS di un colore `#rgb`/`#rrggbb`; `None` se non è in quella forma.
fn hex_saturation(value: &str) -> Option<f64> {
    let hex = value.strip_prefix('#')?;
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |s: &str| u8::from_str_radix(s, 16).ok().map(|v| f64::from(v) / 255.0);
    let (r, g, b) = match hex.len() {
        3 => (channel(&hex[0..1].repeat(2))?, channel(&hex[1..2].repeat(2))?, channel(&hex[2..3].repeat(2))?),
        6 => (channel(&hex[0..2])?, channel(&hex[2..4])?, channel(&hex[4..6])?),
        _ => return None,
    };
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    if max == min {
        return Some(0.0);
    }
    let l = (max + min) / 2.0;
    Some(if l <= 0.5 { (max - min) / (max + min) } else { (max - min) / (2.0 - max - min) })
}

fn validate_palette(key: &str, value: &Value) -> Result<(), String> {
    let Value::Object(palette) = value else {
        return Err(format!("{key} deve essere un oggetto {{nome: colore}}."));
    };
    if palette.len() > MAX_PALETTE_COLORS {
        return Err(format!("La palette può avere al massimo {MAX_PALETTE_COLORS} colori."));
    }
    for (name, color) in palette {
        let Some(color) = color.as_str() else { continue };
        if let Some(s) = hex_saturation(color)
            && s > MAX_SATURATION
        {
            return Err(format!(
                "Il colore '{name}' ({color}) è troppo acceso per una palette calma: saturazione massima consentita {}%.",
                (MAX_SATURATION * 100.0) as u32
            ));
        }
    }
    Ok(())
}

fn validate_typography(value: &Value) -> Result<(), String> {
    let Value::Object(t) = value else {
        return Err("typography deve essere un oggetto.".into());
    };
    let fonts: std::collections::BTreeSet<&str> = t
        .iter()
        .filter(|(k, _)| !NON_FONT_TYPOGRAPHY_KEYS.contains(&k.as_str()))
        .filter_map(|(_, v)| v.as_str())
        .collect();
    if fonts.len() > MAX_FONTS {
        return Err(format!("Al massimo {MAX_FONTS} font distinti."));
    }
    let check = |key: &str, allowed: &[&str], kind: &str| -> Result<(), String> {
        match t.get(key).and_then(Value::as_str) {
            Some(f) if !allowed.contains(&f) => {
                let mut sorted = allowed.to_vec();
                sorted.sort_unstable();
                Err(format!("{key} deve essere un font {kind} tra: {}.", sorted.join(", ")))
            }
            _ => Ok(()),
        }
    };
    check("heading_font", SERIF_FONTS, "serif")?;
    check("body_font", SANS_SERIF_FONTS, "sans-serif")?;
    check("monospace_font", MONOSPACE_FONTS, "a spaziatura fissa")
}

fn validate_footer(value: &Value) -> Result<(), String> {
    let Value::Object(f) = value else {
        return Err("footer deve essere un oggetto.".into());
    };
    let mut unknown: Vec<&str> =
        f.keys().map(String::as_str).filter(|k| !matches!(*k, "column1" | "column2")).collect();
    if !unknown.is_empty() {
        unknown.sort_unstable();
        return Err(format!("Chiavi non valide in footer: {}.", unknown.join(", ")));
    }
    for (key, v) in f {
        match v {
            Value::Null => {}
            Value::String(s) if s.chars().count() > MAX_FOOTER_MARKDOWN_LENGTH => {
                return Err(format!("footer.{key} supera i {MAX_FOOTER_MARKDOWN_LENGTH} caratteri."));
            }
            Value::String(_) => {}
            _ => return Err(format!("footer.{key} deve essere una stringa Markdown.")),
        }
    }
    Ok(())
}

/// Vincoli estetici del prodotto (max 5 colori, max 3 font, titoli serif/corpo sans, palette
/// non aggressiva); il resto della struttura resta libero.
pub fn validate_config(config: &Map<String, Value>) -> Result<(), String> {
    for key in ["palette", "palette_dark"] {
        if let Some(p) = config.get(key).filter(|v| !v.is_null()) {
            validate_palette(key, p)?;
        }
    }
    if let Some(t) = config.get("typography").filter(|v| !v.is_null()) {
        validate_typography(t)?;
    }
    if let Some(f) = config.get("footer").filter(|v| !v.is_null()) {
        validate_footer(f)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn slug_rules() {
        assert!(validate_slug("il-mio-blog", &[]).is_ok());
        assert!(validate_slug("abc", &[]).unwrap_err().contains("almeno 4"));
        for bad in ["Blog-Mio", "-abcd", "abcd-", "ab--cd", "ab_cd", "àbcd", "ab cd"] {
            assert!(validate_slug(bad, &[]).is_err(), "{bad}");
        }
        assert!(validate_slug("admin", &[]).unwrap_err().contains("riservato"));
        assert!(validate_slug("status", &[]).is_err());
        assert!(validate_slug("nuovo-nome", &["nuovo-nome".to_string()]).is_err(), "elenco del Super Admin");
        assert!(validate_slug("nuovo-nome", &[]).is_ok());
    }

    #[test]
    fn text_limits_count_characters_not_bytes() {
        assert!(validate_subtitle(&"è".repeat(64)).is_ok());
        assert!(validate_subtitle(&"è".repeat(65)).is_err());
        assert!(validate_description(&"x".repeat(256)).is_ok());
        assert!(validate_description(&"x".repeat(257)).is_err());
    }

    #[test]
    fn extra_locales_are_deduplicated_and_exclude_the_main_one() {
        let out = normalize_extra_locales(&["en".into(), "it".into(), "en".into(), "de".into()], "it").unwrap();
        assert_eq!(out, vec!["en", "de"]);
        assert!(normalize_extra_locales(&["EN".into()], "it").is_err());
        assert!(normalize_extra_locales(&["eng".into()], "it").is_err());
    }

    #[test]
    fn saturation_matches_hls() {
        assert_eq!(hex_saturation("#808080"), Some(0.0));
        assert!((hex_saturation("#ff0000").unwrap() - 1.0).abs() < 1e-9);
        assert!((hex_saturation("#f00").unwrap() - 1.0).abs() < 1e-9);
        // colorsys.rgb_to_hls(0x3e/255, 0x62/255, 0x59/255)[2] ≈ 0.225
        assert!((hex_saturation("#3e6259").unwrap() - 0.2250).abs() < 1e-3);
        assert_eq!(hex_saturation("rosso"), None);
        assert_eq!(hex_saturation("#12345"), None);
    }

    fn cfg(v: Value) -> Result<(), String> {
        validate_config(v.as_object().unwrap())
    }

    #[test]
    fn default_config_is_valid() {
        assert!(validate_config(default_config().as_object().unwrap()).is_ok());
    }

    #[test]
    fn palette_limits() {
        assert!(
            cfg(json!({"palette": {"a":"#808080","b":"#808080","c":"#808080","d":"#808080","e":"#808080"}})).is_ok()
        );
        let err = cfg(
            json!({"palette": {"a":"#808080","b":"#808080","c":"#808080","d":"#808080","e":"#808080","f":"#808080"}}),
        )
        .unwrap_err();
        assert!(err.contains("al massimo 5"));
        assert!(cfg(json!({"palette": {"primary": "#ff0000"}})).unwrap_err().contains("troppo acceso"));
        assert!(cfg(json!({"palette_dark": {"primary": "#00ff00"}})).is_err());
        assert!(cfg(json!({"palette": ["#fff"]})).unwrap_err().contains("oggetto"));
        // valori non-colore e colori non esadecimali non sono vincolati
        assert!(cfg(json!({"palette": {"primary": "rebeccapurple", "x": 3}})).is_ok());
    }

    #[test]
    fn typography_limits() {
        assert!(cfg(json!({"typography": {"heading_font": "Lora", "body_font": "Inter", "monospace_font": "Fira Code", "body_size": "18"}})).is_ok());
        assert!(cfg(json!({"typography": {"heading_font": "Inter"}})).unwrap_err().contains("serif"));
        assert!(cfg(json!({"typography": {"body_font": "Lora"}})).unwrap_err().contains("sans-serif"));
        assert!(cfg(json!({"typography": {"monospace_font": "Lora"}})).is_err());
        assert!(cfg(json!({"typography": {"heading_font": "Lora", "body_font": "Inter", "monospace_font": "Fira Code", "extra": "Altro"}}))
            .unwrap_err()
            .contains("3 font"));
    }

    #[test]
    fn footer_overrides_only_columns_one_and_two() {
        assert!(cfg(json!({"footer": {"column1": "x", "column2": null}})).is_ok());
        assert!(cfg(json!({"footer": {"column3": "x"}})).unwrap_err().contains("column3"));
        assert!(cfg(json!({"footer": {"column1": 3}})).is_err());
        assert!(cfg(json!({"footer": {"column1": "x".repeat(5001)}})).unwrap_err().contains("5000"));
        assert!(cfg(json!({"layout": "magazine", "qualsiasi": [1, 2]})).is_ok(), "chiavi libere");
    }
}
