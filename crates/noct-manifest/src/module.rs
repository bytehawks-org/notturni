use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleFile {
    pub module: ModuleMeta,
    #[serde(default)]
    pub config: BTreeMap<String, ConfigField>,
    #[serde(default)]
    pub events: Events,
    #[serde(default)]
    pub jobs: BTreeMap<String, JobSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleMeta {
    pub name: String,
    pub version: String,
    pub kind: Kind,
    #[serde(default)]
    pub default_enabled: bool,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Sempre attivo, non disattivabile; non può dipendere da moduli opzionali.
    Core,
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigType {
    String,
    Int,
    Bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigField {
    #[serde(rename = "type")]
    pub ty: ConfigType,
    pub env: String,
    #[serde(default)]
    pub default: Option<toml::Value>,
    /// Mai loggato né esposto.
    #[serde(default)]
    pub secret: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Events {
    #[serde(default)]
    pub subscribes: Vec<String>,
    #[serde(default)]
    pub publishes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobSpec {
    pub subject: String,
    #[serde(default = "default_max_deliver")]
    pub max_deliver: u32,
    #[serde(default)]
    pub backoff: Vec<String>,
}

fn default_max_deliver() -> u32 {
    5
}
