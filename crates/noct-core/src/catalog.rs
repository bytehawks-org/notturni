//! Elenco dei moduli compilati nel binario, per l'amministrazione (attivazione e disattivazione).

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ModuleInfo {
    pub name: String,
    pub version: String,
    /// `core` (sempre attivo) oppure `optional`.
    pub kind: String,
    pub description: String,
    pub requires: Vec<String>,
    pub default_enabled: bool,
}

impl ModuleInfo {
    pub fn is_core(&self) -> bool {
        self.kind == "core"
    }
}
