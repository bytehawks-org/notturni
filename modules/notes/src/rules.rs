//! Regole di dominio delle note: validazione dell'elenco di un post, normalizzazione per la
//! deduplica, stima del tipo, esportazione e importazione BibTeX minimale.

use regex::Regex;
use serde_json::Value;
use std::{collections::HashMap, sync::LazyLock};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

pub const NOTE_KINDS: &[&str] = &["book", "article", "web", "note"];

pub const MAX_NOTES_PER_POST: usize = 100;
pub const MAX_NOTE_LENGTH: usize = 2000;
pub const MAX_NOTE_IDX: i64 = 999;
pub const MAX_TITLE: usize = 300;
pub const MAX_AUTHOR: usize = 300;
pub const MAX_ISBN: usize = 32;
pub const MAX_DOI: usize = 255;
pub const MAX_PAGE: usize = 32;
pub const MAX_SOURCE: usize = 300;
pub const MAX_ISSUED: usize = 32;
pub const MAX_URL: usize = 2000;
/// Tetto assoluto all'input di un import: difesa in profondità oltre al motore a tempo lineare.
pub const MAX_BIBTEX_LENGTH: usize = 200_000;

/// Una nota di post ripulita.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Note {
    pub idx: i32,
    pub content: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub isbn: Option<String>,
    pub doi: Option<String>,
    pub page: Option<String>,
    pub kind: Option<String>,
    pub source: Option<String>,
    pub issued: Option<String>,
    pub url: Option<String>,
}

/// Spazi collassati; vuoto → `None`; oltre il limite → errore.
pub fn clean_optional(value: Option<&str>, max: usize, label: &str) -> Result<Option<String>, String> {
    let Some(v) = value else { return Ok(None) };
    let text = v.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return Ok(None);
    }
    if text.chars().count() > max {
        return Err(format!("Il campo '{label}' della nota può avere al massimo {max} caratteri."));
    }
    Ok(Some(text))
}

/// Come [`clean_optional`] ma l'URL finisce in un `href`: solo http(s), mai `javascript:`.
pub fn clean_url(value: Option<&str>) -> Result<Option<String>, String> {
    let text = clean_optional(value, MAX_URL, "URL")?;
    match text {
        Some(t) if !(t.starts_with("http://") || t.starts_with("https://")) => {
            Err("L'URL della nota deve iniziare con http:// o https://.".into())
        }
        other => Ok(other),
    }
}

fn opt_str<'a>(obj: &'a serde_json::Map<String, Value>, key: &str) -> Result<Option<&'a str>, String> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(format!("Il campo '{key}' della nota deve essere una stringa.")),
    }
}

/// Valida e normalizza l'elenco di note ricevuto (JSON): ordinato per `idx`, senza duplicati.
pub fn parse_notes(value: &Value) -> Result<Vec<Note>, String> {
    let items = value.as_array().ok_or("`notes` deve essere un elenco.")?;
    if items.len() > MAX_NOTES_PER_POST {
        return Err(format!("Massimo {MAX_NOTES_PER_POST} note per post."));
    }
    let mut out: Vec<Note> = Vec::new();
    for item in items {
        let obj = item.as_object().ok_or("Ogni nota deve essere un oggetto.")?;
        let idx = obj.get("idx").and_then(Value::as_i64).ok_or("Il numero della nota (`idx`) è obbligatorio.")?;
        if !(1..=MAX_NOTE_IDX).contains(&idx) {
            return Err(format!("Il numero della nota deve essere tra 1 e {MAX_NOTE_IDX}."));
        }
        let idx = idx as i32;
        if out.iter().any(|n| n.idx == idx) {
            return Err(format!("Numero di nota duplicato: {idx}."));
        }
        let content = opt_str(obj, "content")?.ok_or("Il testo della nota è obbligatorio.")?;
        let content = content.split_whitespace().collect::<Vec<_>>().join(" ");
        if content.is_empty() {
            return Err("Il testo della nota non può essere vuoto.".into());
        }
        if content.chars().count() > MAX_NOTE_LENGTH {
            return Err(format!("Una nota può avere al massimo {MAX_NOTE_LENGTH} caratteri."));
        }
        let kind = clean_optional(opt_str(obj, "kind")?, 10, "tipo")?;
        if let Some(k) = &kind
            && !NOTE_KINDS.contains(&k.as_str())
        {
            return Err(format!("Tipo di nota non valido: usare uno tra {}.", NOTE_KINDS.join(", ")));
        }
        out.push(Note {
            idx,
            content,
            title: clean_optional(opt_str(obj, "title")?, MAX_TITLE, "titolo")?,
            author: clean_optional(opt_str(obj, "author")?, MAX_AUTHOR, "autore")?,
            isbn: clean_optional(opt_str(obj, "isbn")?, MAX_ISBN, "ISBN")?,
            doi: clean_optional(opt_str(obj, "doi")?, MAX_DOI, "DOI")?,
            page: clean_optional(opt_str(obj, "page")?, MAX_PAGE, "pagina")?,
            kind,
            source: clean_optional(opt_str(obj, "source")?, MAX_SOURCE, "editore/rivista/sito")?,
            issued: clean_optional(opt_str(obj, "issued")?, MAX_ISSUED, "anno/data")?,
            url: clean_url(opt_str(obj, "url")?)?,
        });
    }
    out.sort_by_key(|n| n.idx);
    Ok(out)
}

// ---- normalizzazione e stima del tipo ---------------------------------------------------------------------------------

static DOI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)10\.\d{4,9}/[^\s"<>]+"#).expect("regex valida"));
static URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)https?://[^\s)\]>"]+"#).expect("regex valida"));
static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(?:1[5-9]\d{2}|20\d{2})\b").expect("regex valida"));
static NON_ALNUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").expect("regex valida"));

/// Chiave di deduplica: minuscolo, solo ASCII alfanumerico, spazi collassati, max 600.
/// Apostrofi e virgolette non separano parole ("Devil's" ≡ "Devils").
pub fn normalize_note(content: &str) -> String {
    let ascii: String = content.nfkd().filter(char::is_ascii).filter(|c| !matches!(c, '\'' | '`' | '"')).collect();
    let spaced = NON_ALNUM.replace_all(&ascii.to_lowercase(), " ").trim().to_string();
    spaced.chars().take(600).collect()
}

pub fn extract_url(content: &str) -> Option<String> {
    if let Some(m) = DOI.find(content) {
        return Some(format!("https://doi.org/{}", m.as_str().trim_end_matches('.')));
    }
    URL.find(content).map(|m| m.as_str().trim_end_matches(['.', ',', ';']).to_string())
}

fn is_doi_url(url: &str) -> bool {
    let host = url.split("://").nth(1).and_then(|r| r.split(['/', '?', '#']).next()).unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or("").split(':').next().unwrap_or("").to_lowercase();
    host == "doi.org" || host.ends_with(".doi.org")
}

/// Stima grossolana, sempre correggibile dalla libreria.
pub fn guess_kind(content: &str, url: Option<&str>) -> &'static str {
    match url {
        Some(u) if is_doi_url(u) => "article",
        Some(_) => "web",
        None if YEAR.is_match(content) && content.contains(',') => "book",
        None => "note",
    }
}

/// Due note sono "possibili duplicati" se coincidono sui primi 30 caratteri normalizzati.
pub fn duplicate_groups(notes: &[(Uuid, String)]) -> HashMap<Uuid, Vec<Uuid>> {
    let mut groups: HashMap<String, Vec<Uuid>> = HashMap::new();
    for (id, normalized) in notes {
        let key: String = normalized.chars().take(30).collect();
        if !key.is_empty() {
            groups.entry(key).or_default().push(*id);
        }
    }
    let mut out = HashMap::new();
    for members in groups.values().filter(|m| m.len() > 1) {
        for id in members {
            out.insert(*id, members.iter().copied().filter(|m| m != id).collect());
        }
    }
    out
}

/// Chiave di raggruppamento delle bibliografie: testo con spazi collassati, case-insensitive.
pub fn group_key(content: &str) -> String {
    content.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

// ---- BibTeX ------------------------------------------------------------------------------------------------------------

/// Una nota di libreria nei campi che servono all'esportazione.
#[derive(Debug, Clone, Default)]
pub struct BibNote {
    pub content: String,
    pub kind: String,
    pub url: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub source: Option<String>,
    pub issued: Option<String>,
    pub isbn: Option<String>,
    pub doi: Option<String>,
    pub page: Option<String>,
}

fn bibtex_escape(text: &str) -> String {
    text.replace('{', "\\{").replace('}', "\\}")
}

/// Una voce per nota (`@book`/`@article`/`@misc`) con `note` sempre presente: il testo resta la
/// fonte di verità della bibliografia pubblica, i campi strutturati aiutano i gestori esterni.
pub fn to_bibtex(notes: &[BibNote]) -> String {
    let mut entries = Vec::new();
    for (i, n) in notes.iter().enumerate() {
        let kind = match n.kind.as_str() {
            "book" => "book",
            "article" => "article",
            _ => "misc",
        };
        let year = n.issued.clone().or_else(|| YEAR.find(&n.content).map(|m| m.as_str().to_string()));
        let mut fields: Vec<String> = Vec::new();
        let mut add = |name: &str, v: &Option<String>, escape: bool| {
            if let Some(v) = v {
                fields.push(format!("  {name} = {{{}}}", if escape { bibtex_escape(v) } else { v.clone() }));
            }
        };
        add("title", &n.title, true);
        add("author", &n.author, true);
        let source_field = match n.kind.as_str() {
            "book" => "publisher",
            "article" => "journal",
            _ => "organization",
        };
        add(source_field, &n.source, true);
        add("year", &year, false);
        add("isbn", &n.isbn, false);
        add("doi", &n.doi, false);
        add("pages", &n.page, true);
        add("url", &n.url, false);
        fields.push(format!("  note = {{{}}}", bibtex_escape(&n.content)));
        entries.push(format!("@{kind}{{notturni{},\n{}\n}}", i + 1, fields.join(",\n")));
    }
    if entries.is_empty() { String::new() } else { entries.join("\n\n") + "\n" }
}

static ENTRY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)@(\w+)\s*\{\s*([^,\s]*)\s*,(.*?)\n\}").expect("regex valida"));
static FIELD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)(\w+)\s*=\s*(\{(?:[^{}]|\{[^{}]*\})*\}|"[^"]*")"#).expect("regex valida"));

/// Voce importata, con i campi non ancora ripuliti.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Imported {
    pub content: String,
    pub kind: String,
    pub url: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub source: Option<String>,
    pub issued: Option<String>,
    pub isbn: Option<String>,
    pub doi: Option<String>,
    pub page: Option<String>,
}

/// Parser minimale: per ogni voce ricava i campi strutturati, il tipo e il testo (`note` se
/// presente, altrimenti "Autore. Titolo. Editore. Anno." sintetizzato dai campi).
pub fn parse_bibtex(text: &str) -> Result<Vec<Imported>, String> {
    if text.chars().count() > MAX_BIBTEX_LENGTH {
        return Err(format!("Testo BibTeX troppo lungo (massimo {MAX_BIBTEX_LENGTH} caratteri)."));
    }
    let mut out = Vec::new();
    for m in ENTRY.captures_iter(text) {
        let entry_type = m[1].to_lowercase();
        let fields: HashMap<String, String> = FIELD
            .captures_iter(&m[3])
            .map(|f| (f[1].to_lowercase(), f[2].trim_matches(['{', '}', '"']).trim().to_string()))
            .collect();
        let get = |k: &str| fields.get(k).filter(|v| !v.is_empty()).cloned();
        let url = get("url").or_else(|| get("doi").map(|d| format!("https://doi.org/{d}")));
        let source = get("publisher").or_else(|| get("journal")).or_else(|| get("organization"));
        let content = match get("note") {
            Some(n) => n,
            None => {
                let parts: Vec<String> =
                    [get("author"), get("title"), source.clone(), get("year")].into_iter().flatten().collect();
                if parts.is_empty() { String::new() } else { parts.join(". ") + "." }
            }
        };
        if content.is_empty() {
            continue;
        }
        let kind = match entry_type.as_str() {
            "book" => "book",
            "article" | "inproceedings" => "article",
            _ if url.is_some() => "web",
            _ => "note",
        };
        out.push(Imported {
            content: content.chars().take(2000).collect(),
            kind: kind.into(),
            url,
            title: get("title"),
            author: get("author"),
            source,
            issued: get("year").or_else(|| get("date")),
            isbn: get("isbn"),
            doi: get("doi"),
            page: get("pages").or_else(|| get("page")),
        });
    }
    Ok(out)
}

/// Come [`clean_optional`] ma tronca invece di rifiutare: in un lotto un campo fuori limite non
/// deve far fallire tutto.
pub fn truncate_clean(value: Option<&str>, max: usize) -> Option<String> {
    let text = value?.split_whitespace().collect::<Vec<_>>().join(" ");
    let text: String = text.chars().take(max).collect();
    (!text.is_empty()).then_some(text)
}

/// Un URL fuori dalla policy http(s) si scarta (mai si tronca).
pub fn truncate_clean_url(value: Option<&str>) -> Option<String> {
    truncate_clean(value, MAX_URL).filter(|u| u.starts_with("http://") || u.starts_with("https://"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn notes_are_validated_cleaned_and_sorted() {
        let n = parse_notes(&json!([
            {"idx": 2, "content": "  Seconda   nota ", "kind": "book", "url": "https://x.org", "title": " T  "},
            {"idx": 1, "content": "Prima", "isbn": "   "},
        ]))
        .unwrap();
        assert_eq!(n.iter().map(|n| n.idx).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(
            (n[1].content.as_str(), n[1].title.as_deref(), n[0].isbn.clone()),
            ("Seconda nota", Some("T"), None)
        );
        for (bad, expect) in [
            (json!([{"idx": 0, "content": "a"}]), "tra 1 e 999"),
            (json!([{"idx": 1000, "content": "a"}]), "tra 1 e 999"),
            (json!([{"idx": 1, "content": "a"}, {"idx": 1, "content": "b"}]), "duplicato"),
            (json!([{"idx": 1, "content": "   "}]), "vuoto"),
            (json!([{"idx": 1, "content": "a", "kind": "film"}]), "Tipo di nota non valido"),
            (json!([{"idx": 1, "content": "a", "url": "javascript:alert(1)"}]), "http://"),
            (json!([{"idx": 1, "content": "a", "title": 4}]), "stringa"),
            (json!([{"idx": 1, "content": "a".repeat(2001)}]), "al massimo 2000"),
            (json!({"idx": 1}), "elenco"),
        ] {
            let e = parse_notes(&bad).unwrap_err();
            assert!(e.contains(expect), "{e} / {expect}");
        }
        let many: Vec<Value> = (1..=101).map(|i| json!({"idx": i, "content": "x"})).collect();
        assert!(parse_notes(&Value::Array(many)).unwrap_err().contains("Massimo 100"));
    }

    #[test]
    fn normalization_ignores_case_accents_and_apostrophes() {
        assert_eq!(normalize_note("Zerby, C. The Devil's Details. 2002."), "zerby c the devils details 2002");
        assert_eq!(normalize_note("Caffè «È» PERCHÉ"), "caffe e perche");
        assert_eq!(normalize_note(&"a".repeat(700)).len(), 600);
        assert_eq!(normalize_note("!!!"), "");
    }

    #[test]
    fn url_and_kind_are_guessed() {
        assert_eq!(extract_url("vedi 10.1000/xyz123. fine").as_deref(), Some("https://doi.org/10.1000/xyz123"));
        assert_eq!(extract_url("sito https://example.com/a, qui").as_deref(), Some("https://example.com/a"));
        assert_eq!(extract_url("niente"), None);
        assert_eq!(guess_kind("x", Some("https://doi.org/10.1/a")), "article");
        assert_eq!(guess_kind("x", Some("https://evil.com/?x=doi.org")), "web");
        assert_eq!(guess_kind("x", Some("https://notdoi.org/a")), "web");
        assert_eq!(guess_kind("Zerby, C. Titolo. 2002.", None), "book");
        assert_eq!(guess_kind("una nota", None), "note");
    }

    #[test]
    fn duplicates_share_the_first_thirty_normalized_characters() {
        let (a, b, c) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let long = "zerby c the devils details publisher";
        let g = duplicate_groups(&[
            (a, long.into()),
            (b, format!("{long} 2002")),
            (c, "tutt altro".into()),
            (Uuid::new_v4(), String::new()),
        ]);
        assert_eq!(g.get(&a), Some(&vec![b]));
        assert_eq!(g.get(&b), Some(&vec![a]));
        assert!(!g.contains_key(&c));
    }

    #[test]
    fn bibtex_round_trips() {
        let notes = vec![
            BibNote {
                content: "Zerby, C. The Devil's Details. 2002.".into(),
                kind: "book".into(),
                title: Some("The {Devil's} Details".into()),
                author: Some("Zerby".into()),
                source: Some("Touchstone".into()),
                isbn: Some("123".into()),
                ..Default::default()
            },
            BibNote {
                content: "Sito".into(),
                kind: "web".into(),
                url: Some("https://x.org".into()),
                ..Default::default()
            },
        ];
        let text = to_bibtex(&notes);
        assert!(
            text.contains("@book{notturni1,")
                && text.contains("publisher = {Touchstone}")
                && text.contains("year = {2002}")
        );
        assert!(text.contains("title = {The \\{Devil's\\} Details}") && text.contains("@misc{notturni2,"));
        let back = parse_bibtex(&text).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(
            (back[0].kind.as_str(), back[0].isbn.as_deref(), back[0].source.as_deref()),
            ("book", Some("123"), Some("Touchstone"))
        );
        assert_eq!((back[1].kind.as_str(), back[1].url.as_deref()), ("web", Some("https://x.org")));
        assert_eq!(to_bibtex(&[]), "");
    }

    #[test]
    fn bibtex_without_note_synthesizes_text_and_pathological_input_is_bounded() {
        let b = parse_bibtex("@article{k,\n  author = {Rossi},\n  title = {Titolo},\n  journal = {Rivista},\n  year = {2020},\n  doi = {10.1/x}\n}").unwrap();
        assert_eq!(
            (b[0].content.as_str(), b[0].kind.as_str(), b[0].url.as_deref()),
            ("Rossi. Titolo. Rivista. 2020.", "article", Some("https://doi.org/10.1/x"))
        );
        assert!(parse_bibtex(&"x".repeat(MAX_BIBTEX_LENGTH + 1)).is_err());
        let started = std::time::Instant::now();
        let _ = parse_bibtex(&format!("@a{}", " ".repeat(150_000)));
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert_eq!(parse_bibtex("niente").unwrap(), vec![]);
    }

    #[test]
    fn imports_truncate_and_drop_unsafe_urls() {
        assert_eq!(truncate_clean(Some("  a   b "), 3).as_deref(), Some("a b"));
        assert_eq!(truncate_clean(Some("abcdef"), 3).as_deref(), Some("abc"));
        assert_eq!(truncate_clean_url(Some("javascript:alert(1)")), None);
        assert_eq!(truncate_clean_url(Some("https://x.org")).as_deref(), Some("https://x.org"));
    }
}
