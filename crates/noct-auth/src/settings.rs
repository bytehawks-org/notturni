use noct_core::AppState;
use noct_manifest::ConfigValue;
use noct_security::cookie::SameSite;

/// Configurazione del modulo, letta e validata una volta all'avvio (`startup::run`)
/// e riletta dagli handler da `AppState`.
#[derive(Debug, Clone)]
pub struct AuthSettings {
    pub registration_mode: String,
    /// Seme di `platform_config.default_locale`.
    pub default_locale: String,
    pub solo: bool,
    pub refresh_ttl_days: i64,
    pub cookie_secure: bool,
    pub same_site: SameSite,
    pub cookie_domain: Option<String>,
}

impl AuthSettings {
    pub fn from_state(st: &AppState) -> Result<Self, String> {
        let cfg = st.module_config.get("auth").ok_or("configurazione del modulo auth assente")?;
        let text = |k: &str| match cfg.get(k) {
            Some(ConfigValue::String(s)) => Some(s.clone()),
            _ => None,
        };
        let registration_mode = text("registration_mode").unwrap_or_else(|| "open".into());
        if !["open", "invite", "closed"].contains(&registration_mode.as_str()) {
            return Err(format!("NOCT_REGISTRATION_MODE non valido: `{registration_mode}` (open | invite | closed)"));
        }
        let default_locale = text("default_locale").unwrap_or_else(|| "it".into());
        if !["it", "en"].contains(&default_locale.as_str()) {
            return Err(format!("NOCT_DEFAULT_LOCALE non valido: `{default_locale}` (it | en)"));
        }
        let deployment = text("deployment_mode").unwrap_or_else(|| "platform".into());
        if !["platform", "solo"].contains(&deployment.as_str()) {
            return Err(format!("NOCT_DEPLOYMENT_MODE non valido: `{deployment}` (platform | solo)"));
        }
        let refresh_ttl_days = match cfg.get("refresh_ttl_days") {
            Some(ConfigValue::Int(n)) if *n > 0 => *n,
            _ => return Err("NOCT_JWT_REFRESH_TOKEN_TTL_DAYS deve essere un intero positivo".into()),
        };
        let same_site = text("cookie_samesite")
            .and_then(|s| SameSite::parse(&s))
            .ok_or("NOCT_SESSION_COOKIE_SAMESITE non valido (lax | strict | none)")?;
        Ok(Self {
            registration_mode,
            default_locale,
            solo: deployment == "solo",
            refresh_ttl_days,
            cookie_secure: matches!(cfg.get("cookie_secure"), Some(ConfigValue::Bool(true))),
            same_site,
            cookie_domain: text("cookie_domain").filter(|d| !d.is_empty()),
        })
    }

    pub fn refresh_ttl_secs(&self) -> i64 {
        self.refresh_ttl_days * 24 * 3600
    }
}
