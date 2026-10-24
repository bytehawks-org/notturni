//! Elenco dei moduli compilati nel binario. È l'unico posto da toccare per
//! aggiungere un modulo: dipendenza opzionale in Cargo.toml + una riga qui.
//! Le feature Cargo permettono di produrre binari più piccoli; quali moduli
//! siano *attivi* lo decide poi `NOCT_MODULES` (o il default dei manifest).

use noct_core::ModuleDef;

pub fn all() -> Vec<ModuleDef> {
    let mut modules = vec![
        noct_users::module(),
        noct_platform::module(),
        noct_blogs::module(),
        noct_posts::module(),
        noct_media::module(),
        noct_events::module(),
        noct_audit::module(),
        noct_auth::module(),
    ];
    #[cfg(feature = "taxonomy")]
    modules.push(noct_taxonomy::module());
    #[cfg(feature = "publications")]
    modules.push(noct_publications::module());
    #[cfg(feature = "pages")]
    modules.push(noct_pages::module());
    #[cfg(feature = "link-preview")]
    modules.push(noct_link_preview::module());
    #[cfg(feature = "notes")]
    modules.push(noct_notes::module());
    #[cfg(feature = "fragments")]
    modules.push(noct_fragments::module());
    #[cfg(feature = "comments")]
    modules.push(noct_comments::module());
    #[cfg(feature = "sandbox")]
    modules.push(noct_sandbox::module());
    modules
}
