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
    /// Autenticazione di sviluppo (`Authorization: Bearer dev:<user-uuid>`).
    /// Temporanea: sostituita dal JWT reale nella fase auth.
    pub dev_auth: bool,
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
}

impl Config {
    /// Configurazione minima, tutta in locale e in memoria: per test e prime prove.
    pub fn minimal(database_url: impl Into<String>) -> Config {
        Config {
            database_url: database_url.into(),
            bind_addr: "127.0.0.1:8000".into(),
            modules: None,
            dev_auth: false,
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
        }
    }

    pub fn from_env(env: &dyn Fn(&str) -> Option<String>) -> Result<Config, String> {
        let get = |k: &str| env(k).filter(|v| !v.is_empty());
        let flag = |k: &str| get(k).map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"));
        let database_url = match get("NOCT_DATABASE_URL") {
            Some(url) => url,
            None => {
                // stessi nomi dello stack attuale (compose.yaml / .env)
                let user = get("NOCT_POSTGRES_USER")
                    .ok_or("manca NOCT_DATABASE_URL (oppure NOCT_POSTGRES_USER/PASSWORD/DB)")?;
                let password = get("NOCT_POSTGRES_PASSWORD").unwrap_or_default();
                let host = get("NOCT_POSTGRES_HOST").unwrap_or_else(|| "localhost".into());
                let port = get("NOCT_POSTGRES_PORT").unwrap_or_else(|| "5432".into());
                let db = get("NOCT_POSTGRES_DB").ok_or("manca NOCT_POSTGRES_DB")?;
                format!("postgres://{user}:{password}@{host}:{port}/{db}")
            }
        };
        let mut cfg = Config::minimal(database_url);
        cfg.bind_addr = get("NOCT_BIND_ADDR").unwrap_or_else(|| "0.0.0.0:8000".into());
        cfg.modules =
            get("NOCT_MODULES").map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect());
        cfg.dev_auth = flag("NOCT_DEV_AUTH").unwrap_or(false);

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
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(vars: &[(&str, &str)]) -> Result<Config, String> {
        Config::from_env(&|k| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string()))
    }

    #[test]
    fn defaults_are_local_and_in_memory() {
        let c = cfg(&[("NOCT_DATABASE_URL", "postgres://x/y")]).unwrap();
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
    fn invalid_values_are_reported() {
        assert!(cfg(&[]).unwrap_err().contains("NOCT_DATABASE_URL"));
        assert!(
            cfg(&[("NOCT_DATABASE_URL", "x"), ("NOCT_STORAGE_BACKEND", "ftp")])
                .unwrap_err()
                .contains("NOCT_STORAGE_BACKEND")
        );
        assert!(
            cfg(&[("NOCT_DATABASE_URL", "x"), ("NOCT_SMTP_HOST", "h"), ("NOCT_SMTP_PORT", "abc")])
                .unwrap_err()
                .contains("NOCT_SMTP_PORT")
        );
    }
}
