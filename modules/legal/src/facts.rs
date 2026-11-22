//! Ciò che i modelli devono sapere dell'installazione: i dati del titolare inseriti dall'amministratore e le
//! funzioni attive / i valori dedotti dalla configurazione (così il testo descrive ciò che il Servizio fa davvero).

use chrono::Utc;
use noct_core::{ApiError, AppState};
use noct_manifest::ConfigValue;
use sqlx::FromRow;
use std::collections::BTreeMap;

/// Giorni entro cui si risponde a una richiesta GDPR (art. 12): stessa costante del modulo `gdpr`.
const DEADLINE_DAYS: i64 = 30;

#[derive(Debug, Default, Clone, FromRow)]
pub struct Settings {
    pub controller_name: Option<String>,
    pub controller_address: Option<String>,
    pub controller_email: Option<String>,
    pub controller_registry: Option<String>,
    pub dpo_contact: Option<String>,
    pub hosting_description: Option<String>,
    pub processors: Option<String>,
    pub backup_retention: Option<String>,
    pub audit_archive_retention: Option<String>,
    pub minimum_age: Option<i32>,
}

pub const SETTINGS_SELECT: &str = "SELECT controller_name, controller_address, controller_email, controller_registry, dpo_contact,
    hosting_description, processors, backup_retention, audit_archive_retention, minimum_age FROM legal_settings WHERE id = 1";

impl Settings {
    pub async fn load(db: &sqlx::PgPool) -> Result<Settings, sqlx::Error> {
        Ok(sqlx::query_as(SETTINGS_SELECT).fetch_optional(db).await?.unwrap_or_default())
    }

    /// Come [`load`](Self::load), dentro una transazione già aperta (vede le modifiche non ancora confermate).
    pub async fn load_in(conn: &mut sqlx::PgConnection) -> Result<Settings, sqlx::Error> {
        Ok(sqlx::query_as(SETTINGS_SELECT).fetch_optional(conn).await?.unwrap_or_default())
    }

    /// Campo per campo, come testo («» se vuoto).
    pub fn values(&self) -> BTreeMap<String, String> {
        let t = |v: &Option<String>| v.clone().unwrap_or_default();
        [
            ("controller_name", t(&self.controller_name)),
            ("controller_address", t(&self.controller_address)),
            ("controller_email", t(&self.controller_email)),
            ("controller_registry", t(&self.controller_registry)),
            ("dpo_contact", t(&self.dpo_contact)),
            ("hosting_description", t(&self.hosting_description)),
            ("processors", t(&self.processors)),
            ("backup_retention", t(&self.backup_retention)),
            ("audit_archive_retention", t(&self.audit_archive_retention)),
            ("minimum_age", self.minimum_age.map(|a| a.to_string()).unwrap_or_default()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Facts {
    pub flags: BTreeMap<String, bool>,
    /// dati del titolare più valori dedotti (`instance_fqdn`, `session_days`...), senza `last_update`
    pub values: BTreeMap<String, String>,
}

fn int(st: &AppState, module: &str, key: &str) -> Option<i64> {
    match st.module_config.get(module)?.get(key)? {
        ConfigValue::Int(n) => Some(*n),
        _ => None,
    }
}

pub async fn compute(st: &AppState, settings: &Settings) -> Result<Facts, ApiError> {
    let on = |m: &str| st.module_enabled(m);
    let configured = |module: &str, key: &str| on(module) && st.module_config_str(module, key).is_some();
    let provider = |name: &str| {
        configured("auth_sso", &format!("{name}_client_id")) && configured("auth_sso", &format!("{name}_client_secret"))
    };
    let (google, microsoft, github, linkedin) =
        (provider("google"), provider("microsoft"), provider("github"), provider("linkedin"));
    let sso = google || microsoft || github || linkedin;
    // la verifica anti-abuso dei commenti aperti a tutti esiste solo con le chiavi di Cloudflare Turnstile
    let turnstile =
        on("comments") && st.config.turnstile_site_key.is_some() && configured("comments", "turnstile_secret_key");
    let flags: BTreeMap<String, bool> = [
        ("newsletter", on("newsletter")),
        ("comments", on("comments")),
        ("turnstile", turnstile),
        ("sso", sso),
        ("sso_google", google),
        ("sso_microsoft", microsoft),
        ("sso_github", github),
        ("sso_linkedin", linkedin),
        ("mfa", on("auth_mfa")),
        ("custom_domains", on("custom_domains")),
        ("git_sync", on("git_sync")),
        ("link_preview", on("link_preview")),
        ("image_moderation", on("image_moderation") && st.config.moderation_service_url.is_some()),
        ("smtp", st.config.smtp.is_some()),
        ("backup", configured("backup", "s3_bucket")),
        ("audit_archive", st.module_flag("audit", "archive_enabled").unwrap_or(false)),
        ("reports", on("reports")),
        ("gdpr_tools", on("gdpr")),
        ("discovery", on("discovery")),
        ("solo", st.module_config_str("auth", "deployment_mode") == Some("solo")),
        // soggetti che possono trattare dati fuori dallo SEE per le funzioni attive
        ("transfers", sso || turnstile),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();

    let platform = noct_platform::get(&st.db).await?;
    let mut values = settings.values();
    values.extend(
        [
            ("instance_fqdn", st.config.instance_fqdn.clone()),
            ("session_days", int(st, "auth", "refresh_ttl_days").unwrap_or(30).to_string()),
            ("audit_days", platform.audit_retention_days.to_string()),
            ("deletion_days", DEADLINE_DAYS.to_string()),
            ("grace_days", noct_blogs::BLOG_DELETE_GRACE_DAYS.to_string()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v)),
    );
    Ok(Facts { flags, values })
}

/// Data di aggiornamento da scrivere nel testo: quella di pubblicazione, o oggi per l'anteprima.
pub fn today() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}
