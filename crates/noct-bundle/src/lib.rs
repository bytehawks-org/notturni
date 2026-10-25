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
        noct_profiles::module(),
        noct_admin::module(),
    ];
    #[cfg(feature = "taxonomy")]
    modules.push(noct_taxonomy::module());
    #[cfg(feature = "publications")]
    modules.push(noct_publications::module());
    #[cfg(feature = "pages")]
    modules.push(noct_pages::module());
    #[cfg(feature = "wiki")]
    modules.push(noct_wiki::module());
    #[cfg(feature = "git-sync")]
    modules.push(noct_git_sync::module());
    #[cfg(feature = "graph")]
    modules.push(noct_graph::module());
    #[cfg(feature = "legal")]
    modules.push(noct_legal::module());
    #[cfg(feature = "branding")]
    modules.push(noct_branding::module());
    #[cfg(feature = "link-preview")]
    modules.push(noct_link_preview::module());
    #[cfg(feature = "notes")]
    modules.push(noct_notes::module());
    #[cfg(feature = "fragments")]
    modules.push(noct_fragments::module());
    #[cfg(feature = "comments")]
    modules.push(noct_comments::module());
    #[cfg(feature = "discovery")]
    modules.push(noct_discovery::module());
    #[cfg(feature = "interests")]
    modules.push(noct_interests::module());
    #[cfg(feature = "auth-mfa")]
    modules.push(noct_auth_mfa::module());
    #[cfg(feature = "auth-sso")]
    modules.push(noct_auth_sso::module());
    #[cfg(feature = "reports")]
    modules.push(noct_reports::module());
    #[cfg(feature = "newsletter")]
    modules.push(noct_newsletter::module());
    #[cfg(feature = "custom-domains")]
    modules.push(noct_custom_domains::module());
    #[cfg(feature = "gdpr")]
    modules.push(noct_gdpr::module());
    #[cfg(feature = "image-moderation")]
    modules.push(noct_image_moderation::module());
    #[cfg(feature = "backup")]
    modules.push(noct_backup::module());
    #[cfg(feature = "sandbox")]
    modules.push(noct_sandbox::module());
    modules
}
