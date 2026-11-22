use crate::{ApiFile, ConfigType, Issues, Kind, ModuleFile, Schema, is_ident, parse_duration};
use regex::Regex;

/// Contenuto grezzo dei file di un modulo.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sources<'a> {
    pub module: &'a str,
    pub api: Option<&'a str>,
    pub db: Option<&'a str>,
    /// `schema.lock.json`: istantanea dello schema all'ultima `migrate generate`.
    pub lock: Option<&'a str>,
}

/// Manifest di un modulo già parsato e validato internamente.
#[derive(Debug, Clone)]
pub struct Manifest {
    pub module: ModuleFile,
    pub api: ApiFile,
    pub schema: Schema,
    pub lock: Option<Schema>,
}

impl Manifest {
    pub fn parse(src: Sources<'_>) -> Result<Manifest, Issues> {
        let mut issues = Issues::default();
        let module: ModuleFile = match toml::from_str(src.module) {
            Ok(m) => m,
            Err(err) => {
                issues.push("?", "module.toml", err.to_string());
                return Err(issues);
            }
        };
        let name = module.module.name.clone();
        let schema = src.db.map(|t| Schema::parse(&name, t, &mut issues)).unwrap_or_default();
        let api = src.api.map(|t| ApiFile::parse(&name, t, &mut issues)).unwrap_or_default();
        let lock = src.lock.and_then(|t| match serde_json::from_str::<Schema>(t) {
            Ok(s) => Some(s),
            Err(err) => {
                issues.push(&name, "schema.lock.json", err.to_string());
                None
            }
        });
        validate_module_file(&module, &mut issues);
        validate_schema_refs(&name, &schema, &mut issues);
        api.validate(&name, &schema, &mut issues);
        for r in &api.routes {
            if let Some(subject) = &r.emits
                && !module.events.publishes.contains(subject)
            {
                issues.push(
                    &name,
                    format!("api.toml route {}", r.id),
                    format!("`emits = \"{subject}\"` non è dichiarato in `[events] publishes` di module.toml"),
                );
            }
        }
        issues.into_result(Manifest { module, api, schema, lock })
    }

    pub fn name(&self) -> &str {
        &self.module.module.name
    }

    pub fn kind(&self) -> Kind {
        self.module.module.kind
    }

    /// `Some(descrizione)` se `db.toml` e `schema.lock.json` non coincidono:
    /// qualcuno ha modificato lo schema senza lanciare `noct-ctl migrate generate`.
    pub fn drift(&self) -> Option<String> {
        let current = self.schema.canonical();
        match &self.lock {
            None if current.tables.is_empty() && current.enums.is_empty() => None,
            None => Some("schema.lock.json mancante ma db.toml definisce tabelle".to_string()),
            Some(lock) if lock.canonical() == current => None,
            Some(_) => Some("db.toml differisce da schema.lock.json".to_string()),
        }
    }
}

fn validate_module_file(m: &ModuleFile, issues: &mut Issues) {
    let name = m.module.name.as_str();
    if !is_ident(name) {
        issues.push(name, "module.toml", "nome modulo non valido (minuscole, cifre, `_`)");
    }
    let semver = Regex::new(r"^\d+\.\d+\.\d+$").expect("regex statica");
    if !semver.is_match(&m.module.version) {
        issues.push(name, "module.toml", "`version` deve essere nel formato MAJOR.MINOR.PATCH");
    }
    for r in &m.module.requires {
        if !is_ident(r) || r == name {
            issues.push(name, "module.toml", format!("`requires` non valido: `{r}`"));
        }
    }
    for (key, f) in &m.config {
        let loc = format!("module.toml config {key}");
        if !is_ident(key) {
            issues.push(name, &loc, "chiave di configurazione non valida");
        }
        if !f.env.starts_with("NOCT_")
            || !f.env.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            issues.push(name, &loc, "`env` deve essere MAIUSCOLO e iniziare con `NOCT_`");
        }
        if let Some(d) = &f.default {
            let ok = matches!(
                (f.ty, d),
                (ConfigType::String, toml::Value::String(_))
                    | (ConfigType::Int, toml::Value::Integer(_))
                    | (ConfigType::Bool, toml::Value::Boolean(_))
            );
            if !ok {
                issues.push(name, &loc, "il `default` non corrisponde al `type`");
            }
        }
    }
    let subject = Regex::new(r"^[a-z][a-z0-9_]*(\.[a-z0-9_*>]+)*$").expect("regex statica");
    for s in m.events.subscribes.iter().chain(&m.events.publishes) {
        if !subject.is_match(s) {
            issues.push(name, "module.toml events", format!("subject non valido: `{s}`"));
        }
    }
    for (job, spec) in &m.jobs {
        let loc = format!("module.toml jobs {job}");
        if !is_ident(job) || !subject.is_match(&spec.subject) {
            issues.push(name, &loc, "nome job o subject non valido");
        }
        if spec.max_deliver == 0 {
            issues.push(name, &loc, "`max_deliver` deve essere > 0");
        }
        if spec.backoff.iter().any(|b| parse_duration(b).is_none()) {
            issues.push(name, &loc, "`backoff` ammette solo durate come `10s`, `1m`, `1h`");
        }
        if let Some(every) = &spec.schedule
            && !parse_duration(every).is_some_and(|s| s >= 10)
        {
            issues.push(name, &loc, "`schedule` ammette durate come `1m`, `1h`, `1d` (almeno 10s)");
        }
    }
}

/// Riferimenti che si risolvono dentro il solo modulo: FK verso tabelle proprie
/// (quelle verso altri moduli le controlla `validate_registry`).
fn validate_schema_refs(module: &str, schema: &Schema, issues: &mut Issues) {
    for t in &schema.tables {
        for c in &t.columns {
            if let Some(r) = &c.references
                && r.table == t.name
                && t.column(&r.column).is_none()
            {
                issues.push(
                    module,
                    format!("db.toml table {} column {}", t.name, c.name),
                    "FK verso colonna inesistente",
                );
            }
        }
    }
}
