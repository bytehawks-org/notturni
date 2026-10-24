//! Regole di dominio dei post: tag e hashtag, slug e permalink, media e link citati nel
//! Markdown, nome pubblico dell'autore, effetto delle opzioni per crawler. Tutto puro.

use regex::Regex;
use std::sync::LazyLock;

// ---- tag ------------------------------------------------------------------------------------------

pub const MAX_TAGS_PER_POST: usize = 5;
const MAX_TAG_LENGTH: usize = 30;

static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$").expect("regex statica"));
static HASHTAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#([A-Za-z0-9][A-Za-z0-9_-]{0,29})").expect("regex statica"));

/// Minuscolo, senza `#`, `_` e spazi come trattini; solo `[a-z0-9]` con trattini singoli.
pub fn normalize_tag(raw: &str) -> Result<String, String> {
    let lowered = raw.trim().trim_start_matches('#').to_lowercase().replace('_', "-");
    let value = lowered.split_whitespace().collect::<Vec<_>>().join("-");
    if value.is_empty() || value.chars().count() > MAX_TAG_LENGTH || !TAG_RE.is_match(&value) {
        return Err(format!(
            "Tag non valido: {raw:?} (solo lettere minuscole, cifre e trattini singoli, max {MAX_TAG_LENGTH} caratteri)."
        ));
    }
    Ok(value)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Hashtag scritti nel testo, normalizzati e deduplicati nell'ordine in cui compaiono. Uno
/// malformato (es. troppo lungo) è ignorato: nel testo libero non è una scelta esplicita.
pub fn extract_hashtags(content: &str) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for m in HASHTAG_RE.captures_iter(content) {
        let whole = m.get(0).expect("gruppo 0");
        // `#` preceduto da un carattere di parola (es. un indirizzo, un'ancora) non è un hashtag
        if content[..whole.start()].chars().next_back().is_some_and(is_word_char) {
            continue;
        }
        if let Ok(tag) = normalize_tag(&m[1])
            && !seen.contains(&tag)
        {
            seen.push(tag);
        }
    }
    seen
}

/// Tag del campo dedicato (validati rigidamente) più gli hashtag del testo, senza duplicati e
/// con un tetto di [`MAX_TAGS_PER_POST`]: oltre, errore esplicito e non troncamento silenzioso.
/// Restituisce `(manuali normalizzati, effettivi)`.
pub fn resolve_tags(manual: &[String], content: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let mut normalized: Vec<String> = Vec::new();
    for raw in manual {
        let tag = normalize_tag(raw)?;
        if !normalized.contains(&tag) {
            normalized.push(tag);
        }
    }
    let mut effective = normalized.clone();
    for tag in extract_hashtags(content) {
        if !effective.contains(&tag) {
            effective.push(tag);
        }
    }
    if effective.len() > MAX_TAGS_PER_POST {
        return Err(format!(
            "Massimo {MAX_TAGS_PER_POST} tag per post (campo dedicato e hashtag nel testo insieme), trovati {}: {}.",
            effective.len(),
            effective.join(", ")
        ));
    }
    Ok((normalized, effective))
}

/// Come [`resolve_tags`] per dati già salvati: non fallisce, tronca al tetto.
pub fn effective_tags(manual: &[String], content: &str) -> Vec<String> {
    let mut out: Vec<String> = manual.to_vec();
    for tag in extract_hashtags(content) {
        if !out.contains(&tag) {
            out.push(tag);
        }
    }
    out.truncate(MAX_TAGS_PER_POST);
    out
}

// ---- slug e permalink ----------------------------------------------------------------------------------

/// Segmenti statici già usati dal frontend sotto `/{blog_slug}/...`: uno slug uguale a uno di
/// questi resterebbe irraggiungibile (il segmento statico vince sempre su quello dinamico).
pub const RESERVED_POST_SLUGS: &[&str] =
    &["bibliografia", "link", "media", "pagina", "pub", "search", "feed.xml", "atom.xml"];

static SLUG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._~-]{0,254}$").expect("regex statica"));

pub fn validate_post_slug(slug: &str) -> Result<(), String> {
    if !SLUG_RE.is_match(slug) {
        return Err("Lo slug del post può contenere solo lettere, cifre e i caratteri . _ ~ - (max 255).".into());
    }
    if RESERVED_POST_SLUGS.contains(&slug) {
        return Err(format!(
            "'{slug}' è riservato dal frontend (bibliografia/media/link/pagina) e non può essere usato come slug di un post."
        ));
    }
    Ok(())
}

pub fn permalink(blog_slug: &str, post_slug: &str) -> String {
    format!("/{blog_slug}/{post_slug}")
}

// ---- avviso sui contenuti -----------------------------------------------------------------------------

/// Vocabolario del modal "Aggiungi un avviso sul contenuto" (stile Bluesky).
pub const SENSITIVITY_CATEGORIES: &[&str] = &["suggestive", "nudity", "explicit", "other"];

pub fn validate_categories(categories: &[String]) -> Result<Vec<String>, String> {
    let bad: Vec<&str> =
        categories.iter().map(String::as_str).filter(|c| !SENSITIVITY_CATEGORIES.contains(c)).collect();
    if !bad.is_empty() {
        return Err(format!("Categorie non valide: {}.", bad.join(", ")));
    }
    let mut out: Vec<String> = Vec::new();
    for c in categories {
        if !out.contains(c) {
            out.push(c.clone());
        }
    }
    Ok(out)
}

// ---- media e link nel Markdown --------------------------------------------------------------------------

// Un URL può contenere un livello di parentesi bilanciate (es. Wikipedia `.../Esempio_(disambigua)`).
// Il motore `regex` ha tempo lineare: nessun rischio di backtracking catastrofico.
const URL_PATTERN: &str = r"[^\s()]+(?:\([^\s()]*\)[^\s()]*)*";

static IMAGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r#"!\[([^\]]*)\]\(\s*({URL_PATTERN})(?:\s+"([^"]*)")?\s*\)"#)).expect("regex statica")
});
static LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r#"\[([^\]]*)\]\(\s*({URL_PATTERN})(?:\s+"([^"]*)")?\s*\)"#)).expect("regex statica")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRef {
    pub position: i32,
    pub url: String,
    pub alt_text: String,
    pub categories: Vec<String>,
    /// Separato da `categories`: segnalata dalla sola automoderazione ha `categories` vuoto.
    pub is_sensitive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRef {
    pub position: i32,
    pub url: String,
    pub link_text: String,
}

/// `title` Markdown dell'immagine: assente, `sensitive` o `sensitive:cat1,cat2`.
fn sensitivity(title: Option<&str>) -> (Vec<String>, bool) {
    let Some(title) = title.filter(|t| t.starts_with("sensitive")) else {
        return (Vec::new(), false);
    };
    let cats = title
        .split_once(':')
        .map(|(_, raw)| raw)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|c| SENSITIVITY_CATEGORIES.contains(c))
        .map(str::to_string)
        .collect();
    (cats, true)
}

pub fn extract_media(content: &str) -> Vec<MediaRef> {
    IMAGE_RE
        .captures_iter(content)
        .enumerate()
        .map(|(i, m)| {
            let (categories, is_sensitive) = sensitivity(m.get(3).map(|t| t.as_str()));
            MediaRef { position: i as i32, url: m[2].to_string(), alt_text: m[1].to_string(), categories, is_sensitive }
        })
        .collect()
}

/// Esclude le immagini e i marcatori interni delle note (`#nota-n`), che non sono link citati.
/// La posizione conta i link veri e propri, anche quelli esclusi per l'ancora.
pub fn extract_links(content: &str) -> Vec<LinkRef> {
    let is_image = |start: usize| content[..start].ends_with('!');
    LINK_RE
        .captures_iter(content)
        .filter(|m| !is_image(m.get(0).expect("gruppo 0").start()))
        .enumerate()
        .filter(|(_, m)| !m[2].starts_with('#'))
        .map(|(i, m)| LinkRef { position: i as i32, url: m[2].to_string(), link_text: m[1].to_string() })
        .collect()
}

// ---- nome pubblico dell'autore ------------------------------------------------------------------------

/// Dati del profilo da cui deriva il nome personale.
#[derive(Debug, Clone, Default)]
pub struct UserNames {
    pub username: String,
    pub display_name: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    /// `username` | `full_name` | `display_name` | `verified_domain`
    pub style: String,
    /// Dominio personale verificato (modulo `custom_domains`)
    pub verified_domain: Option<String>,
}

fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// Nome scelto dall'utente per firmarsi, in base alla preferenza di profilo.
pub fn personal_display_name(user: &UserNames) -> String {
    match user.style.as_str() {
        "full_name" => {
            let full = [non_empty(&user.first_name), non_empty(&user.last_name)]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            if full.is_empty() { user.username.clone() } else { full }
        }
        "display_name" => non_empty(&user.display_name).unwrap_or(&user.username).to_string(),
        // senza un dominio verificato (rimosso o mai verificato) ci si firma con lo username
        "verified_domain" => non_empty(&user.verified_domain).unwrap_or(&user.username).to_string(),
        _ => user.username.clone(),
    }
}

/// Nome pubblico di un autore su un blog: l'alias della sua membership vince, poi il nome
/// imposto dal blog, poi la preferenza di profilo. Non c'è un valore per singolo post.
pub fn pick_display_name(membership_alias: Option<&str>, blog_default: Option<&str>, user: &UserNames) -> String {
    membership_alias
        .filter(|s| !s.is_empty())
        .or(blog_default.filter(|s| !s.is_empty()))
        .map(str::to_string)
        .unwrap_or_else(|| personal_display_name(user))
}

// ---- opzioni per crawler -------------------------------------------------------------------------------

/// Un blog escluso non si riapre con l'override di un post.
pub fn effective_flag(blog: bool, post: Option<bool>) -> bool {
    blog && post.unwrap_or(blog)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn tags_are_normalized_strictly() {
        assert_eq!(normalize_tag("  #Rust_Lang ").unwrap(), "rust-lang");
        assert_eq!(normalize_tag("due  parole").unwrap(), "due-parole");
        for bad in ["", "#", "a--b", "-a", "a-", "è", "a/b", &"x".repeat(31)] {
            assert!(normalize_tag(bad).is_err(), "{bad:?}");
        }
        assert!(normalize_tag(&"x".repeat(30)).is_ok());
    }

    #[test]
    fn hashtags_come_from_text_but_not_from_words_or_anchors() {
        let text = "Ciao #Rust e #web-dev, ma non a#b né https://x.org/p#ancora né #rust di nuovo, né #-no";
        assert_eq!(extract_hashtags(text), ["rust", "web-dev"]);
        assert_eq!(extract_hashtags("#".repeat(3).as_str()), Vec::<String>::new());
        // troppo lungo: ignorato senza far fallire il salvataggio
        assert!(extract_hashtags(&format!("#{}", "a".repeat(40))).len() <= 1);
        assert_eq!(extract_hashtags("inizio #uno\n#due\t#tre"), ["uno", "due", "tre"]);
    }

    #[test]
    fn tag_union_is_capped_with_an_explicit_error() {
        let (manual, effective) = resolve_tags(&s(&["Uno", "#due", "uno"]), "testo #tre #due").unwrap();
        assert_eq!(manual, ["uno", "due"]);
        assert_eq!(effective, ["uno", "due", "tre"]);
        let err = resolve_tags(&s(&["a", "b", "c"]), "#d #e #f").unwrap_err();
        assert!(err.contains("Massimo 5") && err.contains("a, b, c, d, e, f"), "{err}");
        assert!(resolve_tags(&s(&["non valido!"]), "").is_err());
        // dati già salvati: troncati senza errore
        assert_eq!(effective_tags(&s(&["a", "b", "c"]), "#d #e #f").len(), 5);
    }

    #[test]
    fn post_slugs_are_url_safe_and_not_reserved() {
        for ok in ["ciao-mondo", "Post_1", "v1.2", "a", "x~y"] {
            assert!(validate_post_slug(ok).is_ok(), "{ok}");
        }
        for bad in ["", "ciao mondo", "a/b", "../x", "-inizio", ".nascosto", "è", "a?b", "a#b", &"x".repeat(256)] {
            assert!(validate_post_slug(bad).is_err(), "{bad:?}");
        }
        for reserved in RESERVED_POST_SLUGS {
            assert!(validate_post_slug(reserved).unwrap_err().contains("riservato"), "{reserved}");
        }
        assert_eq!(permalink("diario", "ciao"), "/diario/ciao");
    }

    #[test]
    fn categories_are_checked_and_deduplicated() {
        assert_eq!(validate_categories(&s(&["nudity", "other", "nudity"])).unwrap(), ["nudity", "other"]);
        assert!(validate_categories(&s(&["nudity", "boh"])).unwrap_err().contains("boh"));
        assert!(validate_categories(&[]).unwrap().is_empty());
    }

    #[test]
    fn media_are_extracted_with_their_warnings() {
        let md = r#"Testo ![un gatto](https://x.org/a.png) e ![nudo](https://x.org/b.jpg "sensitive:nudity,boh,explicit")
poi ![auto](https://x.org/c.webp "sensitive") e ![](https://x.org/d.gif)
![wiki](https://it.wikipedia.org/wiki/Gatto_(animale) "titolo qualunque") ![vuota]()"#;
        let m = extract_media(md);
        assert_eq!(m.len(), 5, "{m:?}");
        assert_eq!(
            (m[0].url.as_str(), m[0].alt_text.as_str(), m[0].is_sensitive),
            ("https://x.org/a.png", "un gatto", false)
        );
        assert_eq!(m[1].categories, ["nudity", "explicit"], "le categorie sconosciute sono scartate");
        assert!(m[1].is_sensitive);
        assert!(m[2].is_sensitive && m[2].categories.is_empty(), "segnalata dalla sola automoderazione");
        assert_eq!(m[3].alt_text, "");
        assert_eq!(m[4].url, "https://it.wikipedia.org/wiki/Gatto_(animale)");
        assert!(!m[4].is_sensitive);
        assert_eq!(m.iter().map(|x| x.position).collect::<Vec<_>>(), [0, 1, 2, 3, 4]);
    }

    #[test]
    fn links_exclude_images_and_note_anchors_but_count_positions() {
        let md = "[uno](https://a.org) ![img](https://b.org/i.png) testo[^1] [nota](#nota-1) [tre](https://c.org \"titolo\") [vuoto]()";
        let l = extract_links(md);
        let urls: Vec<_> = l.iter().map(|x| x.url.as_str()).collect();
        assert_eq!(urls, ["https://a.org", "https://c.org"]);
        assert_eq!(l.iter().map(|x| x.position).collect::<Vec<_>>(), [0, 2], "l'ancora occupa la posizione 1");
        assert_eq!(l[0].link_text, "uno");
        // due link adiacenti e un'immagine subito prima di un link
        let adj = extract_links("[a](https://1.org)[b](https://2.org) ![i](https://3.org)[c](https://4.org)");
        assert_eq!(
            adj.iter().map(|x| x.url.as_str()).collect::<Vec<_>>(),
            ["https://1.org", "https://2.org", "https://4.org"]
        );
    }

    #[test]
    fn pathological_input_is_linear() {
        let t = std::time::Instant::now();
        let evil = "![](".repeat(40_000);
        assert!(extract_media(&evil).is_empty());
        assert!(extract_links(&format!("[x]({}", "a".repeat(100_000))).is_empty());
        assert!(t.elapsed().as_secs() < 2, "regex lineari");
    }

    #[test]
    fn author_name_precedence() {
        let user = UserNames {
            username: "anna".into(),
            display_name: Some("Anna B.".into()),
            first_name: Some("Anna".into()),
            last_name: Some("Bianchi".into()),
            style: "full_name".into(),
            verified_domain: Some("anna.example.org".into()),
        };
        assert_eq!(pick_display_name(Some("Firma"), Some("Blog"), &user), "Firma");
        assert_eq!(pick_display_name(None, Some("Blog"), &user), "Blog");
        assert_eq!(pick_display_name(Some(""), Some(""), &user), "Anna Bianchi");
        let signed_with_domain = UserNames { style: "verified_domain".into(), ..user.clone() };
        assert_eq!(personal_display_name(&signed_with_domain), "anna.example.org");
        let lost_domain = UserNames { verified_domain: None, ..signed_with_domain };
        assert_eq!(personal_display_name(&lost_domain), "anna", "senza dominio verificato si torna allo username");
        let mut u = user.clone();
        u.style = "display_name".into();
        assert_eq!(personal_display_name(&u), "Anna B.");
        u.display_name = None;
        assert_eq!(personal_display_name(&u), "anna", "senza nome visualizzato ricade sullo username");
        u.style = "username".into();
        assert_eq!(personal_display_name(&u), "anna");
        u.style = "full_name".into();
        u.first_name = None;
        u.last_name = Some("  ".into());
        assert_eq!(personal_display_name(&u), "anna");
    }

    #[test]
    fn a_closed_blog_cannot_be_reopened_by_a_post() {
        assert!(effective_flag(true, None));
        assert!(!effective_flag(true, Some(false)));
        assert!(effective_flag(true, Some(true)));
        assert!(!effective_flag(false, Some(true)), "l'override del post non riapre un blog escluso");
        assert!(!effective_flag(false, None));
    }
}
