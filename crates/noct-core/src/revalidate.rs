//! Tag di invalidazione della cache del frontend. Le stringhe devono restare identiche a
//! quelle di `frontend/src/lib/revalidate.ts`: un disallineamento rompe l'invalidazione
//! mirata senza errori visibili.

use crate::AppState;
use uuid::Uuid;

pub fn feed() -> String {
    "feed".into()
}

pub fn blog(slug: &str) -> String {
    format!("blog:{slug}")
}

pub fn post(blog_slug: &str, post_slug: &str) -> String {
    format!("post:{blog_slug}:{post_slug}")
}

pub fn user(username: &str) -> String {
    format!("user:{username}")
}

pub fn platform_pages() -> String {
    "platform-pages".into()
}

pub fn platform_page(slug: &str) -> String {
    format!("platform-page:{slug}")
}

pub fn blog_page(blog_slug: &str, page_slug: &str) -> String {
    format!("blog-page:{blog_slug}:{page_slug}")
}

pub fn platform_footer() -> String {
    "platform-footer".into()
}

/// Testi dell'interfaccia modificati dagli amministratori (mostrati su ogni pagina pubblica).
pub fn site_texts() -> String {
    "site-texts".into()
}

/// Identità e meta tag del portale (icona, titolo, descrizione, verifiche): letti dal layout di ogni pagina.
pub fn site_seo() -> String {
    "site-seo".into()
}

/// Invalida tutto ciò che mostra dati pubblici di un utente: la sua pagina (`user:`), i feed e
/// le pagine dei blog di cui è proprietario (nome e avatar dell'autore, conteggi).
/// `extra_usernames`: vecchi username, la cui pagina pubblica non esiste più (rinomina).
pub async fn invalidate_user(st: &AppState, user_id: Uuid, extra_usernames: &[String]) {
    let username: Option<String> = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&st.db)
        .await
        .ok()
        .flatten();
    let slugs: Vec<String> = sqlx::query_scalar("SELECT slug FROM blogs WHERE owner_id = $1")
        .bind(user_id)
        .fetch_all(&st.db)
        .await
        .unwrap_or_default();
    let mut tags = vec![feed()];
    tags.extend(username.iter().chain(extra_usernames).map(|u| user(u)));
    tags.extend(slugs.iter().map(|s| blog(s)));
    st.services.revalidate.spawn(tags);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_match_the_frontend() {
        assert_eq!(feed(), "feed");
        assert_eq!(blog("demo"), "blog:demo");
        assert_eq!(post("demo", "ciao"), "post:demo:ciao");
        assert_eq!(blog_page("demo", "chi-siamo"), "blog-page:demo:chi-siamo");
        assert_eq!(platform_page("privacy"), "platform-page:privacy");
        assert_eq!(platform_pages(), "platform-pages");
        assert_eq!(platform_footer(), "platform-footer");
    }
}
