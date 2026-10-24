//! Handler `custom` del modulo.

use crate::{
    AdminNewsletterSettingsUpdate, CONFIRM_TTL_HOURS, CampaignCreate, CampaignUpdate, NewsletterSettingsUpdate,
    StatusOut, SubscribeRequest, TokenRequest, UnsubscribeRequest, list_key, tokens,
};
use chrono::{DateTime, Utc};
use noct_core::{
    ApiError, AppState, JsonMap, Patch, RouteCtx, body_map,
    bus::{Kind, outbox},
    can_view_blog, load_blog, parse_uuid,
    security::validate::{normalize_email, validate_email},
};
use serde_json::{Value, json};
use sqlx::{PgConnection, Postgres, QueryBuilder};
use std::time::Duration as StdDuration;
use uuid::Uuid;

const EMAIL_LIMIT: u32 = 3;
const EMAIL_WINDOW: StdDuration = StdDuration::from_secs(3600);
const MAX_USER_AGENT: usize = 512;

pub(crate) const SUBJECT_CONFIRMATION: &str = "newsletter.confirmation.send";
pub(crate) const SUBJECT_CAMPAIGN: &str = "newsletter.campaign.send";

fn ok() -> StatusOut {
    StatusOut { status: "ok".into() }
}

fn user_id(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    ctx.actor.as_ref().map(|a| a.user_id).ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))
}

fn secret(st: &AppState) -> &[u8] {
    st.config.jwt_secret.as_bytes()
}

// ---- impostazioni (blog o digest) ---------------------------------------------------------------------------------

pub(crate) struct Settings {
    pub auto_notify_enabled: bool,
    pub sender_name: Option<String>,
    pub banner_url: Option<String>,
    pub banner_alt_text: String,
}

/// Senza riga valgono i default: invio automatico attivo, nessun banner né nome del mittente.
pub(crate) async fn load_settings(db: &sqlx::PgPool, blog: Option<Uuid>) -> Result<Settings, sqlx::Error> {
    let row: Option<(bool, Option<String>, Option<String>, String)> =
        match blog {
            Some(id) => {
                sqlx::query_as(
                    "SELECT auto_notify_enabled, sender_name, banner_url, banner_alt_text
                 FROM newsletter_blog_settings WHERE blog_id = $1",
                )
                .bind(id)
                .fetch_optional(db)
                .await?
            }
            None => sqlx::query_as(
                "SELECT true, sender_name, banner_url, banner_alt_text FROM newsletter_platform_settings WHERE id = 1",
            )
            .fetch_optional(db)
            .await?,
        };
    Ok(row
        .map(|(auto_notify_enabled, sender_name, banner_url, banner_alt_text)| Settings {
            auto_notify_enabled,
            sender_name,
            banner_url,
            banner_alt_text,
        })
        .unwrap_or(Settings {
            auto_notify_enabled: true,
            sender_name: None,
            banner_url: None,
            banner_alt_text: String::new(),
        }))
}

fn settings_json(s: &Settings, with_auto: bool) -> Value {
    let mut v = json!({
        "newsletter_sender_name": s.sender_name,
        "newsletter_banner_url": s.banner_url,
        "newsletter_banner_alt_text": s.banner_alt_text,
    });
    if with_auto {
        v["newsletter_auto_notify_enabled"] = json!(s.auto_notify_enabled);
    }
    v
}

/// Un campo assente dal corpo resta invariato, `null` lo azzera: i tipi delle rotte `custom` non
/// distinguono i due casi, quindi si guarda il corpo grezzo.
fn tri<T: Clone>(body: &JsonMap, key: &str, value: &Option<T>) -> Patch<T> {
    match (body.contains_key(key), value) {
        (false, _) => Patch::Absent,
        (true, None) => Patch::Null,
        (true, Some(v)) => Patch::Value(v.clone()),
    }
}

fn clean_text(patch: &Patch<String>) -> Option<Option<String>> {
    match patch {
        Patch::Absent => None,
        Patch::Null => Some(None),
        Patch::Value(v) => Some(Some(v.trim().to_string()).filter(|t| !t.is_empty())),
    }
}

/// Il banner finisce in un `<img src>` dell'email: solo URL http(s).
fn clean_banner_url(patch: &Patch<String>) -> Result<Option<Option<String>>, ApiError> {
    let cleaned = clean_text(patch);
    if let Some(Some(url)) = &cleaned
        && !(url.starts_with("http://") || url.starts_with("https://"))
    {
        return Err(ApiError::bad_request("L'URL del banner deve iniziare con http:// o https://."));
    }
    Ok(cleaned)
}

async fn patch_settings(
    st: &AppState,
    blog: Option<Uuid>,
    auto: Option<bool>,
    sender: &Patch<String>,
    banner_url: &Patch<String>,
    banner_alt: &Patch<String>,
) -> Result<Settings, ApiError> {
    let (sender, url) = (clean_text(sender), clean_banner_url(banner_url)?);
    let alt = clean_text(banner_alt);
    let mut tx = st.db.begin().await?;
    match blog {
        Some(id) => {
            sqlx::query("INSERT INTO newsletter_blog_settings (blog_id) VALUES ($1) ON CONFLICT DO NOTHING")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        None => {
            sqlx::query("INSERT INTO newsletter_platform_settings (id) VALUES (1) ON CONFLICT DO NOTHING")
                .execute(&mut *tx)
                .await?;
        }
    }
    let table = if blog.is_some() { "newsletter_blog_settings" } else { "newsletter_platform_settings" };
    let mut qb = QueryBuilder::<Postgres>::new(format!("UPDATE {table} SET updated_at = now()"));
    if let Some(v) = auto {
        qb.push(", auto_notify_enabled = ").push_bind(v);
    }
    if let Some(v) = sender {
        qb.push(", sender_name = ").push_bind(v);
    }
    if let Some(v) = url {
        qb.push(", banner_url = ").push_bind(v);
    }
    if let Some(v) = alt {
        qb.push(", banner_alt_text = ").push_bind(v.unwrap_or_default());
    }
    match blog {
        Some(id) => qb.push(" WHERE blog_id = ").push_bind(id),
        None => qb.push(" WHERE id = 1"),
    };
    qb.build().execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(load_settings(&st.db, blog).await?)
}

pub async fn get_blog_settings(ctx: RouteCtx) -> Result<Value, ApiError> {
    let blog = ctx.blog.as_ref().ok_or_else(|| ApiError::internal("scope `blog` assente"))?;
    Ok(settings_json(&load_settings(&ctx.state.db, Some(blog.id)).await?, true))
}

pub async fn update_blog_settings(ctx: RouteCtx, body: NewsletterSettingsUpdate) -> Result<Value, ApiError> {
    let blog = ctx.blog.as_ref().ok_or_else(|| ApiError::internal("scope `blog` assente"))?;
    let raw = body_map(&ctx.body);
    let s = patch_settings(
        &ctx.state,
        Some(blog.id),
        body.newsletter_auto_notify_enabled,
        &tri(&raw, "newsletter_sender_name", &body.newsletter_sender_name),
        &tri(&raw, "newsletter_banner_url", &body.newsletter_banner_url),
        &tri(&raw, "newsletter_banner_alt_text", &body.newsletter_banner_alt_text),
    )
    .await?;
    Ok(settings_json(&s, true))
}

pub async fn get_admin_settings(ctx: RouteCtx) -> Result<Value, ApiError> {
    Ok(settings_json(&load_settings(&ctx.state.db, None).await?, false))
}

pub async fn update_admin_settings(ctx: RouteCtx, body: AdminNewsletterSettingsUpdate) -> Result<Value, ApiError> {
    let raw = body_map(&ctx.body);
    let s = patch_settings(
        &ctx.state,
        None,
        None,
        &tri(&raw, "newsletter_sender_name", &body.newsletter_sender_name),
        &tri(&raw, "newsletter_banner_url", &body.newsletter_banner_url),
        &tri(&raw, "newsletter_banner_alt_text", &body.newsletter_banner_alt_text),
    )
    .await?;
    Ok(settings_json(&s, false))
}

// ---- pubbliche ----------------------------------------------------------------------------------------------------

pub async fn subscribe(ctx: RouteCtx, body: SubscribeRequest) -> Result<StatusOut, ApiError> {
    let st = &ctx.state;
    let email = normalize_email(&body.email);
    validate_email(&email).map_err(ApiError::bad_request)?;
    let locale = match body.locale.as_deref().map(str::trim).filter(|l| !l.is_empty()) {
        Some(l) => {
            let l = l.to_lowercase();
            noct_blogs::rules::validate_locale(&l).map_err(ApiError::bad_request)?;
            Some(l)
        }
        None => None,
    };
    // oltre al limite per IP della rotta: chi vuole spedire email a un indirizzo altrui da più IP si ferma qui
    st.limiter
        .check(&format!("newsletter:subscribe:email:{email}"), EMAIL_LIMIT, EMAIL_WINDOW)
        .await
        .map_err(ApiError::too_many_requests)?;

    let viewer = ctx.actor.as_ref().map(|a| a.user_id);
    let blog_id = match body.blog_slug.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(slug) => {
            let not_found = || ApiError::not_found("Blog non trovato.");
            let blog = load_blog(st, slug).await?.ok_or_else(not_found)?;
            if !can_view_blog(st, viewer, &blog).await? {
                return Err(not_found());
            }
            Some(blog.id)
        }
        None => None,
    };

    let nonce = Uuid::new_v4();
    let user_agent = ctx.meta.user_agent.as_deref().map(|u| u.chars().take(MAX_USER_AGENT).collect::<String>());
    let mut tx = st.db.begin().await?;
    // un iscritto già confermato non cambia (la `WHERE` non fa scattare l'aggiornamento) e la risposta
    // resta identica: nessun segnale su chi è già in lista
    let id: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO newsletter_subscribers
            (list_key, blog_id, email, locale, user_id, confirm_nonce, confirm_expires_at, consent_ip, consent_user_agent)
         VALUES ($1, $2, $3, $4, $5, $6, now() + make_interval(hours => $7), $8, $9)
         ON CONFLICT ON CONSTRAINT uq_newsletter_subscriber_list_email DO UPDATE SET
            status = 'pending',
            locale = COALESCE(EXCLUDED.locale, newsletter_subscribers.locale),
            user_id = COALESCE(EXCLUDED.user_id, newsletter_subscribers.user_id),
            confirm_nonce = EXCLUDED.confirm_nonce, confirm_expires_at = EXCLUDED.confirm_expires_at,
            consent_ip = EXCLUDED.consent_ip, consent_user_agent = EXCLUDED.consent_user_agent, updated_at = now()
         WHERE newsletter_subscribers.status <> 'confirmed'
         RETURNING id",
    )
    .bind(list_key(blog_id))
    .bind(blog_id)
    .bind(&email)
    .bind(locale)
    .bind(viewer)
    .bind(nonce)
    .bind(CONFIRM_TTL_HOURS as i32)
    .bind(ctx.meta.ip.clone())
    .bind(user_agent)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(id) = id {
        outbox::enqueue(&mut tx, Kind::Job, SUBJECT_CONFIRMATION, &json!({"subscriber_id": id}))
            .await
            .map_err(ApiError::internal)?;
    }
    tx.commit().await?;
    Ok(ok())
}

pub async fn confirm_subscription(ctx: RouteCtx) -> Result<StatusOut, ApiError> {
    let st = &ctx.state;
    let token = ctx.query.get("token").ok_or_else(|| ApiError::unprocessable("Parametro `token` mancante."))?;
    let status = |s: &str| Ok(StatusOut { status: s.into() });
    let Some(id) = tokens::confirm_token_subject(token) else { return status("invalid") };
    let mut tx = st.db.begin().await?;
    let row: Option<(String, Option<Uuid>, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT status::text, confirm_nonce, confirm_expires_at FROM newsletter_subscribers WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((current, Some(nonce), expires)) = row else { return status("invalid") };
    if !tokens::verify_confirm_token(secret(st), token, nonce) {
        return status("invalid");
    }
    match current.as_str() {
        // il secondo clic dello stesso link (doppio clic, client che precarica i link) è il caso normale
        "confirmed" => return status("already_confirmed"),
        // chi si è disiscritto non si ri-iscrive con un vecchio link: serve una nuova richiesta
        "pending" if expires.is_none_or(|t| t > Utc::now()) => {}
        _ => return status("invalid"),
    }
    sqlx::query("UPDATE newsletter_subscribers SET status = 'confirmed', confirmed_at = now(), updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    status("confirmed")
}

pub async fn unsubscribe(ctx: RouteCtx, body: UnsubscribeRequest) -> Result<StatusOut, ApiError> {
    let id = tokens::verify_unsubscribe_token(secret(&ctx.state), &body.token)
        .ok_or_else(|| ApiError::bad_request("Link di disiscrizione non valido."))?;
    let reason = body.reason.as_deref().map(str::trim).filter(|r| !r.is_empty());
    // idempotente: un link valido per un'iscrizione già disiscritta o cancellata risponde comunque ok
    sqlx::query(
        "UPDATE newsletter_subscribers SET status = 'unsubscribed', unsubscribed_at = now(), unsubscribe_reason = $2,
                confirm_nonce = NULL, updated_at = now()
         WHERE id = $1 AND status <> 'unsubscribed'",
    )
    .bind(id)
    .bind(reason)
    .execute(&ctx.state.db)
    .await?;
    Ok(ok())
}

pub async fn unsubscribe_and_delete(ctx: RouteCtx, body: TokenRequest) -> Result<StatusOut, ApiError> {
    let id = tokens::verify_unsubscribe_token(secret(&ctx.state), &body.token)
        .ok_or_else(|| ApiError::bad_request("Link non valido."))?;
    sqlx::query("DELETE FROM newsletter_subscribers WHERE id = $1").bind(id).execute(&ctx.state.db).await?;
    Ok(ok())
}

// ---- statistiche e campagne (blog o digest) -----------------------------------------------------------------------

async fn stats(st: &AppState, blog: Option<Uuid>) -> Result<Value, ApiError> {
    let rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT status::text, count(*) FROM newsletter_subscribers WHERE list_key = $1 GROUP BY status")
            .bind(list_key(blog))
            .fetch_all(&st.db)
            .await?;
    let count = |s: &str| rows.iter().find(|(k, _)| k == s).map(|(_, n)| *n).unwrap_or(0);
    Ok(json!({"pending": count("pending"), "confirmed": count("confirmed"), "unsubscribed": count("unsubscribed")}))
}

const CAMPAIGN_COLUMNS: &str = "id, blog_id, kind::text, post_id, subject, body_markdown, status::text, scheduled_at, \
    sent_at, recipient_count::bigint, failed_count::bigint, created_at";

type CampaignRow = (
    Uuid,
    Option<Uuid>,
    String,
    Option<Uuid>,
    String,
    Option<String>,
    String,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    i64,
    i64,
    DateTime<Utc>,
);

fn campaign_json(r: CampaignRow) -> Value {
    json!({
        "id": r.0, "blog_id": r.1, "kind": r.2, "post_id": r.3, "subject": r.4, "body_markdown": r.5, "status": r.6,
        "scheduled_at": r.7, "sent_at": r.8, "recipient_count": r.9, "failed_count": r.10, "created_at": r.11,
    })
}

async fn campaigns(st: &AppState, blog: Option<Uuid>) -> Result<Vec<Value>, ApiError> {
    let rows: Vec<CampaignRow> = sqlx::query_as(&format!(
        "SELECT {CAMPAIGN_COLUMNS} FROM newsletter_campaigns WHERE blog_id IS NOT DISTINCT FROM $1
         ORDER BY created_at DESC, id"
    ))
    .bind(blog)
    .fetch_all(&st.db)
    .await?;
    Ok(rows.into_iter().map(campaign_json).collect())
}

async fn campaign_by_id(conn: &mut PgConnection, id: Uuid) -> Result<Value, ApiError> {
    let row: CampaignRow =
        sqlx::query_as(&format!("SELECT {CAMPAIGN_COLUMNS} FROM newsletter_campaigns WHERE id = $1"))
            .bind(id)
            .fetch_one(conn)
            .await?;
    Ok(campaign_json(row))
}

fn is_immediate(at: Option<DateTime<Utc>>) -> bool {
    at.is_none_or(|t| t <= Utc::now())
}

async fn create_campaign(ctx: &RouteCtx, blog: Option<Uuid>, body: CampaignCreate) -> Result<Value, ApiError> {
    let me = user_id(ctx)?;
    let immediate = is_immediate(body.scheduled_at);
    let mut tx = ctx.state.db.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO newsletter_campaigns (blog_id, kind, created_by_id, subject, body_markdown, status, scheduled_at)
         VALUES ($1, 'manual', $2, $3, $4, $5::newsletter_campaign_status, $6) RETURNING id",
    )
    .bind(blog)
    .bind(me)
    .bind(body.subject.trim())
    .bind(&body.body_markdown)
    .bind(if immediate { "sending" } else { "scheduled" })
    .bind(body.scheduled_at)
    .fetch_one(&mut *tx)
    .await?;
    if immediate {
        outbox::enqueue(&mut tx, Kind::Job, SUBJECT_CAMPAIGN, &json!({"campaign_id": id}))
            .await
            .map_err(ApiError::internal)?;
    }
    let out = campaign_by_id(&mut tx, id).await?;
    tx.commit().await?;
    Ok(out)
}

/// La campagna `id` della lista, bloccata per la modifica; `404` se è di un'altra lista.
async fn lock_campaign(conn: &mut PgConnection, blog: Option<Uuid>, id: Uuid) -> Result<(String, String), ApiError> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT status::text, kind::text FROM newsletter_campaigns WHERE id = $1 AND blog_id IS NOT DISTINCT FROM $2 FOR UPDATE",
    )
    .bind(id)
    .bind(blog)
    .fetch_optional(conn)
    .await?
    .ok_or_else(|| ApiError::not_found("Campagna non trovata."))
}

async fn update_campaign(ctx: &RouteCtx, blog: Option<Uuid>, body: CampaignUpdate) -> Result<Value, ApiError> {
    let id = parse_uuid(ctx.path.get("campaign_id").map(String::as_str).unwrap_or_default())?;
    let mut tx = ctx.state.db.begin().await?;
    let (status, kind) = lock_campaign(&mut tx, blog, id).await?;
    // dopo l'avvio il worker può averla già presa in carico: non c'è una finestra sicura per intercettarla
    if status != "scheduled" {
        return Err(ApiError::conflict(
            "Solo una campagna pianificata (non ancora in invio) può essere modificata o annullata.",
        ));
    }
    if kind != "manual" {
        return Err(ApiError::conflict("Una notifica automatica non è modificabile: si può solo annullare."));
    }
    let mut qb = QueryBuilder::<Postgres>::new("UPDATE newsletter_campaigns SET updated_at = now()");
    if let Some(v) = &body.subject {
        qb.push(", subject = ").push_bind(v.trim().to_string());
    }
    if let Some(v) = &body.body_markdown {
        qb.push(", body_markdown = ").push_bind(v.clone());
    }
    let mut immediate = false;
    let scheduled = tri(&body_map(&ctx.body), "scheduled_at", &body.scheduled_at);
    match &scheduled {
        Patch::Absent => {}
        Patch::Null => immediate = true,
        Patch::Value(t) => immediate = is_immediate(Some(*t)),
    }
    match &scheduled {
        Patch::Absent => {}
        Patch::Null => {
            qb.push(", scheduled_at = NULL");
        }
        Patch::Value(t) => {
            qb.push(", scheduled_at = ").push_bind(*t);
        }
    }
    if immediate {
        qb.push(", status = 'sending'");
    }
    qb.push(" WHERE id = ").push_bind(id);
    qb.build().execute(&mut *tx).await?;
    if immediate {
        outbox::enqueue(&mut tx, Kind::Job, SUBJECT_CAMPAIGN, &json!({"campaign_id": id}))
            .await
            .map_err(ApiError::internal)?;
    }
    let out = campaign_by_id(&mut tx, id).await?;
    tx.commit().await?;
    Ok(out)
}

async fn cancel_campaign(ctx: &RouteCtx, blog: Option<Uuid>) -> Result<Value, ApiError> {
    let id = parse_uuid(ctx.path.get("campaign_id").map(String::as_str).unwrap_or_default())?;
    let mut tx = ctx.state.db.begin().await?;
    let (status, _) = lock_campaign(&mut tx, blog, id).await?;
    if status != "scheduled" {
        return Err(ApiError::conflict(
            "Solo una campagna pianificata (non ancora in invio) può essere modificata o annullata.",
        ));
    }
    sqlx::query("UPDATE newsletter_campaigns SET status = 'canceled', updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let out = campaign_by_id(&mut tx, id).await?;
    tx.commit().await?;
    Ok(out)
}

fn blog_id(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    ctx.blog.as_ref().map(|b| b.id).ok_or_else(|| ApiError::internal("scope `blog` assente"))
}

pub async fn blog_stats(ctx: RouteCtx) -> Result<Value, ApiError> {
    stats(&ctx.state, Some(blog_id(&ctx)?)).await
}
pub async fn blog_campaigns(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    campaigns(&ctx.state, Some(blog_id(&ctx)?)).await
}
pub async fn create_blog_campaign(ctx: RouteCtx, body: CampaignCreate) -> Result<Value, ApiError> {
    create_campaign(&ctx, Some(blog_id(&ctx)?), body).await
}
pub async fn update_blog_campaign(ctx: RouteCtx, body: CampaignUpdate) -> Result<Value, ApiError> {
    update_campaign(&ctx, Some(blog_id(&ctx)?), body).await
}
pub async fn cancel_blog_campaign(ctx: RouteCtx) -> Result<Value, ApiError> {
    cancel_campaign(&ctx, Some(blog_id(&ctx)?)).await
}

pub async fn admin_stats(ctx: RouteCtx) -> Result<Value, ApiError> {
    stats(&ctx.state, None).await
}
pub async fn admin_campaigns(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    campaigns(&ctx.state, None).await
}
pub async fn create_admin_campaign(ctx: RouteCtx, body: CampaignCreate) -> Result<Value, ApiError> {
    create_campaign(&ctx, None, body).await
}
pub async fn update_admin_campaign(ctx: RouteCtx, body: CampaignUpdate) -> Result<Value, ApiError> {
    update_campaign(&ctx, None, body).await
}
pub async fn cancel_admin_campaign(ctx: RouteCtx) -> Result<Value, ApiError> {
    cancel_campaign(&ctx, None).await
}
