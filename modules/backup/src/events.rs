//! Handler degli eventi a cui il modulo è iscritto.

use noct_core::{
    AppState,
    bus::{HandlerError, Message},
};
use uuid::Uuid;

fn retry(e: impl ToString) -> HandlerError {
    HandlerError::Retry(e.to_string())
}

/// `{site}/userdata/{autore}/{blog}/posts/{post}.md`: la stessa radice dei media di quell'utente nel blog.
pub fn post_key(site: &str, author: Uuid, blog: Uuid, post: Uuid) -> String {
    format!("{site}/userdata/{author}/{blog}/posts/{post}.md")
}

pub async fn on_posts_post_published(st: AppState, msg: Message) -> Result<(), HandlerError> {
    let post = msg
        .payload
        .pointer("/data/post_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| HandlerError::Permanent("payload senza `post_id` valido".into()))?;
    let row: Option<(Uuid, Uuid, String)> =
        sqlx::query_as("SELECT author_id, blog_id, content FROM posts WHERE id = $1")
            .bind(post)
            .fetch_optional(&st.db)
            .await
            .map_err(retry)?;
    // cancellato nel frattempo: niente da copiare
    let Some((author, blog, content)) = row else { return Ok(()) };
    let bucket = &st.config.storage.bucket_content;
    st.services.storage.ensure_bucket(bucket).await.map_err(retry)?;
    st.services
        .storage
        .put(
            bucket,
            &post_key(&st.config.site_slug, author, blog, post),
            content.into_bytes().into(),
            "text/markdown; charset=utf-8",
        )
        .await
        .map_err(retry)
}
