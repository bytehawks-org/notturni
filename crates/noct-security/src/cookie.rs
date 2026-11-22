//! Costruzione e lettura dei cookie di sessione, con validazione di ogni
//! componente (nessuna iniezione di attributi via `;` o a-capo).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
    Lax,
    Strict,
    None,
}

impl SameSite {
    pub fn parse(s: &str) -> Option<SameSite> {
        match s.to_ascii_lowercase().as_str() {
            "lax" => Some(SameSite::Lax),
            "strict" => Some(SameSite::Strict),
            "none" => Some(SameSite::None),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            SameSite::Lax => "Lax",
            SameSite::Strict => "Strict",
            SameSite::None => "None",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("componente del cookie non valido: {0}")]
pub struct CookieError(&'static str);

#[derive(Debug, Clone)]
pub struct Cookie<'a> {
    pub name: &'a str,
    pub value: &'a str,
    pub path: &'a str,
    pub domain: Option<&'a str>,
    pub max_age_secs: i64,
    pub http_only: bool,
    pub secure: bool,
    pub same_site: SameSite,
}

fn safe_text(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_graphic() && !matches!(c, ';' | ',' | '"' | '\\'))
}

impl Cookie<'_> {
    /// Valore dell'header `Set-Cookie`.
    pub fn to_header(&self) -> Result<String, CookieError> {
        if self.name.is_empty() || !safe_text(self.name) || self.name.contains('=') {
            return Err(CookieError("nome"));
        }
        if !safe_text(self.value) {
            return Err(CookieError("valore"));
        }
        if !self.path.starts_with('/') || !safe_text(self.path) {
            return Err(CookieError("path"));
        }
        let mut out = format!("{}={}; Path={}; Max-Age={}", self.name, self.value, self.path, self.max_age_secs);
        if let Some(d) = self.domain {
            if d.is_empty() || !safe_text(d) {
                return Err(CookieError("domain"));
            }
            out.push_str(&format!("; Domain={d}"));
        }
        if self.http_only {
            out.push_str("; HttpOnly");
        }
        // SameSite=None è accettato dai browser solo con Secure
        if self.secure || self.same_site == SameSite::None {
            out.push_str("; Secure");
        }
        out.push_str(&format!("; SameSite={}", self.same_site.as_str()));
        Ok(out)
    }

    /// Header che fa cancellare il cookie al browser (stessi path/domain dell'originale).
    pub fn to_clearing_header(&self) -> Result<String, CookieError> {
        Cookie { value: "", max_age_secs: 0, ..self.clone() }.to_header()
    }
}

/// Legge il cookie `name` da un header `Cookie`.
pub fn read_cookie<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    header.split(';').filter_map(|kv| kv.trim().split_once('=')).find(|(k, _)| *k == name).map(|(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Cookie<'static> {
        Cookie {
            name: "noct_refresh_token",
            value: "noct_sess_abc-DEF_123",
            path: "/api/v1/auth",
            domain: None,
            max_age_secs: 2_592_000,
            http_only: true,
            secure: true,
            same_site: SameSite::Lax,
        }
    }

    #[test]
    fn builds_a_hardened_header() {
        assert_eq!(
            base().to_header().unwrap(),
            "noct_refresh_token=noct_sess_abc-DEF_123; Path=/api/v1/auth; Max-Age=2592000; HttpOnly; Secure; SameSite=Lax"
        );
    }

    #[test]
    fn domain_and_clearing() {
        let c = Cookie { domain: Some(".notturni.eu"), ..base() };
        assert!(c.to_header().unwrap().contains("; Domain=.notturni.eu;"));
        let clear = c.to_clearing_header().unwrap();
        assert!(clear.starts_with("noct_refresh_token=; ") && clear.contains("Max-Age=0"));
    }

    #[test]
    fn samesite_none_forces_secure() {
        let c = Cookie { secure: false, same_site: SameSite::None, ..base() };
        assert!(c.to_header().unwrap().contains("; Secure; SameSite=None"));
        let c = Cookie { secure: false, ..base() };
        assert!(!c.to_header().unwrap().contains("Secure"), "in sviluppo, senza TLS, Secure è opzionale");
    }

    #[test]
    fn injection_attempts_are_rejected() {
        for bad in ["a; Domain=evil.com", "a\r\nSet-Cookie: x=y", "a b", "a,b", "a\"b"] {
            assert!(Cookie { value: bad, ..base() }.to_header().is_err(), "{bad:?}");
        }
        assert!(Cookie { path: "api", ..base() }.to_header().is_err());
        assert!(Cookie { path: "/a;b", ..base() }.to_header().is_err());
        assert!(Cookie { domain: Some("x.com; Path=/"), ..base() }.to_header().is_err());
        assert!(Cookie { name: "a=b", ..base() }.to_header().is_err());
    }

    #[test]
    fn reads_cookies_from_the_request_header() {
        let h = "theme=dark; noct_csrf_token=tok123; noct_refresh_token=r";
        assert_eq!(read_cookie(h, "noct_csrf_token"), Some("tok123"));
        assert_eq!(read_cookie(h, "noct_refresh_token"), Some("r"));
        assert_eq!(read_cookie(h, "missing"), None);
        assert_eq!(read_cookie("", "x"), None);
        assert_eq!(read_cookie("noct_csrf_token_x=1", "noct_csrf_token"), None, "nome esatto, non prefisso");
    }

    #[test]
    fn samesite_parsing() {
        assert_eq!(SameSite::parse("Lax"), Some(SameSite::Lax));
        assert_eq!(SameSite::parse("STRICT"), Some(SameSite::Strict));
        assert_eq!(SameSite::parse("boh"), None);
    }
}
