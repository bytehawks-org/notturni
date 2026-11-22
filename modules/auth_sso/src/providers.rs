//! Provider supportati: endpoint, scope e lettura del profilo. Nessuna rete qui, così è testabile.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Google,
    Microsoft,
    Github,
    Linkedin,
}

impl Provider {
    pub const ALL: [Provider; 4] = [Provider::Google, Provider::Microsoft, Provider::Github, Provider::Linkedin];

    pub fn name(self) -> &'static str {
        match self {
            Provider::Google => "google",
            Provider::Microsoft => "microsoft",
            Provider::Github => "github",
            Provider::Linkedin => "linkedin",
        }
    }

    pub fn parse(s: &str) -> Option<Provider> {
        Provider::ALL.into_iter().find(|p| p.name() == s)
    }

    pub fn scope(self) -> &'static str {
        match self {
            Provider::Github => "read:user user:email",
            _ => "openid email profile",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    pub authorize: String,
    pub token: String,
    pub userinfo: String,
    /// Solo GitHub: l'email si legge a parte e va verificata.
    pub emails: Option<String>,
}

/// Endpoint reali del provider; con `base` non vuoto (solo prove) `<base>/<provider>/…`.
pub fn endpoints(p: Provider, base: &str) -> Endpoints {
    if !base.is_empty() {
        let b = format!("{}/{}", base.trim_end_matches('/'), p.name());
        return Endpoints {
            authorize: format!("{b}/authorize"),
            token: format!("{b}/token"),
            userinfo: format!("{b}/userinfo"),
            emails: (p == Provider::Github).then(|| format!("{b}/emails")),
        };
    }
    let (authorize, token, userinfo, emails) = match p {
        Provider::Google => (
            "https://accounts.google.com/o/oauth2/v2/auth",
            "https://oauth2.googleapis.com/token",
            "https://openidconnect.googleapis.com/v1/userinfo",
            None,
        ),
        Provider::Microsoft => (
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            "https://login.microsoftonline.com/common/oauth2/v2.0/token",
            "https://graph.microsoft.com/oidc/userinfo",
            None,
        ),
        Provider::Github => (
            "https://github.com/login/oauth/authorize",
            "https://github.com/login/oauth/access_token",
            "https://api.github.com/user",
            Some("https://api.github.com/user/emails"),
        ),
        Provider::Linkedin => (
            "https://www.linkedin.com/oauth/v2/authorization",
            "https://www.linkedin.com/oauth/v2/accessToken",
            "https://api.linkedin.com/v2/userinfo",
            None,
        ),
    };
    Endpoints {
        authorize: authorize.into(),
        token: token.into(),
        userinfo: userinfo.into(),
        emails: emails.map(str::to_string),
    }
}

/// Dati minimi dell'identità esterna.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalProfile {
    pub provider_user_id: String,
    pub email: String,
    /// Il provider garantisce che l'utente controlla questa email. Senza, l'identità non può
    /// collegarsi a un account esistente: chiunque potrebbe dichiarare l'email di un altro.
    pub email_verified: bool,
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s.eq_ignore_ascii_case("true"),
        _ => false,
    }
}

/// Profilo da un userinfo OIDC (Google, Microsoft, LinkedIn): `sub`, `email`, `email_verified`.
pub fn from_oidc(v: &Value) -> Option<ExternalProfile> {
    let id = v.get("sub").and_then(Value::as_str).filter(|s| !s.is_empty())?;
    let email = v.get("email").and_then(Value::as_str).map(str::trim).filter(|e| !e.is_empty())?;
    Some(ExternalProfile {
        provider_user_id: id.to_string(),
        email: email.to_lowercase(),
        email_verified: truthy(v.get("email_verified")),
    })
}

/// Profilo GitHub: l'id da `/user`, l'email dalla lista di `/user/emails` — solo quella
/// primaria; è verificata se lo dice GitHub. L'email pubblica del profilo non si usa: è libera.
pub fn from_github(user: &Value, emails: &Value) -> Option<ExternalProfile> {
    let id = user.get("id").and_then(Value::as_i64)?;
    let primary = emails.as_array()?.iter().find(|e| truthy(e.get("primary")))?;
    let email = primary.get("email").and_then(Value::as_str).map(str::trim).filter(|e| !e.is_empty())?;
    Some(ExternalProfile {
        provider_user_id: id.to_string(),
        email: email.to_lowercase(),
        email_verified: truthy(primary.get("verified")),
    })
}

/// `code_challenge` PKCE (S256).
pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn authorize_url(
    ep: &Endpoints,
    client_id: &str,
    redirect_uri: &str,
    scope: &str,
    state: &str,
    challenge: &str,
) -> String {
    let sep = if ep.authorize.contains('?') { '&' } else { '?' };
    format!(
        "{}{sep}response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={challenge}&code_challenge_method=S256",
        ep.authorize,
        percent_encode(client_id),
        percent_encode(redirect_uri),
        percent_encode(scope),
        percent_encode(state),
    )
}

/// Username valido a partire dalla parte locale di un'email (l'unicità è di chi chiama).
pub fn username_candidate(email: &str) -> String {
    let local = email.split('@').next().unwrap_or("");
    let cleaned: String =
        local.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-')).collect();
    // separatori ripetuti → uno solo, mai a inizio o fine
    let mut collapsed = String::new();
    for c in cleaned.chars() {
        if matches!(c, '_' | '-') && collapsed.ends_with(['_', '-']) {
            continue;
        }
        collapsed.push(c);
    }
    let mut name: String =
        collapsed.trim_matches(['_', '-']).chars().take(noct_core::security::validate::USERNAME_MAX_LENGTH).collect();
    if name.chars().count() < noct_core::security::validate::USERNAME_MIN_LENGTH {
        name = format!("{name}utente").chars().take(noct_core::security::validate::USERNAME_MAX_LENGTH).collect();
    }
    name.trim_end_matches(['_', '-']).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn providers_parse_and_have_endpoints() {
        for p in Provider::ALL {
            assert_eq!(Provider::parse(p.name()), Some(p));
            let ep = endpoints(p, "");
            assert!(
                ep.authorize.starts_with("https://")
                    && ep.token.starts_with("https://")
                    && ep.userinfo.starts_with("https://")
            );
        }
        assert_eq!(Provider::parse("facebook"), None);
        assert!(endpoints(Provider::Github, "").emails.is_some() && endpoints(Provider::Google, "").emails.is_none());
        let t = endpoints(Provider::Github, "http://127.0.0.1:9/");
        assert_eq!(
            (t.authorize.as_str(), t.emails.as_deref()),
            ("http://127.0.0.1:9/github/authorize", Some("http://127.0.0.1:9/github/emails"))
        );
    }

    #[test]
    fn oidc_profile_needs_sub_and_email_and_tracks_verification() {
        let p = from_oidc(&json!({"sub": "123", "email": " Anna@Example.com ", "email_verified": true})).unwrap();
        assert_eq!(
            (p.provider_user_id.as_str(), p.email.as_str(), p.email_verified),
            ("123", "anna@example.com", true)
        );
        assert!(from_oidc(&json!({"sub": "1", "email": "a@b.it", "email_verified": "true"})).unwrap().email_verified);
        assert!(
            !from_oidc(&json!({"sub": "1", "email": "a@b.it"})).unwrap().email_verified,
            "senza claim non è verificata"
        );
        assert!(!from_oidc(&json!({"sub": "1", "email": "a@b.it", "email_verified": false})).unwrap().email_verified);
        assert!(from_oidc(&json!({"email": "a@b.it"})).is_none() && from_oidc(&json!({"sub": "1"})).is_none());
    }

    #[test]
    fn github_uses_only_the_primary_email_and_its_verification() {
        let user = json!({"id": 42, "email": "pubblica@example.com"});
        let emails = json!([
            {"email": "altra@example.com", "primary": false, "verified": true},
            {"email": "Primaria@Example.com", "primary": true, "verified": true},
        ]);
        let p = from_github(&user, &emails).unwrap();
        assert_eq!(
            (p.provider_user_id.as_str(), p.email.as_str(), p.email_verified),
            ("42", "primaria@example.com", true)
        );
        let unverified = json!([{"email": "x@example.com", "primary": true, "verified": false}]);
        assert!(!from_github(&user, &unverified).unwrap().email_verified);
        assert!(from_github(&user, &json!([])).is_none() && from_github(&json!({}), &emails).is_none());
    }

    #[test]
    fn pkce_matches_the_rfc_7636_example() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn authorize_url_is_encoded() {
        let ep = endpoints(Provider::Google, "");
        let url = authorize_url(&ep, "id 1", "http://x.org/cb?a=b", "openid email", "st", "ch");
        assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id=id%201&redirect_uri=http%3A%2F%2Fx.org%2Fcb%3Fa%3Db&scope=openid%20email&state=st&code_challenge=ch&code_challenge_method=S256"));
    }

    #[test]
    fn username_candidates_are_valid_shapes() {
        for (email, expected) in [
            ("anna.rossi@x.org", "annarossi"),
            ("a@x.org", "autente"),
            ("--ab--cd__@x.org", "ab-cd"),
            ("ÈÈ@x.org", "utente"),
        ] {
            assert_eq!(username_candidate(email), expected, "{email}");
        }
        let long = username_candidate(&format!("{}@x.org", "a".repeat(80)));
        assert_eq!(long.len(), 32);
        for email in ["anna@x.org", "x_y-z@x.org", "-@x.org", "12@x.org"] {
            noct_core::security::validate::validate_username(&username_candidate(email)).unwrap_or_else(|e| {
                // "me"/"admin"... li gestisce chi chiama aggiungendo un suffisso
                assert!(e.contains("riservato"), "{email}: {e}");
            });
        }
    }
}
