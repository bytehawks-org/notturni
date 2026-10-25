//! Regole pure del modulo: link interni, differenze tra revisioni, validazione dell'albero.

use regex::Regex;
use std::{collections::HashMap, sync::LazyLock};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

/// Profondità massima dell'albero (le radici sono a 0).
pub const MAX_DEPTH: usize = 8;
/// Revisioni conservate per post: le più vecchie oltre il limite vengono scartate.
pub const MAX_REVISIONS: i32 = 200;
/// Link interni estratti da un solo contenuto.
pub const MAX_WIKILINKS: usize = 200;
/// Prodotto massimo delle righe di due revisioni per il calcolo fine delle differenze.
const MAX_DIFF_CELLS: usize = 4_000_000;

// `[[target]]` o `[[target|testo]]`. L'editor WYSIWYG può salvare le parentesi con il backslash
// (`\[\[x\]\]`): sono accettate entrambe le forme.
static WIKILINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\\?\[){2}([^\[\]\n|]{1,200}?)(?:\|([^\[\]\n]{0,200}?))?(?:\\?\]){2}").expect("regex valida")
});
static FENCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?ms)^(```|~~~).*?^(?:```|~~~)[ \t]*$").expect("regex valida"));
static INLINE_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`[^`\n]*`").expect("regex valida"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLink {
    pub target: String,
    pub label: Option<String>,
}

/// Normalizza il bersaglio di un link come uno slug: minuscolo, senza accenti, solo `[a-z0-9]`
/// separati da un trattino. Vuoto se non resta nulla.
pub fn slugify(raw: &str) -> String {
    let mut out = String::new();
    for c in raw.replace('\\', "").nfkd().filter(|c| !('\u{300}'..='\u{36f}').contains(c)) {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').chars().take(255).collect()
}

/// I link interni del Markdown, in ordine, ignorando blocchi e frammenti di codice. Al massimo
/// [`MAX_WIKILINKS`]; i bersagli che non producono uno slug sono scartati.
pub fn extract_wikilinks(markdown: &str) -> Vec<WikiLink> {
    let no_fences = FENCE.replace_all(markdown, "");
    let text = INLINE_CODE.replace_all(&no_fences, "");
    WIKILINK
        .captures_iter(&text)
        .filter_map(|c| {
            let target = slugify(&c[1]);
            if target.is_empty() {
                return None;
            }
            let label = c.get(2).map(|m| m.as_str().replace('\\', "").trim().to_string()).filter(|l| !l.is_empty());
            Some(WikiLink { target, label })
        })
        .take(MAX_WIKILINKS)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub op: &'static str,
    pub text: String,
}

fn push_line(hunks: &mut Vec<Hunk>, op: &'static str, line: &str) {
    match hunks.last_mut() {
        Some(h) if h.op == op => {
            h.text.push('\n');
            h.text.push_str(line);
        }
        _ => hunks.push(Hunk { op, text: line.to_string() }),
    }
}

/// Differenze per righe tra `a` e `b` (sottosequenza comune più lunga, dopo aver tolto prefisso e
/// suffisso uguali). Oltre [`MAX_DIFF_CELLS`] il calcolo fine non si fa: il blocco centrale
/// risulta tutto cancellato e tutto inserito.
pub fn line_diff(a: &str, b: &str) -> Vec<Hunk> {
    let a: Vec<&str> = a.split('\n').collect();
    let b: Vec<&str> = b.split('\n').collect();
    let pre = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suf = a[pre..].iter().rev().zip(b[pre..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (ma, mb) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);

    let mut hunks = Vec::new();
    for l in &a[..pre] {
        push_line(&mut hunks, "equal", l);
    }
    if ma.len().saturating_mul(mb.len()) > MAX_DIFF_CELLS {
        ma.iter().for_each(|l| push_line(&mut hunks, "delete", l));
        mb.iter().for_each(|l| push_line(&mut hunks, "insert", l));
    } else {
        let (n, m) = (ma.len(), mb.len());
        // lcs[i][j] = lunghezza della sottosequenza comune di ma[i..] e mb[j..]
        let mut lcs = vec![vec![0u32; m + 1]; n + 1];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                lcs[i][j] = if ma[i] == mb[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && ma[i] == mb[j] {
                push_line(&mut hunks, "equal", ma[i]);
                i += 1;
                j += 1;
            } else if i < n && (j == m || lcs[i + 1][j] >= lcs[i][j + 1]) {
                push_line(&mut hunks, "delete", ma[i]);
                i += 1;
            } else {
                push_line(&mut hunks, "insert", mb[j]);
                j += 1;
            }
        }
    }
    for l in &a[a.len() - suf..] {
        push_line(&mut hunks, "equal", l);
    }
    hunks
}

/// Controlla che nella mappa `post → padre` nessun elenco di `ids` finisca in un ciclo, superi
/// [`MAX_DEPTH`] o punti a un padre che non appartiene allo Spazio.
pub fn check_parents(parents: &HashMap<Uuid, Option<Uuid>>, ids: &[Uuid]) -> Result<(), String> {
    for start in ids {
        let mut current = *start;
        let mut depth = 0;
        while let Some(parent) = parents.get(&current).copied().flatten() {
            if !parents.contains_key(&parent) {
                return Err("La pagina padre non appartiene a questo spazio.".into());
            }
            if parent == *start {
                return Err("Una pagina non può stare sotto sé stessa o sotto una sua sottopagina.".into());
            }
            depth += 1;
            if depth > MAX_DEPTH {
                return Err(format!("L'albero può avere al massimo {MAX_DEPTH} livelli di sottopagine."));
            }
            current = parent;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_strips_accents_and_punctuation() {
        assert_eq!(slugify("  Città  di Mare! "), "citta-di-mare");
        assert_eq!(slugify("a\\_b"), "a-b");
        assert_eq!(slugify("***"), "");
    }

    #[test]
    fn wikilinks_are_extracted_outside_code() {
        let md = "Vedi [[Guida rapida]] e [[api|la API]].\n```\n[[nel-codice]]\n```\nanche `[[inline]]` no, \\[\\[escape\\]\\] sì.";
        let links = extract_wikilinks(md);
        assert_eq!(
            links,
            vec![
                WikiLink { target: "guida-rapida".into(), label: None },
                WikiLink { target: "api".into(), label: Some("la API".into()) },
                WikiLink { target: "escape".into(), label: None },
            ]
        );
        assert!(extract_wikilinks("[[ ]] [[]] [[!!]]").is_empty());
    }

    fn texts(h: &[Hunk]) -> Vec<(&str, &str)> {
        h.iter().map(|h| (h.op, h.text.as_str())).collect()
    }

    #[test]
    fn diff_groups_consecutive_lines() {
        let h = line_diff("a\nb\nc\nd", "a\nB\nX\nd");
        assert_eq!(texts(&h), [("equal", "a"), ("delete", "b\nc"), ("insert", "B\nX"), ("equal", "d")]);
        assert_eq!(texts(&line_diff("x", "x")), [("equal", "x")]);
        assert_eq!(texts(&line_diff("", "x")), [("delete", ""), ("insert", "x")]);
    }

    #[test]
    fn diff_finds_moved_insertions() {
        let h = line_diff("1\n2\n3", "1\n2\n2b\n3");
        assert_eq!(texts(&h), [("equal", "1\n2"), ("insert", "2b"), ("equal", "3")]);
    }

    #[test]
    fn parents_reject_cycles_depth_and_foreigners() {
        let ids: Vec<Uuid> = (0..12).map(|_| Uuid::new_v4()).collect();
        let mut m: HashMap<Uuid, Option<Uuid>> = ids.iter().map(|i| (*i, None)).collect();
        m.insert(ids[1], Some(ids[0]));
        assert!(check_parents(&m, &ids).is_ok());
        m.insert(ids[0], Some(ids[1]));
        assert!(check_parents(&m, &ids).unwrap_err().contains("sotto sé stessa"));
        m.insert(ids[0], None);
        m.insert(ids[2], Some(Uuid::new_v4()));
        assert!(check_parents(&m, &ids).unwrap_err().contains("non appartiene"));
        m.insert(ids[2], None);
        for w in 1..=9 {
            m.insert(ids[w], Some(ids[w - 1]));
        }
        assert!(check_parents(&m, &ids).unwrap_err().contains("livelli"));
    }
}
