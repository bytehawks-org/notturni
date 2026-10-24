//! Costruzione del testo delle email: estratto, piè di pagina e corpo HTML.

use chrono::{DateTime, Utc};

/// Estratto del contenuto del post nell'email di notifica (il post non ha un campo dedicato:
/// si tronca il Markdown grezzo, a un confine di parola).
pub const EXCERPT_LENGTH: usize = 400;

pub fn excerpt(content: &str) -> String {
    let text = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= EXCERPT_LENGTH {
        return text;
    }
    let cut: String = text.chars().take(EXCERPT_LENGTH).collect();
    let cut = cut.rsplit_once(' ').map(|(head, _)| head).unwrap_or(&cut);
    format!("{cut}…")
}

pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#x27;")
}

fn confirmed_label(at: Option<DateTime<Utc>>) -> String {
    at.map(|t| t.format("%d/%m/%Y").to_string()).unwrap_or_else(|| "recentemente".to_string())
}

pub fn text_footer(list_label: &str, confirmed_at: Option<DateTime<Utc>>, unsubscribe_link: &str) -> String {
    format!(
        "\n\n---\nStai ricevendo questa email perché ti sei iscritto/a alla newsletter di {list_label} il {}.\n\
         Per non ricevere più queste email, disiscriviti qui: {unsubscribe_link}",
        confirmed_label(confirmed_at)
    )
}

fn html_footer(list_label: &str, confirmed_at: Option<DateTime<Utc>>, unsubscribe_link: &str) -> String {
    format!(
        "<hr style=\"border:none;border-top:1px solid #ddd;margin:24px 0;\">\
         <p style=\"font-size:12px;color:#666;\">Stai ricevendo questa email perché ti sei iscritto/a alla newsletter di {} il {}.<br>\
         Per non ricevere più queste email, <a href=\"{}\">disiscriviti qui</a>.</p>",
        escape_html(list_label),
        confirmed_label(confirmed_at),
        escape_html(unsubscribe_link)
    )
}

/// Corpo HTML: banner (se c'è), Markdown già reso in HTML sicuro, piè di pagina.
pub fn html_body(
    rendered_markdown: &str,
    banner_url: Option<&str>,
    banner_alt: &str,
    list_label: &str,
    confirmed_at: Option<DateTime<Utc>>,
    unsubscribe_link: &str,
) -> String {
    let banner = banner_url
        .map(|url| {
            format!(
                "<img src=\"{}\" alt=\"{}\" style=\"max-width:100%;display:block;margin-bottom:20px;\">",
                escape_html(url),
                escape_html(banner_alt)
            )
        })
        .unwrap_or_default();
    format!(
        "<!DOCTYPE html><html><body style=\"font-family:sans-serif;max-width:600px;margin:0 auto;color:#111;line-height:1.5;\">\
         {banner}{rendered_markdown}{}</body></html>",
        html_footer(list_label, confirmed_at, unsubscribe_link)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpt_collapses_whitespace_and_cuts_at_a_word() {
        assert_eq!(excerpt("uno  due\n\ntre"), "uno due tre");
        let long = "parola ".repeat(100);
        let e = excerpt(&long);
        assert!(e.ends_with("parola…") && e.chars().count() <= EXCERPT_LENGTH + 1, "{e}");
        // senza spazi si taglia comunque alla lunghezza massima
        assert_eq!(excerpt(&"x".repeat(1000)).chars().count(), EXCERPT_LENGTH + 1);
        assert_eq!(excerpt(&"è".repeat(1000)).chars().count(), EXCERPT_LENGTH + 1, "taglio su caratteri, non su byte");
    }

    #[test]
    fn user_controlled_values_are_escaped_in_the_html() {
        let html = html_body(
            "<p>ciao</p>",
            Some("https://example.com/b.png\" onerror=\"alert(1)"),
            "<script>",
            "Blog <b>",
            None,
            "https://example.com/u?t=1&x=2",
        );
        assert!(!html.contains("onerror=\"alert") && !html.contains("<script>") && !html.contains("<b>"), "{html}");
        assert!(html.contains("&amp;x=2") && html.contains("recentemente") && html.contains("<p>ciao</p>"), "{html}");
        assert!(!html_body("", None, "", "x", None, "u").contains("<img"));
    }
}
