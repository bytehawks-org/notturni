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
}

impl Config {
    pub fn from_env(env: &dyn Fn(&str) -> Option<String>) -> Result<Config, String> {
        let get = |k: &str| env(k).filter(|v| !v.is_empty());
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
        let modules =
            get("NOCT_MODULES").map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect());
        let dev_auth = get("NOCT_DEV_AUTH").is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes"));
        Ok(Config {
            database_url,
            bind_addr: get("NOCT_BIND_ADDR").unwrap_or_else(|| "0.0.0.0:8000".into()),
            modules,
            dev_auth,
        })
    }
}
