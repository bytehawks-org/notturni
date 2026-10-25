//! Regole pure: validazione dell'URL e del branch, e generazione dei file da scrivere nel repository.

use chrono::{DateTime, Utc};
use regex::Regex;
use reqwest::Url;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};
use uuid::Uuid;

/// File alla radice del branch che dichiara che il contenuto è gestito da Notturni: senza, un branch già
/// esistente non viene mai sovrascritto.
pub const MARKER_FILE: &str = ".notturni.json";
const MAX_DEPTH: usize = 16;

static BRANCH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._/-]{0,199}$").expect("regex valida"));

/// L'URL del repository: solo `https` con un host, senza credenziali incorporate (il token viaggia a parte).
/// Con `allow_private` (sviluppo e test) è ammesso anche `file://`.
pub fn validate_remote(raw: &str, allow_private: bool) -> Result<Url, String> {
    let invalid = || "URL del repository non valido: serve un indirizzo https:// completo.".to_string();
    let url = Url::parse(raw.trim()).map_err(|_| invalid())?;
    if url.scheme() == "file" && allow_private {
        return Ok(url);
    }
    if url.scheme() != "https" || url.host_str().is_none_or(str::is_empty) {
        return Err(invalid());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Non inserire credenziali nell'URL: il token si imposta nel campo apposito.".into());
    }
    if url.path().trim_matches('/').is_empty() {
        return Err(invalid());
    }
    Ok(url)
}

/// Nome di branch sicuro: niente `..`, `//`, `@{`, finali `/` `.lock` `.`, né inizio con `-`.
pub fn validate_branch(name: &str) -> Result<(), String> {
    let bad = name.contains("..")
        || name.contains("//")
        || name.contains("@{")
        || name.ends_with('/')
        || name.ends_with('.')
        || name.ends_with(".lock")
        || name.split('/').any(|c| c.starts_with('.') || c.ends_with(".lock"));
    if !BRANCH.is_match(name) || bad {
        return Err("Nome del branch non valido (lettere, cifre e . _ - /).".into());
    }
    Ok(())
}

/// Una pagina da esportare.
#[derive(Debug, Clone)]
pub struct ExportPost {
    pub id: Uuid,
    pub slug: String,
    pub locale: String,
    pub title: String,
    pub content: String,
    pub status: String,
    pub published_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub author: String,
    pub tags: Vec<String>,
    pub parent_id: Option<Uuid>,
    pub position: i32,
}

/// Un componente di percorso sicuro: solo `[A-Za-z0-9_-]` (il punto non c'è, quindi mai `..`), mai vuoto.
fn component(s: &str) -> String {
    let c: String =
        s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    if c.is_empty() { "pagina".into() } else { c }
}

fn front_matter(p: &ExportPost, parent_slug: Option<&str>, tree: bool) -> String {
    let q = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
    let ts = |t: Option<DateTime<Utc>>| t.map_or("null".to_string(), |t| q(&t.to_rfc3339()));
    let mut out = String::from("---\n");
    out.push_str(&format!("title: {}\n", q(&p.title)));
    out.push_str(&format!("slug: {}\n", q(&p.slug)));
    out.push_str(&format!("locale: {}\n", q(&p.locale)));
    out.push_str(&format!("status: {}\n", q(&p.status)));
    out.push_str(&format!("published_at: {}\n", ts(p.published_at)));
    out.push_str(&format!("updated_at: {}\n", ts(Some(p.updated_at))));
    out.push_str(&format!("author: {}\n", q(&p.author)));
    out.push_str(&format!("tags: {}\n", json!(p.tags)));
    if tree {
        out.push_str(&format!("parent: {}\n", parent_slug.map_or("null".to_string(), q)));
        out.push_str(&format!("position: {}\n", p.position));
    }
    out.push_str("---\n\n");
    out
}

/// I file del repository: `(percorso, contenuto)` ordinati per percorso. Un blog ha `{lingua}/{slug}.md`;
/// wiki e docs specchiano l'albero: una pagina con sottopagine diventa `{lingua}/…/{slug}/index.md`, le
/// altre `{lingua}/…/{slug}.md`. Una pagina il cui padre non è tra quelle esportate sta alla radice.
pub fn build_files(kind: &str, space_slug: &str, posts: &[ExportPost]) -> Vec<(String, String)> {
    let tree = kind != "blog";
    let by_id: HashMap<Uuid, &ExportPost> = posts.iter().map(|p| (p.id, p)).collect();
    let parent_of = |p: &ExportPost| p.parent_id.filter(|id| by_id.get(id).is_some_and(|q| q.locale == p.locale));
    let with_children: HashSet<Uuid> = posts.iter().filter_map(parent_of).collect();

    let mut files: Vec<(String, String)> = vec![(
        MARKER_FILE.to_string(),
        format!("{}\n", json!({"generator": "notturni", "version": 1, "space": space_slug, "kind": kind})),
    )];
    let mut used: HashSet<String> = HashSet::from([MARKER_FILE.to_string()]);
    for p in posts {
        let mut dirs: Vec<String> = Vec::new();
        if tree {
            let mut seen = HashSet::from([p.id]);
            let mut cur = parent_of(p);
            while let Some(id) = cur.filter(|id| seen.insert(*id) && dirs.len() < MAX_DEPTH) {
                let parent = by_id[&id];
                dirs.push(component(&parent.slug));
                cur = parent_of(parent);
            }
            dirs.reverse();
        }
        let mut path = component(&p.locale);
        for d in &dirs {
            path = format!("{path}/{d}");
        }
        path = if tree && with_children.contains(&p.id) {
            format!("{path}/{}/index.md", component(&p.slug))
        } else {
            format!("{path}/{}.md", component(&p.slug))
        };
        if !used.insert(path.clone()) {
            path = format!("{}-{}.md", path.trim_end_matches(".md"), &p.id.to_string()[..8]);
            used.insert(path.clone());
        }
        let parent_slug = parent_of(p).map(|id| by_id[&id].slug.as_str());
        let mut body = front_matter(p, parent_slug, tree);
        body.push_str(p.content.trim_end());
        body.push('\n');
        files.push((path, body));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

/// Impronta dei file: invariata tra due giri = niente da sincronizzare.
pub fn fingerprint(files: &[(String, String)]) -> String {
    let mut h = Sha256::new();
    for (path, content) in files {
        h.update(path.len().to_le_bytes());
        h.update(path.as_bytes());
        h.update(content.len().to_le_bytes());
        h.update(content.as_bytes());
    }
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_must_be_plain_https() {
        assert!(validate_remote("https://github.com/anna/testi.git", false).is_ok());
        for bad in [
            "http://github.com/anna/testi.git",
            "git@github.com:anna/testi.git",
            "ssh://git@github.com/anna/testi.git",
            "https://anna:token@github.com/anna/testi.git",
            "https://github.com",
            "file:///tmp/repo",
            "javascript:alert(1)",
            "",
        ] {
            assert!(validate_remote(bad, false).is_err(), "{bad}");
        }
        assert!(validate_remote("file:///tmp/repo", true).is_ok(), "solo con allow_private");
    }

    #[test]
    fn branch_names_are_checked() {
        for ok in ["notturni-backup", "backup/diario", "v1.2", "a"] {
            assert!(validate_branch(ok).is_ok(), "{ok}");
        }
        for bad in
            ["", "-x", "a..b", "a//b", "a/", "a.lock", "x/.hidden", "a b", "a@{u}", "ends.", "é", "--upload-pack=x"]
        {
            assert!(validate_branch(bad).is_err(), "{bad}");
        }
    }

    fn post(slug: &str, parent: Option<&ExportPost>, locale: &str) -> ExportPost {
        ExportPost {
            id: Uuid::new_v4(),
            slug: slug.into(),
            locale: locale.into(),
            title: format!("Titolo \"{slug}\""),
            content: format!("testo di {slug}\n\n"),
            status: "published".into(),
            published_at: None,
            updated_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            author: "anna".into(),
            tags: vec!["uno".into()],
            parent_id: parent.map(|p| p.id),
            position: 0,
        }
    }

    fn paths(files: &[(String, String)]) -> Vec<&str> {
        files.iter().map(|f| f.0.as_str()).collect()
    }

    #[test]
    fn blogs_are_flat_and_trees_mirror_the_hierarchy() {
        let a = post("intro", None, "it");
        let b = post("install", Some(&a), "it");
        let c = post("deep", Some(&b), "it");
        let d = post("en-page", None, "en");
        let all = [a, b, c, d];
        let blog = build_files("blog", "diario", &all);
        assert_eq!(paths(&blog), [".notturni.json", "en/en-page.md", "it/deep.md", "it/install.md", "it/intro.md"]);
        let wiki = build_files("wiki", "guida", &all);
        assert_eq!(
            paths(&wiki),
            [
                ".notturni.json",
                "en/en-page.md",
                "it/intro/index.md",
                "it/intro/install/deep.md",
                "it/intro/install/index.md"
            ]
        );
        let deep = &wiki.iter().find(|f| f.0.ends_with("deep.md")).unwrap().1;
        assert!(deep.starts_with("---\ntitle: \"Titolo \\\"deep\\\"\"\n"), "{deep}");
        assert!(deep.contains("parent: \"install\"\n") && deep.contains("tags: [\"uno\"]\n"));
        assert!(deep.ends_with("---\n\ntesto di deep\n"));
    }

    #[test]
    fn orphans_and_hostile_slugs_stay_inside_the_tree() {
        let ghost = post("assente", None, "it");
        let child = post("figlia", Some(&ghost), "it");
        let evil = post("../../etc/passwd", None, "it");
        let files = build_files("docs", "x", &[child, evil]);
        assert_eq!(paths(&files), [".notturni.json", "it/------etc-passwd.md", "it/figlia.md"]);
        assert!(files.iter().all(|f| !f.0.contains("..") && !f.0.starts_with('/')));
    }

    #[test]
    fn fingerprint_tracks_content() {
        let a = vec![("a.md".to_string(), "x".to_string())];
        let b = vec![("a.md".to_string(), "y".to_string())];
        assert_eq!(fingerprint(&a), fingerprint(&a.clone()));
        assert_ne!(fingerprint(&a), fingerprint(&b));
        assert_ne!(fingerprint(&[("ab".into(), "c".into())]), fingerprint(&[("a".into(), "bc".into())]));
    }
}
