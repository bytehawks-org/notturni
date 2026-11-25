//! Permalink dei post: `/<blog-slug>/<post-slug>`, e `/post/<post-slug>` per il blog primario
//! in modalità `solo`.

use noct_core::{ApiError, AppState};

/// Permalink puro. `primary` è lo slug del blog primario (solo modalità `solo`).
pub fn permalink_for(blog_slug: &str, post_slug: &str, primary: Option<&str>, solo: bool) -> String {
    if solo && primary == Some(blog_slug) { format!("/post/{post_slug}") } else { format!("/{blog_slug}/{post_slug}") }
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
}

#[cfg(test)]
mod tests {
    use super::permalink_for;

    #[test]
    fn primary_blog_uses_post_prefix_only_in_solo() {
        assert_eq!(permalink_for("anna", "ciao", Some("anna"), true), "/post/ciao");
        assert_eq!(permalink_for("anna", "ciao", Some("anna"), false), "/anna/ciao");
        assert_eq!(permalink_for("diario", "ciao", Some("anna"), true), "/diario/ciao");
        assert_eq!(permalink_for("diario", "ciao", None, true), "/diario/ciao");
    }
}
