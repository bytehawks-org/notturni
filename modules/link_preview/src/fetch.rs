//! Recupero dell'anteprima di una pagina esterna. L'endpoint è pubblico e fa una richiesta
//! verso un URL scelto dal chiamante, quindi il fetch è rigido contro SSRF:
//!
//! * solo `http`/`https` con un host esplicito;
//! * l'host viene risolto *qui* e ogni indirizzo deve essere pubblico; la connessione va poi
//!   proprio a quell'indirizzo verificato (`ClientBuilder::resolve`), senza un secondo giro di
//!   DNS che un server malevolo potrebbe far rispondere in modo diverso (DNS rebinding);
//! * nessun proxy di sistema, nessun redirect seguito, timeout stretti, corpo troncato.

use regex::Regex;
use reqwest::Url;
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::LazyLock,
    time::Duration,
};

const TIMEOUT: Duration = Duration::from_secs(8);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Tetto di sicurezza per pagine senza `</head>`: alcuni siti mettono molti dati prima dei meta tag.
const MAX_BYTES: usize = 3 * 1024 * 1024;
const USER_AGENT: &str = "NotturniLinkPreview/1.0";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Preview {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
}

impl Preview {
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.description.is_none() && self.image.is_none()
    }
}

fn v4_blocked(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || o[0] == 0
        || o[0] >= 240 // riservato
        || (o[0] == 100 && (64..128).contains(&o[1])) // CGNAT
        || (o[0] == 192 && o[1] == 0 && o[2] == 0) // IETF
        || (o[0] == 198 && (o[1] == 18 || o[1] == 19)) // benchmark
        || (o[0] == 192 && o[1] == 0 && o[2] == 2)
        || (o[0] == 198 && o[1] == 51 && o[2] == 100)
        || (o[0] == 203 && o[1] == 0 && o[2] == 113) // documentazione
}

fn v6_blocked(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return v4_blocked(v4);
    }
    let seg = ip.segments();
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (seg[0] & 0xfe00) == 0xfc00 // unique local
        || (seg[0] & 0xffc0) == 0xfe80 // link local
        || (seg[0] == 0x2001 && seg[1] == 0x0db8) // documentazione
        || (seg[0] == 0x0064 && seg[1] == 0xff9b) // NAT64
}

/// `true` per gli indirizzi che non devono mai essere raggiunti da un fetch pubblico.
pub fn is_blocked(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4_blocked(v4),
        IpAddr::V6(v6) => v6_blocked(v6),
    }
}

/// Un URL idoneo all'anteprima: schema http/https e host esplicito.
pub fn parse_url(raw: &str) -> Result<Url, String> {
    let invalid = || "URL non valido: solo http/https con un host esplicito.".to_string();
    let url = Url::parse(raw.trim()).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none_or(str::is_empty) {
        return Err(invalid());
    }
    Ok(url)
}

/// Risolve l'host e restituisce l'indirizzo verificato a cui connettersi. `None` se non
/// risolve o se *uno qualunque* degli indirizzi è privato (un host con record misti è sospetto).
pub async fn resolve_public(url: &Url, allow_private: bool) -> Option<SocketAddr> {
    let host = url.host_str()?;
    let port = url.port_or_known_default()?;
    let addrs: Vec<SocketAddr> = match host.trim_matches(['[', ']']).parse::<IpAddr>() {
        Ok(ip) => vec![SocketAddr::new(ip, port)],
        Err(_) => tokio::net::lookup_host((host, port)).await.ok()?.collect(),
    };
    if addrs.is_empty() || (!allow_private && addrs.iter().any(|a| is_blocked(a.ip()))) {
        return None;
    }
    addrs.into_iter().next()
}

static META: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<meta\s+[^>]*?(?:property|name)=["'](og:title|og:description|og:image|description)["'][^>]*?content=["']([^"']*)["'][^>]*?/?>"#,
    )
    .expect("regex valida")
});
static META_REVERSED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<meta\s+[^>]*?content=["']([^"']*)["'][^>]*?(?:property|name)=["'](og:title|og:description|og:image|description)["'][^>]*?/?>"#,
    )
    .expect("regex valida")
});
static TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<title[^>]*>([^<]*)</title>").expect("regex valida"));

/// Estrae titolo, descrizione e immagine dall'HTML (Open Graph, poi `<title>`/`description`).
pub fn parse_html(url: &str, html: &str) -> Preview {
    let mut meta = std::collections::HashMap::<String, String>::new();
    for c in META.captures_iter(html) {
        meta.entry(c[1].to_lowercase()).or_insert_with(|| c[2].to_string());
    }
    for c in META_REVERSED.captures_iter(html) {
        meta.entry(c[2].to_lowercase()).or_insert_with(|| c[1].to_string());
    }
    let title = meta
        .get("og:title")
        .cloned()
        .or_else(|| TITLE.captures(html).map(|c| c[1].trim().to_string()))
        .filter(|s| !s.is_empty());
    Preview {
        url: url.to_string(),
        title,
        description: meta.get("og:description").or_else(|| meta.get("description")).cloned().filter(|s| !s.is_empty()),
        image: meta.get("og:image").cloned().filter(|s| !s.is_empty()),
    }
}

/// Client che si connette solo all'indirizzo pubblico verificato per `url`, con la GET già inviata.
async fn get(url: Url, allow_private: bool) -> Option<reqwest::Response> {
    let addr = resolve_public(&url, allow_private).await?;
    let host = url.host_str().map(|h| h.trim_matches(['[', ']']).to_string())?;

    let builder = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .user_agent(USER_AGENT);
    // per un IP letterale non serve (né si può) fissare la risoluzione
    let builder = if host.parse::<IpAddr>().is_ok() { builder } else { builder.resolve(&host, addr) };
    builder.build().ok()?.get(url).send().await.ok()
}

/// `true` per gli host di YouTube, che verso un client senza cookie/consenso restituiscono una
/// pagina generica (stesso titolo e descrizione per qualunque video) invece dei meta del video.
fn is_youtube(url: &Url) -> bool {
    matches!(
        url.host_str().map(str::to_ascii_lowercase).as_deref(),
        Some("youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com" | "youtu.be")
    )
}

/// Anteprima di un video YouTube dall'endpoint oEmbed pubblico: titolo, canale e miniatura.
async fn fetch_youtube(raw: &str, allow_private: bool) -> Option<Preview> {
    let mut endpoint = Url::parse("https://www.youtube.com/oembed").ok()?;
    endpoint.query_pairs_mut().append_pair("format", "json").append_pair("url", raw);
    let resp = get(endpoint, allow_private).await?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = resp.bytes().await.ok().filter(|b| b.len() <= 64 * 1024)?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let field =
        |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let preview = Preview {
        url: raw.to_string(),
        title: field("title"),
        description: field("author_name"),
        image: field("thumbnail_url"),
    };
    (!preview.is_empty()).then_some(preview)
}

/// Non fallisce mai per problemi di rete o di parsing: restituisce un'anteprima con il solo
/// `url`, così chi chiama può ripiegare su un link semplice.
pub async fn fetch(raw: &str, allow_private: bool) -> Preview {
    let bare = Preview { url: raw.to_string(), ..Preview::default() };
    let Ok(url) = parse_url(raw) else { return bare };
    if is_youtube(&url)
        && let Some(preview) = fetch_youtube(raw, allow_private).await
    {
        return preview;
    }

    let Some(mut resp) = get(url, allow_private).await else { return bare };
    let is_html = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.to_ascii_lowercase().contains("text/html"));
    if !resp.status().is_success() || !is_html {
        return bare;
    }
    let mut body: Vec<u8> = Vec::new();
    while let Ok(Some(chunk)) = resp.chunk().await {
        body.extend_from_slice(&chunk);
        // ci si ferma alla chiusura dell'<head>: i meta tag sono tutti lì
        let tail_from = body.len().saturating_sub(chunk.len() + 8);
        if body.len() >= MAX_BYTES || body[tail_from..].windows(6).any(|w| w.eq_ignore_ascii_case(b"</head")) {
            break;
        }
    }
    parse_html(raw, &String::from_utf8_lossy(&body))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn private_and_reserved_addresses_are_blocked() {
        for s in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.5.4",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "224.0.0.1",
            "255.255.255.255",
            "240.0.0.1",
            "198.18.0.1",
            "::1",
            "::",
            "fe80::1",
            "fc00::1",
            "fd12::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "2001:db8::1",
        ] {
            assert!(is_blocked(ip(s)), "{s} deve essere bloccato");
        }
        for s in ["8.8.8.8", "1.1.1.1", "93.184.216.34", "2606:4700:4700::1111", "::ffff:8.8.8.8"] {
            assert!(!is_blocked(ip(s)), "{s} è pubblico");
        }
    }

    #[test]
    fn youtube_hosts_use_oembed() {
        for u in ["https://www.youtube.com/watch?v=x", "https://youtu.be/x", "https://m.youtube.com/watch?v=x"] {
            assert!(is_youtube(&parse_url(u).unwrap()), "{u}");
        }
        assert!(!is_youtube(&parse_url("https://notyoutube.com/watch?v=x").unwrap()));
        assert!(!is_youtube(&parse_url("https://youtube.com.evil.org/").unwrap()));
    }

    #[test]
    fn only_http_urls_with_a_host_are_accepted() {
        assert!(parse_url("https://example.com/a?b=1").is_ok());
        assert!(parse_url("http://192.168.1.1:8080/").is_ok(), "la verifica dell'IP è in resolve_public");
        for bad in [
            "ftp://example.com",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "example.com",
            "http://",
            "",
            "//example.com",
        ] {
            assert!(parse_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn open_graph_is_preferred_over_plain_tags_in_any_attribute_order() {
        let html = r#"<html><head><title> Titolo semplice </title>
            <meta name="description" content="descr semplice">
            <meta property="og:title" content="Titolo OG"/>
            <meta content="Descr OG" property="og:description">
            <meta property='og:image' content='https://x.org/i.png'></head><body>"#;
        let p = parse_html("https://x.org", html);
        assert_eq!(
            (p.title.as_deref(), p.description.as_deref(), p.image.as_deref()),
            (Some("Titolo OG"), Some("Descr OG"), Some("https://x.org/i.png"))
        );
        let p = parse_html("u", "<head><title> Solo titolo </title><meta name=\"description\" content=\"d\"></head>");
        assert_eq!((p.title.as_deref(), p.description.as_deref(), p.image), (Some("Solo titolo"), Some("d"), None));
        assert!(parse_html("u", "<p>niente</p>").is_empty());
    }

    #[test]
    fn pathological_html_is_parsed_in_linear_time() {
        let html = format!("<meta {}", "a=\"".repeat(200_000));
        let started = std::time::Instant::now();
        let _ = parse_html("u", &html);
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
