//! Permalink dei post e delle sottopagine: `/<blog-slug>/<post-slug>`, e `/post/<post-slug>`
//! per il blog primario in modalità `solo`. Condiviso dai crate dei moduli che non dipendono
//! da `noct-posts`.

use crate::{ApiError, AppState};

/// Permalink puro. `primary` è lo slug del blog primario (solo modalità `solo`).
pub fn permalink_for(blog_slug: &str, post_slug: &str, primary: Option<&str>, solo: bool) -> String {
    if solo && primary == Some(blog_slug) { format!("/post/{post_slug}") } else { format!("/{blog_slug}/{post_slug}") }
}

/// Percorso di una risorsa dello spazio di un blog. `suffix` è vuoto oppure inizia con `/`.
/// Nel blog primario in modalità `solo` lo spazio coincide con la radice del sito.
pub fn space_path_for(blog_slug: &str, suffix: &str, primary: Option<&str>, solo: bool) -> String {
    if solo && primary == Some(blog_slug) {
        if suffix.is_empty() { "/".to_string() } else { suffix.to_string() }
    } else {
        format!("/{blog_slug}{suffix}")
    }
}

/// Contesto caricato una volta per risposta: evita una query per ogni post.
#[derive(Debug, Clone, Default)]
pub struct Permalinks {
    solo: bool,
    primary: Option<String>,
}

impl Permalinks {
    /// In modalità platform non legge il database: il comportamento è quello di sempre.
    pub async fn load(st: &AppState) -> Result<Self, ApiError> {
        if !st.is_solo() {
            return Ok(Self::default());
        }
        let primary: Option<Option<String>> = sqlx::query_scalar(
            "SELECT b.slug FROM platform_config p JOIN blogs b ON b.id = p.primary_blog_id WHERE p.id = 1",
        )
        .fetch_optional(&st.db)
        .await?;
        Ok(Self { solo: true, primary: primary.flatten() })
    }

    /// Slug del blog primario, solo in modalità `solo` (altrimenti `None`).
    pub fn primary_slug(&self) -> Option<&str> {
        if self.solo { self.primary.as_deref() } else { None }
    }

    pub fn of(&self, blog_slug: &str, post_slug: &str) -> String {
        permalink_for(blog_slug, post_slug, self.primary.as_deref(), self.solo)
    }

    /// Percorso di una sottopagina (`suffix` vuoto o che inizia con `/`) dello spazio del blog.
    pub fn space(&self, blog_slug: &str, suffix: &str) -> String {
        space_path_for(blog_slug, suffix, self.primary.as_deref(), self.solo)
    }
}

#[cfg(test)]
mod tests {
    use super::{Permalinks, permalink_for, space_path_for};

    #[test]
    fn primary_blog_uses_post_prefix_only_in_solo() {
        assert_eq!(permalink_for("anna", "ciao", Some("anna"), true), "/post/ciao");
        assert_eq!(permalink_for("anna", "ciao", Some("anna"), false), "/anna/ciao");
        assert_eq!(permalink_for("diario", "ciao", Some("anna"), true), "/diario/ciao");
        assert_eq!(permalink_for("diario", "ciao", None, true), "/diario/ciao");
    }

    #[test]
    fn primary_space_in_solo_maps_to_root() {
        assert_eq!(space_path_for("anna", "", Some("anna"), true), "/");
        assert_eq!(space_path_for("anna", "/pagina/x", Some("anna"), true), "/pagina/x");
        assert_eq!(space_path_for("diario", "", Some("anna"), true), "/diario");
        assert_eq!(space_path_for("diario", "/pagina/x", Some("anna"), true), "/diario/pagina/x");
    }

    #[test]
    fn platform_space_ignores_primary() {
        assert_eq!(space_path_for("anna", "", Some("anna"), false), "/anna");
        assert_eq!(space_path_for("anna", "/pagina/x", Some("anna"), false), "/anna/pagina/x");
    }

    #[test]
    fn permalinks_space_uses_loaded_context() {
        let solo = Permalinks { solo: true, primary: Some("anna".into()) };
        assert_eq!(solo.space("anna", ""), "/");
        assert_eq!(solo.space("anna", "/pagina/x"), "/pagina/x");
        assert_eq!(solo.space("diario", "/pagina/x"), "/diario/pagina/x");
        let platform = Permalinks { solo: false, primary: Some("anna".into()) };
        assert_eq!(platform.space("anna", ""), "/anna");
    }
}
