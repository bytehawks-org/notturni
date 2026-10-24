use noct_mail::SmtpConfig;
use noct_storage::S3Config;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageBackend {
    S3,
    Local,
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub backend: StorageBackend,
    pub s3: S3Config,
    pub local_base_path: PathBuf,
    /// Prefisso con cui il browser raggiunge i file del backend locale.
    pub local_public_url: String,
    pub bucket_avatars: String,
    pub bucket_content: String,
    pub bucket_audit: String,
}

/// Configurazione di piattaforma letta dall'ambiente (prefisso `NOCT_`).
/// Quella dei singoli moduli è dichiarata nei loro `module.toml`.
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    /// `None` = i moduli con `default_enabled = true`; `Some` = elenco esplicito.
    pub modules: Option<Vec<String>>,
    /// Segreto HMAC dei JWT (e da cui derivano gli OTP): almeno 32 byte, mai un valore di esempio.
    pub jwt_secret: String,
    pub jwt_access_ttl_minutes: i64,
    /// Quanti proxy fidati stanno davanti all'API (Traefik = 1). Si legge l'IP del client
    /// dal N-esimo valore *da destra* di `X-Forwarded-For`; 0 ignora l'header e usa l'IP del peer.
    pub trusted_proxies: usize,
    pub storage: StorageConfig,
    /// Senza Redis i contatori e i lock vivono in memoria (un solo processo).
    pub redis_url: Option<String>,
    /// Senza NATS il bus è in memoria (un solo processo, nessuna persistenza).
    pub nats_url: Option<String>,
    pub nats_prefix: String,
    /// Senza SMTP gli invii falliscono in modo esplicito.
    pub smtp: Option<SmtpConfig>,
    /// Esegue relay dell'outbox e consumer dentro il processo API (default: sì
    /// se non c'è NATS, perché altrimenti nessuno li eseguirebbe).
    pub worker_inline: bool,
    /// Prima componente del path degli oggetti su storage (`{site_slug}/userdata/...`).
    pub site_slug: String,
    /// Invalidazione della cache del frontend (entrambi necessari, altrimenti disattivata).
    pub frontend_revalidate_url: Option<String>,
    pub revalidate_secret: Option<String>,
    /// Servizio di moderazione immagini; assente = nessuna classificazione.
    pub moderation_service_url: Option<String>,
    /// Dominio pubblico dell'istanza (`NOCT_INSTANCE_FQDN`): base dei link nelle email, degli
    /// identificatori di federazione e dominio email «blu» del sigillo di verifica.
    pub instance_fqdn: String,
    /// Origini ammesse per le chiamate dal browser (CORS, `NOCT_CORS_ORIGINS`, separate da virgola).
    pub cors_origins: Vec<String>,
    /// Origini a wildcard in aggiunta (`NOCT_CORS_ORIGIN_REGEX`), per i sottodomini per-blog che non
    /// si possono elencare in anticipo; il match è sull'intera origine.
    pub cors_origin_regex: Option<String>,
    /// Site key pubblica di Cloudflare Turnstile (`NOCT_TURNSTILE_SITE_KEY`): il frontend la usa per
    /// mostrare il captcha sui commenti aperti a tutti. La secret key è del modulo `comments`.
    pub turnstile_site_key: Option<String>,
}

impl Config {
    /// Configurazione minima, tutta in locale e in memoria: per test e prime prove.
    pub fn minimal(database_url: impl Into<String>) -> Config {
        Config {
            database_url: database_url.into(),
            bind_addr: "127.0.0.1:8000".into(),
            modules: None,
            // solo per test e prove locali: `from_env` richiede sempre NOCT_JWT_SECRET
            jwt_secret: "segreto-di-test-NON-usare-in-produzione-0123456789".into(),
            jwt_access_ttl_minutes: 15,
            trusted_proxies: 1,
            storage: StorageConfig {
                backend: StorageBackend::Local,
                s3: S3Config::default(),
                local_base_path: PathBuf::from("./data/storage"),
                local_public_url: "http://localhost:8000/storage".into(),
                bucket_avatars: "avatars".into(),
                bucket_content: "notturni-content".into(),
                bucket_audit: "notturni-audit".into(),
            },
            redis_url: None,
            nats_url: None,
            nats_prefix: "noct".into(),
            smtp: None,
            worker_inline: true,
            site_slug: "notturni".into(),
            frontend_revalidate_url: None,
            revalidate_secret: None,
            moderation_service_url: None,
            instance_fqdn: "notturni.eu".into(),
            cors_origins: vec!["http://localhost:3000".into()],
            cors_origin_regex: None,
            turnstile_site_key: None,
        }
    }

    /// Solo l'URL del database (`NOCT_DATABASE_URL` oppure `NOCT_POSTGRES_*`): per gli
    /// strumenti che non hanno bisogno di segreti né di altri servizi.
    pub fn database_url_from_env(env: &dyn Fn(&str) -> Option<String>) -> Result<String, String> {
        let get = |k: &str| env(k).filter(|v| !v.is_empty());
        if let Some(url) = get("NOCT_DATABASE_URL") {
            return Ok(url);
        }
        // stessi nomi dello stack attuale (compose.yaml / .env)
        let user =
            get("NOCT_POSTGRES_USER").ok_or("manca NOCT_DATABASE_URL (oppure NOCT_POSTGRES_USER/PASSWORD/DB)")?;
        let password = get("NOCT_POSTGRES_PASSWORD").unwrap_or_default();
        let host = get("NOCT_POSTGRES_HOST").unwrap_or_else(|| "localhost".into());
        let port = get("NOCT_POSTGRES_PORT").unwrap_or_else(|| "5432".into());
        let db = get("NOCT_POSTGRES_DB").ok_or("manca NOCT_POSTGRES_DB")?;
        Ok(format!("postgres://{user}:{password}@{host}:{port}/{db}"))
    }

    pub fn from_env(env: &dyn Fn(&str) -> Option<String>) -> Result<Config, String> {
        let get = |k: &str| env(k).filter(|v| !v.is_empty());
        let flag = |k: &str| get(k).map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"));
        let database_url = Self::database_url_from_env(env)?;
        let mut cfg = Config::minimal(database_url);
        cfg.bind_addr = get("NOCT_BIND_ADDR").unwrap_or_else(|| "0.0.0.0:8000".into());
        cfg.modules =
            get("NOCT_MODULES").map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect());
        cfg.jwt_secret = get("NOCT_JWT_SECRET").ok_or("manca NOCT_JWT_SECRET (almeno 32 byte casuali)")?;
        if cfg.jwt_secret.len() < 32 {
            return Err("NOCT_JWT_SECRET troppo corto: servono almeno 32 byte".into());
        }
        if let Some(v) = get("NOCT_JWT_ACCESS_TOKEN_TTL_MINUTES") {
            cfg.jwt_access_ttl_minutes =
                v.parse().ok().filter(|n| *n > 0).ok_or("NOCT_JWT_ACCESS_TOKEN_TTL_MINUTES non valido")?;
        }
        if let Some(v) = get("NOCT_TRUSTED_PROXIES") {
            cfg.trusted_proxies = v.parse().map_err(|_| format!("NOCT_TRUSTED_PROXIES non valido: `{v}`"))?;
        }

        let st = &mut cfg.storage;
        st.backend = match get("NOCT_STORAGE_BACKEND").as_deref() {
            None | Some("localstorage") => StorageBackend::Local,
            Some("s3") => StorageBackend::S3,
            Some(other) => return Err(format!("NOCT_STORAGE_BACKEND non valido: `{other}` (s3 | localstorage)")),
        };
        st.s3 = S3Config {
            endpoint_url: get("NOCT_S3_ENDPOINT_URL"),
            region: get("NOCT_S3_REGION"),
            access_key_id: get("NOCT_S3_ACCESS_KEY_ID"),
            secret_access_key: get("NOCT_S3_SECRET_ACCESS_KEY"),
            public_url: get("NOCT_S3_PUBLIC_URL"),
        };
        if let Some(v) = get("NOCT_LOCAL_STORAGE_BASE_PATH") {
            st.local_base_path = PathBuf::from(v);
        }
        if let Some(v) = get("NOCT_LOCAL_STORAGE_PUBLIC_URL") {
            st.local_public_url = v;
        }
        if let Some(v) = get("NOCT_S3_BUCKET_AVATARS") {
            st.bucket_avatars = v;
        }
        if let Some(v) = get("NOCT_S3_BUCKET_CONTENT") {
            st.bucket_content = v;
        }
        if let Some(v) = get("NOCT_S3_BUCKET_AUDIT") {
            st.bucket_audit = v;
        }

        cfg.redis_url = match (get("NOCT_REDIS_URL"), get("NOCT_REDIS_HOST")) {
            (Some(url), _) => Some(url),
            (None, Some(host)) => {
                let port = get("NOCT_REDIS_PORT").unwrap_or_else(|| "6379".into());
                let db = get("NOCT_REDIS_DB").unwrap_or_else(|| "0".into());
                Some(format!("redis://{host}:{port}/{db}"))
            }
            (None, None) => None,
        };
        cfg.nats_url = get("NOCT_NATS_URL");
        if let Some(p) = get("NOCT_NATS_PREFIX") {
            cfg.nats_prefix = p;
        }
        cfg.smtp = match get("NOCT_SMTP_HOST") {
            None => None,
            Some(host) => Some(SmtpConfig {
                host,
                port: get("NOCT_SMTP_PORT")
                    .map(|p| p.parse().map_err(|_| format!("NOCT_SMTP_PORT non valida: `{p}`")))
                    .transpose()?
                    .unwrap_or(587),
                user: get("NOCT_SMTP_USER"),
                password: get("NOCT_SMTP_PASSWORD"),
                use_tls: flag("NOCT_SMTP_USE_TLS").unwrap_or(true),
                from: get("NOCT_SMTP_FROM_EMAIL").unwrap_or_else(|| "no-reply@notturni.eu".into()),
            }),
        };
        cfg.worker_inline = flag("NOCT_WORKER_INLINE").unwrap_or(cfg.nats_url.is_none());
        if let Some(v) = get("NOCT_SITE_SLUG") {
            if !v.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
                return Err(format!("NOCT_SITE_SLUG non valido: `{v}` (minuscole, cifre, `-`)"));
            }
            cfg.site_slug = v;
        }
        cfg.frontend_revalidate_url = get("NOCT_FRONTEND_REVALIDATE_URL");
        cfg.revalidate_secret = get("NOCT_REVALIDATE_SECRET");
        cfg.moderation_service_url = get("NOCT_MODERATION_SERVICE_URL");
        if let Some(v) = get("NOCT_INSTANCE_FQDN") {
            cfg.instance_fqdn = v.trim().trim_end_matches('.').to_lowercase();
        }
        cfg.turnstile_site_key = get("NOCT_TURNSTILE_SITE_KEY");
        if let Some(v) = get("NOCT_CORS_ORIGINS") {
            cfg.cors_origins =
                v.split(',').map(|s| s.trim().trim_end_matches('/').to_string()).filter(|s| !s.is_empty()).collect();
        }
        if let Some(v) = get("NOCT_CORS_ORIGIN_REGEX") {
            regex::Regex::new(&format!("^(?:{v})$")).map_err(|e| format!("NOCT_CORS_ORIGIN_REGEX non valida: {e}"))?;
            cfg.cors_origin_regex = Some(v);
        }
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "0123456789abcdef0123456789abcdef";

    fn cfg(vars: &[(&str, &str)]) -> Result<Config, String> {
        Config::from_env(&|k| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string()))
    }

    #[test]
    fn defaults_are_local_and_in_memory() {
        let c = cfg(&[("NOCT_DATABASE_URL", "postgres://x/y"), ("NOCT_JWT_SECRET", SECRET)]).unwrap();
        assert_eq!(c.storage.backend, StorageBackend::Local);
        assert!(c.redis_url.is_none() && c.nats_url.is_none() && c.smtp.is_none());
        assert!(c.worker_inline, "senza NATS i worker girano nel processo API");
    }

    #[test]
    fn legacy_variable_names_are_understood() {
        let c = cfg(&[
            ("NOCT_POSTGRES_USER", "u"),
            ("NOCT_POSTGRES_PASSWORD", "p"),
            ("NOCT_POSTGRES_HOST", "postgres"),
            ("NOCT_POSTGRES_DB", "d"),
            ("NOCT_JWT_SECRET", SECRET),
            ("NOCT_REDIS_HOST", "redis"),
            ("NOCT_REDIS_DB", "2"),
            ("NOCT_STORAGE_BACKEND", "s3"),
            ("NOCT_S3_ENDPOINT_URL", "http://garage:3900"),
            ("NOCT_NATS_URL", "nats://nats:4222"),
            ("NOCT_SMTP_HOST", "mailhog"),
            ("NOCT_SMTP_PORT", "1025"),
            ("NOCT_SMTP_USE_TLS", "false"),
        ])
        .unwrap();
        assert_eq!(c.database_url, "postgres://u:p@postgres:5432/d");
        assert_eq!(c.redis_url.as_deref(), Some("redis://redis:6379/2"));
        assert_eq!(c.storage.backend, StorageBackend::S3);
        assert_eq!(c.storage.s3.endpoint_url.as_deref(), Some("http://garage:3900"));
        let smtp = c.smtp.unwrap();
        assert_eq!((smtp.port, smtp.use_tls), (1025, false));
        assert!(!c.worker_inline, "con NATS i worker sono processi a parte");
    }

    #[test]
    fn site_slug_and_auxiliary_services() {
        let base = [("NOCT_DATABASE_URL", "x"), ("NOCT_JWT_SECRET", SECRET)];
        let c = cfg(&base).unwrap();
        assert_eq!(c.site_slug, "notturni");
        assert!(c.frontend_revalidate_url.is_none() && c.moderation_service_url.is_none());
        let mut vars = base.to_vec();
        vars.extend([
            ("NOCT_SITE_SLUG", "demo-1"),
            ("NOCT_FRONTEND_REVALIDATE_URL", "http://frontend:3000/api/revalidate"),
            ("NOCT_REVALIDATE_SECRET", "s"),
            ("NOCT_MODERATION_SERVICE_URL", "http://moderation:8080"),
        ]);
        let c = cfg(&vars).unwrap();
        assert_eq!(c.site_slug, "demo-1");
        assert_eq!(c.moderation_service_url.as_deref(), Some("http://moderation:8080"));
        // lo slug diventa parte di una chiave di storage: nessun carattere strano
        let mut bad = base.to_vec();
        bad.push(("NOCT_SITE_SLUG", "../x"));
        assert!(cfg(&bad).unwrap_err().contains("NOCT_SITE_SLUG"));
    }

    #[test]
    fn invalid_values_are_reported() {
        assert!(cfg(&[]).unwrap_err().contains("NOCT_DATABASE_URL"));
        assert!(
            cfg(&[("NOCT_DATABASE_URL", "x"), ("NOCT_JWT_SECRET", SECRET), ("NOCT_STORAGE_BACKEND", "ftp")])
                .unwrap_err()
                .contains("NOCT_STORAGE_BACKEND")
        );
        assert!(
            cfg(&[
                ("NOCT_DATABASE_URL", "x"),
                ("NOCT_JWT_SECRET", SECRET),
                ("NOCT_SMTP_HOST", "h"),
                ("NOCT_SMTP_PORT", "abc")
            ])
            .unwrap_err()
            .contains("NOCT_SMTP_PORT")
        );
    }
}
