//! Elenco dei moduli compilati nel binario. È l'unico posto da toccare per
//! aggiungere un modulo: dipendenza opzionale in Cargo.toml + una riga qui.
//! Le feature Cargo permettono di produrre binari più piccoli; quali moduli
//! siano *attivi* lo decide poi `NOCT_MODULES` (o il default dei manifest).

use noct_core::ModuleDef;

pub fn all() -> Vec<ModuleDef> {
    let mut modules = vec![noct_users::module(), noct_blogs::module()];
    #[cfg(feature = "taxonomy")]
    modules.push(noct_taxonomy::module());
    #[cfg(feature = "sandbox")]
    modules.push(noct_sandbox::module());
    modules
}
