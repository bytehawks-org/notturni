use crate::{ConfigType, Issues, ModuleFile};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    String(String),
    Int(i64),
    Bool(bool),
}

/// Risolve la configurazione di un modulo da `env` (lookup iniettabile, per i test):
/// variabile → default → errore se manca. Tutti gli errori vengono raccolti.
pub fn resolve_config(
    module: &ModuleFile,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<BTreeMap<String, ConfigValue>, Issues> {
    let mut out = BTreeMap::new();
    let mut issues = Issues::default();
    let name = &module.module.name;
    for (key, field) in &module.config {
        let loc = format!("config {key} ({})", field.env);
        let value = match env(&field.env) {
            Some(raw) => match field.ty {
                ConfigType::String => Some(ConfigValue::String(raw)),
                ConfigType::Int => match raw.trim().parse() {
                    Ok(n) => Some(ConfigValue::Int(n)),
                    Err(_) => {
                        issues.push(name, &loc, "valore non intero");
                        None
                    }
                },
                ConfigType::Bool => match raw.trim().to_ascii_lowercase().as_str() {
                    "1" | "true" | "yes" | "on" => Some(ConfigValue::Bool(true)),
                    "0" | "false" | "no" | "off" => Some(ConfigValue::Bool(false)),
                    _ => {
                        issues.push(name, &loc, "valore non booleano (true/false)");
                        None
                    }
                },
            },
            None => match &field.default {
                Some(toml::Value::String(s)) => Some(ConfigValue::String(s.clone())),
                Some(toml::Value::Integer(n)) => Some(ConfigValue::Int(*n)),
                Some(toml::Value::Boolean(b)) => Some(ConfigValue::Bool(*b)),
                _ => {
                    issues.push(name, &loc, "variabile mancante e nessun default");
                    None
                }
            },
        };
        if let Some(v) = value {
            out.insert(key.clone(), v);
        }
    }
    issues.into_result(out)
}
