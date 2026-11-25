//! Markdown → HTML sicuro. Stesso contratto della v1 (`markdown` + `nh3`): HTML
//! incorporato nel sorgente non è mai fidato, e il risultato passa da una
//! whitelist di tag, attributi e schemi di URL.

use ammonia::Builder;
use pulldown_cmark::{Options, Parser, html};
use std::collections::{HashMap, HashSet};

const TAGS: &[&str] = &[
    "p",
    "br",
    "hr",
    "strong",
    "em",
    "s",
    "del",
    "code",
    "pre",
    "a",
    "ul",
    "ol",
    "li",
    "blockquote",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "table",
    "thead",
    "tbody",
    "tr",
    "th",
    "td",
    "img",
];
const URL_SCHEMES: &[&str] = &["http", "https", "mailto"];

fn sanitizer() -> Builder<'static> {
    let mut b = Builder::empty();
    b.tags(TAGS.iter().copied().collect::<HashSet<_>>())
        .tag_attributes(HashMap::from([
            ("a", HashSet::from(["href", "title"])),
            ("img", HashSet::from(["src", "alt", "title"])),
        ]))
        .url_schemes(URL_SCHEMES.iter().copied().collect::<HashSet<_>>())
        .link_rel(Some("noopener noreferrer"));
    b
}

/// Riscrive i blocchi ```` ```gallery ```` in righe immagine separate da una
/// riga vuota, così ogni immagine diventa un normale paragrafo con `<img>`.
///
/// Un blocco si apre con una riga ```` ```gallery ```` e si chiude con una riga
/// ```` ``` ````. Un blocco senza chiusura, e ogni altro fence, restano invariati.
/// Le righe non vuote del blocco sono mantenute: la validazione del contenuto
/// spetta al chiamante (vedi `noct-posts`).
pub fn expand_gallery_fences(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut lines = markdown.split_inclusive('\n').peekable();
    while let Some(line) = lines.next() {
        if line.trim_end() != "```gallery" {
            out.push_str(line);
            continue;
        }
        // Cerca la chiusura senza consumare le righe finché non la trova.
        let rest: Vec<&str> = lines.clone().collect();
        let Some(close) = rest.iter().position(|l| l.trim_end() == "```") else {
            out.push_str(line);
            continue;
        };
        for item in rest[..close].iter().map(|l| l.trim()).filter(|l| !l.is_empty()) {
            out.push_str(item);
            out.push_str("\n\n");
        }
        for _ in 0..=close {
            lines.next();
        }
    }
    out
}

/// Converte Markdown in HTML e lo sanifica.
pub fn render_safe_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let expanded = expand_gallery_fences(markdown);
    let mut raw = String::new();
    html::push_html(&mut raw, Parser::new_ext(&expanded, options));
    sanitizer().clean(&raw).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_basic_markdown() {
        let html = render_safe_html("# Titolo\n\nUn **testo** con ~~barrato~~ e `codice`.\n\n- a\n- b\n");
        assert!(html.contains("<h1>Titolo</h1>"));
        assert!(
            html.contains("<strong>testo</strong>")
                && html.contains("<del>barrato</del>")
                && html.contains("<code>codice</code>")
        );
        assert!(html.contains("<ul>") && html.contains("<li>a</li>"));
    }

    #[test]
    fn renders_tables() {
        let html = render_safe_html("| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert!(html.contains("<table>") && html.contains("<th>a</th>") && html.contains("<td>2</td>"));
    }

    #[test]
    fn strips_scripts_handlers_and_dangerous_urls() {
        let html =
            render_safe_html("<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>\n\n[x](javascript:alert(1))\n");
        assert!(!html.contains("<script") && !html.contains("alert(1)</script"));
        assert!(!html.contains("onerror"));
        assert!(!html.contains("javascript:"));
    }

    #[test]
    fn links_get_rel_and_keep_allowed_schemes() {
        let html = render_safe_html("[sito](https://example.com) [mail](mailto:a@example.com)");
        assert!(html.contains(r#"href="https://example.com""#) && html.contains(r#"rel="noopener noreferrer""#));
        assert!(html.contains("mailto:a@example.com"));
    }

    #[test]
    fn disallowed_tags_are_removed_but_text_survives() {
        let html = render_safe_html("<div><span>ciao</span></div>");
        assert!(!html.contains("<div") && !html.contains("<span") && html.contains("ciao"));
    }

    #[test]
    fn gallery_fence_renders_each_image() {
        let md = "Intro\n\n```gallery\n![uno](https://example.com/1.jpg)\n![due](https://example.com/2.jpg \"t\")\n![tre](https://example.com/3.jpg)\n```\n\nFine\n";
        let html = render_safe_html(md);
        assert_eq!(html.matches("<img ").count(), 3, "{html}");
        assert!(!html.contains("<pre"), "{html}");
        assert!(!html.contains("!["), "{html}");
        assert!(html.contains(r#"src="https://example.com/2.jpg""#));
        assert!(html.contains("Fine"));
    }

    #[test]
    fn normal_fenced_code_is_unchanged() {
        let md = "```rust\nfn main() {}\n```\n";
        assert_eq!(expand_gallery_fences(md), md);
        let html = render_safe_html(md);
        assert!(html.contains("<pre>") && html.contains("fn main() {}"));
    }

    #[test]
    fn gallery_fence_with_other_info_is_untouched() {
        let md = "```gallerytext\n![a](https://example.com/a.jpg)\n```\n";
        assert_eq!(expand_gallery_fences(md), md);
    }

    #[test]
    fn unclosed_gallery_fence_does_not_panic_and_is_kept() {
        let md = "```gallery\n![a](https://example.com/a.jpg)\n";
        assert_eq!(expand_gallery_fences(md), md);
        let html = render_safe_html(md);
        assert!(!html.contains("<img"));
    }

    #[test]
    fn gallery_alt_and_url_are_sanitized() {
        let md = "```gallery\n![<script>alert(1)</script>](javascript:alert(1))\n![x\" onerror=\"alert(2)](https://example.com/b.jpg)\n```\n";
        let html = render_safe_html(md);
        // Alt text is escaped inside the attribute: no live element or handler.
        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains(r#"src="javascript"#), "{html}");
        assert!(!html.contains(r#"" onerror"#) && !html.contains("<img onerror"), "{html}");
        assert!(html.contains(r#"src="https://example.com/b.jpg""#), "{html}");
    }

    #[test]
    fn gallery_fence_is_not_rewritten_inside_other_fence_content() {
        // Un blocco gallery senza chiusura prima di un altro fence non deve
        // inghiottire il resto del documento.
        let md = "```gallery\n![a](https://example.com/a.jpg)\n\n```rust\nlet x = 1;\n```\n";
        let out = expand_gallery_fences(md);
        assert!(out.contains("```rust"), "{out}");
    }
}
