//! Job del modulo (`[jobs.*]` in module.toml).

use crate::{
    content,
    handlers_impl::{SUBJECT_CAMPAIGN, Settings, load_settings},
    list_key, tokens,
};
use chrono::{DateTime, Utc};
use noct_core::{
    AppState,
    bus::{HandlerError, Kind, Message, outbox},
    mail::{Email, MailError},
    markdown::render_safe_html,
};
use serde_json::json;
use sqlx::PgConnection;
use uuid::Uuid;

const BATCH: i64 = 100;

fn retry(e: impl ToString) -> HandlerError {
    HandlerError::Retry(e.to_string())
}

fn fqdn(st: &AppState) -> &str {
    &st.config.instance_fqdn
}

fn payload_uuid(msg: &Message, key: &str) -> Result<Uuid, HandlerError> {
    msg.payload
        .get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| HandlerError::Permanent(format!("payload senza `{key}` valido")))
}

// ---- conferma -------------------------------------------------------------------------------------------------------

/// Spedisce il link di conferma. Il messaggio porta solo l'id dell'iscritto: il token si deriva qui.
pub async fn send_confirmation(st: AppState, msg: Message) -> Result<(), HandlerError> {
    let id = payload_uuid(&msg, "subscriber_id")?;
    let row: Option<(String, String, Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "SELECT email, status::text, blog_id, confirm_nonce FROM newsletter_subscribers
         WHERE id = $1 AND (confirm_expires_at IS NULL OR confirm_expires_at > now())",
    )
    .bind(id)
    .fetch_optional(&st.db)
    .await
    .map_err(retry)?;
    // già confermato, disiscritto, scaduto o cancellato: niente da spedire
    let Some((email, status, blog, Some(nonce))) = row else { return Ok(()) };
    if status != "pending" {
        return Ok(());
    }
    let label: String = match blog {
        Some(b) => sqlx::query_scalar("SELECT title FROM blogs WHERE id = $1")
            .bind(b)
            .fetch_optional(&st.db)
            .await
            .map_err(retry)?
            .unwrap_or_default(),
        None => "Notturni".to_string(),
    };
    let link = format!(
        "https://{}/newsletter/confirm?token={}",
        fqdn(&st),
        tokens::confirm_token(st.config.jwt_secret.as_bytes(), id, nonce)
    );
    let body = format!(
        "Conferma la tua iscrizione alla newsletter di {label} cliccando questo link:\n\n{link}\n\n\
         Il link scade tra {} ore. Se non hai richiesto tu questa iscrizione, ignora pure questa email.",
        crate::CONFIRM_TTL_HOURS
    );
    st.services
        .mailer
        .send(&Email::text(email, format!("Conferma la tua iscrizione alla newsletter di {label}"), body))
        .await
        .map_err(retry)
}

// ---- campagne -------------------------------------------------------------------------------------------------------

struct Content {
    subject: String,
    text: String,
    markdown: String,
}

enum Outcome {
    Send(Content),
    /// Niente da inviare: il post non c'è più, il blog non è raggiungibile…
    Skip(&'static str, &'static str),
}

async fn build_content(
    st: &AppState,
    kind: &str,
    post: Option<Uuid>,
    blog_slug: Option<&str>,
    subject: String,
    body_markdown: Option<String>,
) -> Result<Outcome, sqlx::Error> {
    if kind != "post_notification" {
        let body = body_markdown.unwrap_or_default();
        return Ok(Outcome::Send(Content { subject, text: body.clone(), markdown: body }));
    }
    let (Some(post), Some(blog_slug)) = (post, blog_slug) else {
        return Ok(Outcome::Skip("failed", "notifica senza post"));
    };
    let row: Option<(String, String, String, bool)> = sqlx::query_as(
        "SELECT title, slug, content, (status = 'published' AND NOT is_hidden) FROM posts WHERE id = $1",
    )
    .bind(post)
    .fetch_optional(&st.db)
    .await?;
    let Some((title, slug, content, visible)) = row else { return Ok(Outcome::Skip("failed", "post non trovato")) };
    if !visible {
        return Ok(Outcome::Skip("canceled", "il post non è più pubblicato"));
    }
    let excerpt = content::excerpt(&content);
    let url = format!("https://{}{}", fqdn(st), noct_posts::rules::permalink(blog_slug, &slug));
    Ok(Outcome::Send(Content {
        subject,
        text: format!("{title}\n\n{excerpt}\n\nLeggi tutto: {url}"),
        markdown: format!("### {title}\n\n{excerpt}\n\n[Leggi tutto]({url})"),
    }))
}

async fn finish(db: &sqlx::PgPool, id: Uuid, status: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE newsletter_campaigns SET status = $2::newsletter_campaign_status, updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .execute(db)
    .await
    .map(|_| ())
}

/// Spedisce una campagna agli iscritti confermati della lista. Idempotente: chi ha già ricevuto la
/// campagna (`newsletter_deliveries`) non la riceve di nuovo se l'invio viene ripreso; un lock
/// advisory impedisce a due worker di lavorare sulla stessa campagna.
pub async fn send_campaign(st: AppState, msg: Message) -> Result<(), HandlerError> {
    let id = payload_uuid(&msg, "campaign_id")?;
    let mut lock = st.db.acquire().await.map_err(retry)?;
    let key = format!("newsletter:campaign:{id}");
    let got: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtextextended($1, 0))")
        .bind(&key)
        .fetch_one(&mut *lock)
        .await
        .map_err(retry)?;
    if !got {
        return Err(retry("campagna già in invio da un altro worker"));
    }
    let result = process(&st, id).await;
    // se la connessione cade il lock si rilascia da solo
    let _ = sqlx::query("SELECT pg_advisory_unlock(hashtextextended($1, 0))").bind(&key).execute(&mut *lock).await;
    result
}

type CampaignHead = (String, String, Option<Uuid>, Option<Uuid>, String, Option<String>);

async fn process(st: &AppState, id: Uuid) -> Result<(), HandlerError> {
    let head: Option<CampaignHead> = sqlx::query_as(
        "SELECT kind::text, status::text, blog_id, post_id, subject, body_markdown FROM newsletter_campaigns WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&st.db)
    .await
    .map_err(retry)?;
    // annullata, già inviata, non più in invio o scomparsa (redelivery): niente da fare
    let Some((kind, status, blog, post, subject, body)) = head else { return Ok(()) };
    if status != "sending" {
        return Ok(());
    }
    let (label, slug) = match blog {
        None => ("Notturni".to_string(), None),
        Some(b) => {
            let row: Option<(String, String, bool)> = sqlx::query_as(
                "SELECT title, slug, (NOT is_suspended AND NOT is_paused AND deleted_at IS NULL) FROM blogs WHERE id = $1",
            )
            .bind(b)
            .fetch_optional(&st.db)
            .await
            .map_err(retry)?;
            match row {
                Some((title, slug, true)) => (title, Some(slug)),
                _ => {
                    tracing::info!(campaign = %id, "blog sospeso, in pausa o cancellato: campagna annullata");
                    return finish(&st.db, id, "canceled").await.map_err(retry);
                }
            }
        }
    };
    let content = match build_content(st, &kind, post, slug.as_deref(), subject, body).await.map_err(retry)? {
        Outcome::Send(c) => c,
        Outcome::Skip(status, why) => {
            tracing::warn!(campaign = %id, why, "campagna non inviabile");
            return finish(&st.db, id, status).await.map_err(retry);
        }
    };
    let settings = load_settings(&st.db, blog).await.map_err(retry)?;
    let rendered = render_safe_html(&content.markdown);
    let key = list_key(blog);
    let failed = deliver(st, id, &key, &label, &content, &rendered, &settings).await?;
    let sent: i64 = sqlx::query_scalar("SELECT count(*) FROM newsletter_deliveries WHERE campaign_id = $1")
        .bind(id)
        .fetch_one(&st.db)
        .await
        .map_err(retry)?;
    sqlx::query(
        "UPDATE newsletter_campaigns SET status = 'sent', sent_at = now(), recipient_count = $2, failed_count = $3,
                updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(sent as i32)
    .bind(failed)
    .execute(&st.db)
    .await
    .map_err(retry)?;
    tracing::info!(campaign = %id, recipients = sent, failed, "campagna inviata");
    Ok(())
}

/// Invia a tutti i destinatari non ancora raggiunti; restituisce quanti invii sono falliti.
async fn deliver(
    st: &AppState,
    campaign: Uuid,
    list: &str,
    label: &str,
    content: &Content,
    rendered: &str,
    settings: &Settings,
) -> Result<i32, HandlerError> {
    let secret = st.config.jwt_secret.as_bytes();
    let (mut last, mut failed) = (Uuid::nil(), 0);
    loop {
        // a ogni blocco si rileggono gli iscritti: chi si disiscrive durante l'invio non riceve il resto
        let batch: Vec<(Uuid, String, Option<DateTime<Utc>>)> = sqlx::query_as(
            "SELECT s.id, s.email, s.confirmed_at FROM newsletter_subscribers s
             WHERE s.list_key = $1 AND s.status = 'confirmed' AND s.id > $2
               AND NOT EXISTS (SELECT 1 FROM newsletter_deliveries d WHERE d.campaign_id = $3 AND d.subscriber_id = s.id)
             ORDER BY s.id LIMIT $4",
        )
        .bind(list)
        .bind(last)
        .bind(campaign)
        .bind(BATCH)
        .fetch_all(&st.db)
        .await
        .map_err(retry)?;
        let Some(&(tail, ..)) = batch.last() else { return Ok(failed) };
        last = tail;
        for (subscriber, email, confirmed_at) in batch {
            let link = format!(
                "https://{}/newsletter/unsubscribe?token={}",
                fqdn(st),
                tokens::unsubscribe_token(secret, subscriber)
            );
            let html = content::html_body(
                rendered,
                settings.banner_url.as_deref(),
                &settings.banner_alt_text,
                label,
                confirmed_at,
                &link,
            );
            let text = format!("{}{}", content.text, content::text_footer(label, confirmed_at, &link));
            let email = Email::alternative(email, content.subject.clone(), text, html)
                .with_from_name(settings.sender_name.clone().unwrap_or_else(|| label.to_string()));
            match st.services.mailer.send(&email).await {
                Ok(()) => {
                    // se il worker si ferma qui, alla ripresa questo iscritto non riceve una seconda copia
                    sqlx::query("INSERT INTO newsletter_deliveries (campaign_id, subscriber_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                        .bind(campaign)
                        .bind(subscriber)
                        .execute(&st.db)
                        .await
                        .map_err(retry)?;
                }
                Err(MailError::NotConfigured) => {
                    // un problema dell'intera installazione, non del singolo destinatario: si ferma e si vede
                    tracing::error!(campaign = %campaign, "SMTP non configurato: campagna non inviata");
                    finish(&st.db, campaign, "failed").await.map_err(retry)?;
                    return Err(HandlerError::Permanent("SMTP non configurato".into()));
                }
                Err(e) => {
                    tracing::warn!(campaign = %campaign, error = %e, "invio fallito per un destinatario");
                    failed += 1;
                }
            }
        }
    }
}

// ---- campagne pianificate -------------------------------------------------------------------------------------------

/// Mette in invio le campagne pianificate arrivate a scadenza. Restituisce quante sono partite.
pub async fn dispatch_due_campaigns(conn: &mut PgConnection) -> Result<usize, HandlerError> {
    let due: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE newsletter_campaigns SET status = 'sending', updated_at = now()
         WHERE status = 'scheduled' AND scheduled_at <= now() RETURNING id",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(retry)?;
    for id in &due {
        outbox::enqueue(conn, Kind::Job, SUBJECT_CAMPAIGN, &json!({"campaign_id": id})).await.map_err(retry)?;
    }
    Ok(due.len())
}

/// Job periodico: l'accodamento a intervalli regolari è del pianificatore della Fase 7.
pub async fn dispatch_due(st: AppState, _msg: Message) -> Result<(), HandlerError> {
    let mut tx = st.db.begin().await.map_err(retry)?;
    let n = dispatch_due_campaigns(&mut tx).await?;
    tx.commit().await.map_err(retry)?;
    if n > 0 {
        tracing::info!(campaigns = n, "campagne pianificate messe in invio");
    }
    Ok(())
}
