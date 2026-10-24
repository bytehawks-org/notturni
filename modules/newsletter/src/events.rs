//! Handler degli eventi a cui il modulo è iscritto (`[events] subscribes`).

use crate::handlers_impl::{SUBJECT_CAMPAIGN, load_settings};
use chrono::{DateTime, Utc};
use noct_core::{
    AppState,
    bus::{HandlerError, Kind, Message, outbox},
};
use serde_json::json;
use uuid::Uuid;

type PostHead = (Uuid, String, String, String, bool, Option<DateTime<Utc>>);

fn retry(e: impl ToString) -> HandlerError {
    HandlerError::Retry(e.to_string())
}

/// Alla pubblicazione di un post nasce la campagna di notifica agli iscritti del blog: una sola per
/// post (vincolo di unicità su `post_id`), anche se l'evento viene consegnato più volte o il post
/// viene ripubblicato. Un post pianificato nel futuro dà una campagna pianificata alla stessa ora.
pub async fn on_posts_post_published(st: AppState, msg: Message) -> Result<(), HandlerError> {
    // l'evento porta i dati del post sotto `data` (insieme a `module`, `blog_id` e `actor_id`)
    let post = msg
        .payload
        .pointer("/data/post_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| HandlerError::Permanent("payload senza `post_id` valido".into()))?;
    let row: Option<PostHead> = sqlx::query_as(
        "SELECT p.blog_id, b.title, p.title, p.status::text, p.is_hidden, p.published_at
         FROM posts p JOIN blogs b ON b.id = p.blog_id WHERE p.id = $1",
    )
    .bind(post)
    .fetch_optional(&st.db)
    .await
    .map_err(retry)?;
    // post cancellato nel frattempo: niente da notificare
    let Some((blog, blog_title, post_title, status, hidden, published_at)) = row else { return Ok(()) };
    if status != "published" || hidden || !load_settings(&st.db, Some(blog)).await.map_err(retry)?.auto_notify_enabled {
        return Ok(());
    }
    let immediate = published_at.is_none_or(|t| t <= Utc::now());
    let subject: String = format!("Nuovo post su {blog_title}: {post_title}").chars().take(255).collect();

    let mut tx = st.db.begin().await.map_err(retry)?;
    let campaign: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO newsletter_campaigns (blog_id, kind, post_id, subject, status, scheduled_at)
         VALUES ($1, 'post_notification', $2, $3, $4::newsletter_campaign_status, $5)
         ON CONFLICT ON CONSTRAINT uq_newsletter_campaign_post DO NOTHING RETURNING id",
    )
    .bind(blog)
    .bind(post)
    .bind(subject)
    .bind(if immediate { "sending" } else { "scheduled" })
    .bind(if immediate { None } else { published_at })
    .fetch_optional(&mut *tx)
    .await
    .map_err(retry)?;
    if let (Some(id), true) = (campaign, immediate) {
        outbox::enqueue(&mut tx, Kind::Job, SUBJECT_CAMPAIGN, &json!({"campaign_id": id})).await.map_err(retry)?;
    }
    tx.commit().await.map_err(retry)
}
