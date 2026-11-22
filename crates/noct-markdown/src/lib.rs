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

/// Converte Markdown in HTML e lo sanifica.
pub fn render_safe_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let mut raw = String::new();
    html::push_html(&mut raw, Parser::new_ext(markdown, options));
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
}
