//! Handler `custom` del modulo.

use crate::{
    AvatarOut, Detail, EMAIL_CODE_TTL_MINUTES, EmailChangeCode, EmailChangeRequestIn, PURPOSE_EMAIL_NEW,
    PURPOSE_EMAIL_OLD, ProfileUpdate, SocialLinkCreate,
    rules::{self, MAX_AVATAR_BYTES, MAX_SOCIAL_LINKS, USERNAME_COOLDOWN_DAYS},
    view::{self, USER_COLUMNS, UserRow},
};
use axum::http::header::CONTENT_TYPE;
use chrono::{DateTime, Duration, Utc};
use noct_audit::{Entry, user_actor};
use noct_core::{
    ApiError, AppState, RouteCtx,
    bus::{Kind, outbox},
    map_db_error, revalidate,
    security::{
        constant_time_eq,
        otp::derive_otp,
        validate::{normalize_email, validate_email, validate_username},
    },
};
use noct_media::upload;
use noct_posts::{PostRow, VISIBLE_SQL, render};
use serde_json::{Value, json};
use sqlx::{PgConnection, Postgres, QueryBuilder};
use uuid::Uuid;

/// Un blog compare negli elenchi pubblici solo se è pubblico, non sospeso, non in pausa e non
/// in cancellazione (alias `b`).
const LISTABLE: &str = "b.visibility = 'public' AND NOT b.is_suspended AND NOT b.is_paused AND b.deleted_at IS NULL";

fn user_id(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    ctx.actor.as_ref().map(|a| a.user_id).ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))
}

fn identifier(ctx: &RouteCtx) -> &str {
    ctx.path.get("username").map(String::as_str).unwrap_or_default()
}

fn int_param(ctx: &RouteCtx, name: &str, default: i64, min: i64, max: i64) -> Result<i64, ApiError> {
    match ctx.query.get(name) {
        None => Ok(default),
        Some(raw) => raw
            .trim()
            .parse::<i64>()
            .map(|n| n.clamp(min, max))
            .map_err(|_| ApiError::unprocessable(format!("`{name}` deve essere un intero."))),
    }
}

/// `%` e `_` del testo cercato sono letterali, non caratteri jolly.
fn like_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

async fn audit(
    conn: &mut PgConnection,
    ctx: &RouteCtx,
    action: &str,
    user: Uuid,
    payload: Value,
) -> Result<(), ApiError> {
    let actor = user_actor(&mut *conn, user).await.map_err(ApiError::internal)?;
    noct_audit::record(
        conn,
        Entry { action, actor, target: Some(("user", user)), blog_id: None, meta: &ctx.meta, payload },
    )
    .await
    .map_err(ApiError::internal)
}

// ---- profilo ------------------------------------------------------------------------------------------------------

pub async fn get_my_profile(ctx: RouteCtx) -> Result<Value, ApiError> {
    let me = view::fetch(&ctx.state, user_id(&ctx)?).await?;
    view::me_json(&ctx.state, &me).await
}

pub async fn get_profile(ctx: RouteCtx) -> Result<Value, ApiError> {
    let u = view::find_public(&ctx.state, identifier(&ctx)).await?;
    view::public_json(&ctx.state, &u).await
}

/// Chiavi degli interessi di piattaforma; vuoto se il modulo è spento (nessun interesse scegliibile).
async fn available_interests(st: &AppState) -> Result<Vec<String>, ApiError> {
    if !st.module_enabled("interests") {
        return Ok(Vec::new());
    }
    let items: Option<Value> =
        sqlx::query_scalar("SELECT items FROM interests_settings WHERE id = 1").fetch_optional(&st.db).await?;
    Ok(items
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|i| i["key"].as_str().map(str::to_string))
        .collect())
}

fn blank_to_none(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

pub async fn update_my_profile(ctx: RouteCtx, body: ProfileUpdate) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let me = user_id(&ctx)?;
    let bad = ApiError::bad_request;

    // validazioni che non dipendono dal database, prima di aprire la transazione
    let ui_locale = body.ui_locale.as_deref().map(rules::normalize_ui_locale).transpose().map_err(bad)?;
    let country = body.country.as_deref().map(rules::normalize_country).transpose().map_err(bad)?;
    let native = body.native_language.as_deref().map(rules::normalize_native_language).transpose().map_err(bad)?;
    let fallback =
        body.fallback_languages.as_deref().map(rules::normalize_fallback_languages).transpose().map_err(bad)?;
    let interests = match &body.interests {
        None => None,
        Some(keys) if keys.is_empty() => Some(Vec::new()),
        Some(keys) => Some(rules::normalize_interests(keys, &available_interests(st).await?).map_err(bad)?),
    };

    let mut tx = st.db.begin().await?;
    let (current, changed_at): (String, Option<DateTime<Utc>>) =
        sqlx::query_as("SELECT username, username_changed_at FROM users WHERE id = $1 FOR UPDATE")
            .bind(me)
            .fetch_one(&mut *tx)
            .await?;

    let mut qb = QueryBuilder::<Postgres>::new("UPDATE users SET updated_at = now()");
    let mut renamed: Option<String> = None;
    if let Some(raw) = &body.username {
        let new = raw.trim().to_lowercase();
        if new != current {
            if let Some(at) = changed_at {
                let ends = at + Duration::days(USERNAME_COOLDOWN_DAYS);
                if Utc::now() < ends {
                    return Err(ApiError::conflict(format!(
                        "Puoi cambiare username al massimo una volta ogni {USERNAME_COOLDOWN_DAYS} giorni. \
                         Prossimo cambio consentito dal {}.",
                        ends.format("%d/%m/%Y")
                    )));
                }
            }
            validate_username(&new).map_err(ApiError::bad_request)?;
            qb.push(", username = ").push_bind(new.clone()).push(", username_changed_at = now()");
            renamed = Some(new);
        }
    }
    if let Some(v) = ui_locale {
        qb.push(", ui_locale = ").push_bind(v);
    }
    if let Some(v) = &body.bio {
        qb.push(", bio = ").push_bind(blank_to_none(v));
    }
    if let Some(v) = &body.first_name {
        qb.push(", first_name = ").push_bind(blank_to_none(v));
    }
    if let Some(v) = &body.last_name {
        qb.push(", last_name = ").push_bind(blank_to_none(v));
    }
    if let Some(v) = &body.display_name {
        qb.push(", display_name = ").push_bind(blank_to_none(v));
    }
    if let Some(v) = &body.post_author_name_style {
        let style = v.trim().to_lowercase();
        if !rules::NAME_STYLES.contains(&style.as_str()) {
            return Err(ApiError::unprocessable(
                "`post_author_name_style` deve essere uno tra: username, full_name, display_name, verified_domain.",
            ));
        }
        qb.push(", post_author_name_style = ").push_bind(style).push("::post_author_name_style");
    }
    if let Some(v) = country {
        qb.push(", country = ").push_bind(v);
    }
    if let Some(v) = native {
        qb.push(", native_language = ").push_bind(v);
    }
    if let Some(v) = fallback {
        qb.push(", fallback_languages = ").push_bind(v);
    }
    if let Some(v) = body.directory_listed {
        qb.push(", directory_listed = ").push_bind(v);
    }
    if let Some(v) = interests {
        qb.push(", interests = ").push_bind(v);
    }
    qb.push(" WHERE id = ").push_bind(me);
    qb.build()
        .execute(&mut *tx)
        .await
        .map_err(|e| map_db_error(e, &[("uq_users_username", 409, "Username già in uso.")]))?;
    let was_renamed = renamed.is_some();
    if let Some(new) = renamed {
        audit(&mut tx, &ctx, "user.username_changed", me, json!({"old_username": current, "new_username": new}))
            .await?;
    }
    tx.commit().await?;
    // dopo una rinomina la pagina col vecchio username non esiste più
    let old_names: Vec<String> = was_renamed.then(|| current.clone()).into_iter().collect();
    revalidate::invalidate_user(st, me, &old_names).await;
    view::me_json(st, &view::fetch(st, me).await?).await
}

// ---- avatar -------------------------------------------------------------------------------------------------------

pub async fn upload_avatar(ctx: RouteCtx) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let me = user_id(&ctx)?;
    let ct = ctx.headers.get(CONTENT_TYPE).and_then(|v| v.to_str().ok());
    let up = upload::read_file(ct, ctx.body.clone()).await?;
    let ext = upload::check_image(&up, upload::FAVICON_TYPES, MAX_AVATAR_BYTES, "PNG, JPEG o WEBP", "2 MiB")?;
    let key = format!("{me}/{}.{ext}", Uuid::new_v4());
    let bucket = &st.config.storage.bucket_avatars;
    st.services.storage.put(bucket, &key, up.data.clone(), &up.content_type).await.map_err(ApiError::internal)?;
    let old: Option<String> =
        sqlx::query_scalar("SELECT avatar_object_key FROM users WHERE id = $1").bind(me).fetch_one(&st.db).await?;
    sqlx::query("UPDATE users SET avatar_object_key = $2, updated_at = now() WHERE id = $1")
        .bind(me)
        .bind(&key)
        .execute(&st.db)
        .await?;
    if let Some(old) = old
        && let Err(e) = st.services.storage.delete(bucket, &old).await
    {
        tracing::warn!(error = %e, old, "cancellazione dell'avatar precedente non riuscita");
    }
    revalidate::invalidate_user(st, me, &[]).await;
    Ok(json!(AvatarOut { avatar_url: view::avatar_url(st, Some(&key)) }))
}

pub async fn delete_avatar(ctx: RouteCtx) -> Result<(), ApiError> {
    let st = &ctx.state;
    let me = user_id(&ctx)?;
    let old: Option<String> =
        sqlx::query_scalar("SELECT avatar_object_key FROM users WHERE id = $1").bind(me).fetch_one(&st.db).await?;
    if let Some(old) = old {
        sqlx::query("UPDATE users SET avatar_object_key = NULL, updated_at = now() WHERE id = $1")
            .bind(me)
            .execute(&st.db)
            .await?;
        if let Err(e) = st.services.storage.delete(&st.config.storage.bucket_avatars, &old).await {
            tracing::warn!(error = %e, old, "cancellazione dell'avatar non riuscita");
        }
        revalidate::invalidate_user(st, me, &[]).await;
    }
    Ok(())
}

// ---- link social --------------------------------------------------------------------------------------------------

pub async fn add_social_link(ctx: RouteCtx, body: SocialLinkCreate) -> Result<Value, ApiError> {
    let me = user_id(&ctx)?;
    let (label, url) = (body.label.trim().to_string(), body.url.trim().to_string());
    rules::validate_social_url(&url).map_err(ApiError::bad_request)?;
    let mut tx = ctx.state.db.begin().await?;
    // la riga utente serializza due aggiunte concorrenti: il tetto non si aggira con due richieste
    sqlx::query("SELECT 1 FROM users WHERE id = $1 FOR UPDATE").bind(me).execute(&mut *tx).await?;
    let (count, next): (i64, i32) =
        sqlx::query_as("SELECT count(*), COALESCE(max(position) + 1, 0) FROM social_links WHERE user_id = $1")
            .bind(me)
            .fetch_one(&mut *tx)
            .await?;
    if count >= MAX_SOCIAL_LINKS {
        return Err(ApiError::bad_request(format!("Massimo {MAX_SOCIAL_LINKS} link social per profilo.")));
    }
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO social_links (user_id, label, url, position) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(me)
    .bind(&label)
    .bind(&url)
    .bind(next)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    revalidate::invalidate_user(&ctx.state, me, &[]).await;
    Ok(json!({"id": id, "label": label, "url": url, "position": next}))
}

pub async fn delete_social_link(ctx: RouteCtx) -> Result<(), ApiError> {
    let me = user_id(&ctx)?;
    let id = ctx.path.get("link_id").and_then(|s| Uuid::parse_str(s).ok());
    let done = match id {
        Some(id) => sqlx::query("DELETE FROM social_links WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(me)
            .execute(&ctx.state.db)
            .await?
            .rows_affected(),
        None => 0,
    };
    if done == 0 {
        return Err(ApiError::not_found("Link non trovato."));
    }
    revalidate::invalidate_user(&ctx.state, me, &[]).await;
    Ok(())
}

// ---- cambio email -------------------------------------------------------------------------------------------------

const MSG_NO_REQUEST: &str = "Nessuna richiesta di cambio email in corso.";
const MSG_BAD_CODE: &str = "Codice non valido o scaduto.";
const MSG_IN_USE: &str = "Email già in uso da un altro account.";

struct Pending {
    id: Uuid,
    new_email: String,
    old_expires_at: DateTime<Utc>,
    old_consumed_at: Option<DateTime<Utc>>,
    new_expires_at: Option<DateTime<Utc>>,
}

type PendingRow = (Uuid, String, DateTime<Utc>, Option<DateTime<Utc>>, Option<DateTime<Utc>>);

async fn pending(conn: &mut PgConnection, user: Uuid) -> Result<Option<Pending>, ApiError> {
    let row: Option<PendingRow> = sqlx::query_as(
        "SELECT id, new_email, old_expires_at, old_consumed_at, new_expires_at FROM email_change_requests
         WHERE user_id = $1 AND completed_at IS NULL ORDER BY created_at DESC, id LIMIT 1 FOR UPDATE",
    )
    .bind(user)
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|(id, new_email, old_expires_at, old_consumed_at, new_expires_at)| Pending {
        id,
        new_email,
        old_expires_at,
        old_consumed_at,
        new_expires_at,
    }))
}

async fn email_in_use(conn: &mut PgConnection, email: &str, except: Uuid) -> Result<bool, ApiError> {
    Ok(sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM users WHERE email = $1 AND id <> $2)")
        .bind(email)
        .bind(except)
        .fetch_one(conn)
        .await?)
}

fn code_matches(st: &AppState, purpose: &str, request: Uuid, code: &str) -> bool {
    constant_time_eq(&derive_otp(st.config.jwt_secret.as_bytes(), purpose, request), code.trim())
}

pub async fn request_email_change(ctx: RouteCtx, body: EmailChangeRequestIn) -> Result<Value, ApiError> {
    let me = user_id(&ctx)?;
    let new_email = normalize_email(&body.new_email);
    validate_email(&new_email).map_err(ApiError::bad_request)?;
    let mut tx = ctx.state.db.begin().await?;
    let current: String =
        sqlx::query_scalar("SELECT email FROM users WHERE id = $1 FOR UPDATE").bind(me).fetch_one(&mut *tx).await?;
    if new_email == current.to_lowercase() {
        return Err(ApiError::bad_request("Il nuovo indirizzo coincide con quello attuale."));
    }
    if email_in_use(&mut tx, &new_email, me).await? {
        return Err(ApiError::bad_request(MSG_IN_USE));
    }
    // una richiesta precedente non completata viene sostituita
    sqlx::query("DELETE FROM email_change_requests WHERE user_id = $1 AND completed_at IS NULL")
        .bind(me)
        .execute(&mut *tx)
        .await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO email_change_requests (user_id, new_email, old_expires_at)
         VALUES ($1, $2, now() + make_interval(mins => $3)) RETURNING id",
    )
    .bind(me)
    .bind(&new_email)
    .bind(EMAIL_CODE_TTL_MINUTES as i32)
    .fetch_one(&mut *tx)
    .await?;
    outbox::enqueue(&mut tx, Kind::Job, "profiles.email_change.send", &json!({"request_id": id, "stage": "old"}))
        .await
        .map_err(ApiError::internal)?;
    tx.commit().await?;
    Ok(json!(Detail { detail: "Codice inviato all'indirizzo email attuale.".into() }))
}

pub async fn verify_current_email(ctx: RouteCtx, body: EmailChangeCode) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let me = user_id(&ctx)?;
    let mut tx = st.db.begin().await?;
    let p = pending(&mut tx, me).await?.ok_or_else(|| ApiError::bad_request(MSG_NO_REQUEST))?;
    if p.old_consumed_at.is_some() {
        return Err(ApiError::bad_request("Passo già completato."));
    }
    if p.old_expires_at <= Utc::now() || !code_matches(st, PURPOSE_EMAIL_OLD, p.id, &body.code) {
        return Err(ApiError::bad_request(MSG_BAD_CODE));
    }
    if email_in_use(&mut tx, &p.new_email, me).await? {
        return Err(ApiError::bad_request(MSG_IN_USE));
    }
    sqlx::query(
        "UPDATE email_change_requests SET old_consumed_at = now(), new_expires_at = now() + make_interval(mins => $2),
                updated_at = now() WHERE id = $1",
    )
    .bind(p.id)
    .bind(EMAIL_CODE_TTL_MINUTES as i32)
    .execute(&mut *tx)
    .await?;
    outbox::enqueue(&mut tx, Kind::Job, "profiles.email_change.send", &json!({"request_id": p.id, "stage": "new"}))
        .await
        .map_err(ApiError::internal)?;
    tx.commit().await?;
    Ok(json!(Detail { detail: "Codice inviato al nuovo indirizzo email.".into() }))
}

pub async fn cancel_email_change(ctx: RouteCtx) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM email_change_requests WHERE user_id = $1 AND completed_at IS NULL")
        .bind(user_id(&ctx)?)
        .execute(&ctx.state.db)
        .await?;
    Ok(())
}

pub async fn verify_new_email(ctx: RouteCtx, body: EmailChangeCode) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let me = user_id(&ctx)?;
    let mut tx = st.db.begin().await?;
    let p = pending(&mut tx, me)
        .await?
        .filter(|p| p.old_consumed_at.is_some())
        .ok_or_else(|| ApiError::bad_request("Nessuna richiesta di cambio email pronta per la conferma finale."))?;
    if p.new_expires_at.is_none_or(|t| t <= Utc::now()) || !code_matches(st, PURPOSE_EMAIL_NEW, p.id, &body.code) {
        return Err(ApiError::bad_request(MSG_BAD_CODE));
    }
    if email_in_use(&mut tx, &p.new_email, me).await? {
        return Err(ApiError::bad_request(MSG_IN_USE));
    }
    let old: String =
        sqlx::query_scalar("SELECT email FROM users WHERE id = $1 FOR UPDATE").bind(me).fetch_one(&mut *tx).await?;
    sqlx::query("UPDATE users SET email = $2, updated_at = now() WHERE id = $1")
        .bind(me)
        .bind(&p.new_email)
        .execute(&mut *tx)
        .await
        .map_err(|e| map_db_error(e, &[("uq_users_email", 400, MSG_IN_USE)]))?;
    sqlx::query(
        "UPDATE email_change_requests SET new_consumed_at = now(), completed_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(p.id)
    .execute(&mut *tx)
    .await?;
    audit(&mut tx, &ctx, "user.email_changed", me, json!({"old_email": old, "new_email": p.new_email})).await?;
    tx.commit().await?;
    view::me_json(st, &view::fetch(st, me).await?).await
}

// ---- directory e viste pubbliche ----------------------------------------------------------------------------------

pub async fn list_users(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let st = &ctx.state;
    let limit = int_param(&ctx, "limit", 30, 1, 100)?;
    let offset = int_param(&ctx, "offset", 0, 0, i64::MAX)?;
    // i follow sono del modulo `discovery`: spento, nessun conteggio e niente ordinamento per follower
    let follows = st.module_enabled("discovery");
    let followers_sql =
        if follows { "(SELECT count(*) FROM user_follows f WHERE f.followed_user_id = u.id)" } else { "0::bigint" };
    // il Super Admin non compare mai nella directory, qualunque sia la sua impostazione
    let mut qb = QueryBuilder::<Postgres>::new(format!(
        "SELECT {USER_COLUMNS}, {followers_sql} AS follower_count FROM users u
         WHERE u.is_active AND u.directory_listed AND u.platform_role <> 'super_admin'"
    ));
    if let Some(q) = ctx.query.get("q").map(|q| q.trim()).filter(|q| !q.is_empty()) {
        let needle = format!("%{}%", like_escape(q));
        qb.push(" AND (u.username ILIKE ").push_bind(needle.clone());
        qb.push(" OR u.display_name ILIKE ").push_bind(needle.clone());
        qb.push(" OR u.bio ILIKE ").push_bind(needle).push(")");
    }
    if let Some(locale) = ctx.query.get("locale").filter(|l| !l.is_empty()) {
        qb.push(" AND u.native_language = ").push_bind(locale.clone());
    }
    if let Some(interest) = ctx.query.get("interest").filter(|i| !i.is_empty()) {
        qb.push(" AND ").push_bind(interest.clone()).push(" = ANY(u.interests)");
    }
    qb.push(match ctx.query.get("sort").map(String::as_str) {
        Some("followers") if follows => " ORDER BY follower_count DESC, u.created_at DESC, u.id",
        _ => " ORDER BY u.created_at DESC, u.id",
    });
    qb.push(" LIMIT ").push_bind(limit).push(" OFFSET ").push_bind(offset);

    #[derive(sqlx::FromRow)]
    struct Listed {
        #[sqlx(flatten)]
        user: UserRow,
        follower_count: i64,
    }
    let rows: Vec<Listed> = qb.build_query_as().fetch_all(&st.db).await?;
    let (fqdn, lists) = (view::instance_fqdn(st), view::verification_lists(st).await?);
    Ok(rows
        .into_iter()
        .map(|Listed { user: u, follower_count }| {
            json!({
                "username": u.username,
                "display_name": u.display_name,
                "bio": u.bio,
                "avatar_url": view::avatar_url(st, u.avatar_object_key.as_deref()),
                "verification_tier": rules::verification_tier(&lists, &fqdn, &u.email, &u.username, u.verified_domain.as_deref()),
                "custom_domain": u.verified_domain,
                "interests": u.interests,
                "follower_count": follower_count,
            })
        })
        .collect())
}

/// Blog pubblici dell'utente. Un blog che si presenta con un alias diverso dallo username non
/// compare: collegherebbe l'alias all'identità reale.
pub async fn list_user_blogs(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let st = &ctx.state;
    let u = view::find_public(st, identifier(&ctx)).await?;
    let ids: Vec<Uuid> = sqlx::query_scalar(&format!(
        "SELECT b.id FROM blogs b WHERE b.owner_id = $1 AND {LISTABLE} ORDER BY b.created_at, b.id"
    ))
    .bind(u.id)
    .fetch_all(&st.db)
    .await?;
    let rows = noct_blogs::view::fetch(st, &ids)
        .await?
        .into_iter()
        .filter(|b| {
            b.default_author_display_name
                .as_deref()
                .is_none_or(|n| n.is_empty() || n.trim().eq_ignore_ascii_case(&u.username))
        })
        .collect();
    noct_blogs::view::render(st, rows, None).await
}

/// Post pubblicati dell'utente su blog pubblici, solo quelli firmati con lo username (mai con un alias).
pub async fn list_user_posts(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let st = &ctx.state;
    let u = view::find_public(st, identifier(&ctx)).await?;
    let limit = int_param(&ctx, "limit", 20, 1, 50)?;
    let offset = int_param(&ctx, "offset", 0, 0, i64::MAX)?;
    let mut qb = QueryBuilder::<Postgres>::new(format!(
        "{} WHERE {VISIBLE_SQL} AND {LISTABLE} AND p.author_id = ",
        PostRow::SELECT
    ));
    // firmati con lo username o con il proprio dominio verificato (entrambi identità pubbliche)
    qb.push_bind(u.id)
        .push(" AND lower(p.author_display_name) IN (lower(")
        .push_bind(u.username.clone())
        .push("), lower(");
    qb.push_bind(u.verified_domain.clone()).push("))");
    qb.push(" ORDER BY p.published_at DESC NULLS LAST, p.id LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);
    let rows: Vec<PostRow> = qb.build_query_as().fetch_all(&st.db).await?;
    render(st, rows).await
}
