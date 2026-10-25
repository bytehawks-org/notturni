//! Handler `custom` del modulo.

use chrono::{DateTime, Duration, NaiveDate, Utc};
use noct_core::{ApiError, BlogRef, RouteCtx, load_blog, membership_role, revalidate};
use noct_posts::{PostRow, VISIBLE_SQL, render};
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder};
use std::collections::HashMap;
use uuid::Uuid;

/// Un blog compare negli elenchi pubblici solo se è pubblico, non sospeso, non in pausa e non
/// in cancellazione (alias `b`).
const LISTABLE: &str = "b.visibility = 'public' AND NOT b.is_suspended AND NOT b.is_paused AND b.deleted_at IS NULL";

const MAX_FEED_LIMIT: i64 = 50;
const DEFAULT_FEED_LIMIT: i64 = 20;
const MAX_LOCALES: i64 = 5;
const MAX_TRENDING_DAYS: i64 = 90;
const MAX_TRENDING_LIMIT: i64 = 30;

fn user_id(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    ctx.actor.as_ref().map(|a| a.user_id).ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))
}

fn blog_of(ctx: &RouteCtx) -> Result<&BlogRef, ApiError> {
    ctx.blog.as_ref().ok_or_else(|| ApiError::internal("scope `blog` assente"))
}

/// Parametro numerico della query, con valore predefinito e limiti; non numerico → 422.
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

fn flag(ctx: &RouteCtx, name: &str) -> bool {
    matches!(ctx.query.get(name).map(|v| v.to_ascii_lowercase()).as_deref(), Some("true" | "1" | "yes"))
}

/// `%` e `_` del testo cercato sono letterali, non caratteri jolly.
fn like_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

// ---- feed e ricerca -----------------------------------------------------------------------------------------------

pub async fn list_feed(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let st = &ctx.state;
    let following = flag(&ctx, "following");
    let viewer = ctx.actor.as_ref().map(|a| a.user_id);
    if following && viewer.is_none() {
        return Err(ApiError::unauthorized("Accedi per vedere i post dei blog che segui."));
    }
    let limit = int_param(&ctx, "limit", DEFAULT_FEED_LIMIT, 1, MAX_FEED_LIMIT)?;
    let offset = int_param(&ctx, "offset", 0, 0, i64::MAX)?;

    let mut qb = QueryBuilder::<Postgres>::new(format!("{} WHERE {VISIBLE_SQL} AND {LISTABLE}", PostRow::SELECT));
    if let Some(locale) = ctx.query.get("locale") {
        qb.push(" AND p.locale = ").push_bind(locale.clone());
    }
    if let Some(tag) = ctx.query.get("tag") {
        qb.push(" AND EXISTS (SELECT 1 FROM post_tags pt JOIN tags t ON t.id = pt.tag_id WHERE pt.post_id = p.id AND t.name = ")
            .push_bind(tag.clone())
            .push(")");
    }
    if let Some(category) = ctx.query.get("category") {
        // la categoria è per-blog: blog diversi con una categoria omonima compaiono insieme
        if st.module_enabled("taxonomy") {
            qb.push(
                " AND EXISTS (SELECT 1 FROM post_categories pc JOIN categories c ON c.id = pc.category_id WHERE pc.post_id = p.id AND c.slug = ",
            )
            .push_bind(category.clone())
            .push(")");
        } else {
            qb.push(" AND false");
        }
    }
    if let (true, Some(me)) = (following, viewer) {
        // i post degli utenti seguiti contano solo se firmati con lo username: un post con alias
        // non deve rivelare, a chi segue la persona, di chi è
        qb.push(" AND (p.blog_id IN (SELECT blog_id FROM blog_follows WHERE follower_id = ")
            .push_bind(me)
            .push(") OR EXISTS (SELECT 1 FROM user_follows uf JOIN users au ON au.id = uf.followed_user_id WHERE uf.follower_id = ")
            .push_bind(me)
            .push(" AND uf.followed_user_id = p.author_id AND lower(p.author_display_name) = lower(au.username)))");
    }
    qb.push(" ORDER BY p.published_at DESC NULLS LAST, p.id LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);
    let rows: Vec<PostRow> = qb.build_query_as().fetch_all(&st.db).await?;
    render(st, rows).await
}

pub async fn list_feed_locales(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let rows: Vec<(String, i64)> = sqlx::query_as(&format!(
        "SELECT p.locale, count(*) AS n FROM posts p JOIN blogs b ON b.id = p.blog_id
         WHERE {VISIBLE_SQL} AND {LISTABLE} GROUP BY p.locale ORDER BY n DESC, p.locale LIMIT $1"
    ))
    .bind(MAX_LOCALES)
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(rows.into_iter().map(|(locale, count)| json!({"locale": locale, "count": count})).collect())
}

pub async fn list_trending_tags(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let days = int_param(&ctx, "days", 7, 1, MAX_TRENDING_DAYS)?;
    let limit = int_param(&ctx, "limit", 10, 1, MAX_TRENDING_LIMIT)?;
    let since = Utc::now() - Duration::days(days);
    let rows: Vec<(String, i64)> = sqlx::query_as(&format!(
        "SELECT t.name, count(*) AS n FROM tags t
         JOIN post_tags pt ON pt.tag_id = t.id JOIN posts p ON p.id = pt.post_id JOIN blogs b ON b.id = p.blog_id
         WHERE {VISIBLE_SQL} AND p.published_at >= $1 AND {LISTABLE}
         GROUP BY t.name ORDER BY n DESC, t.name LIMIT $2"
    ))
    .bind(since)
    .bind(limit)
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(rows.into_iter().map(|(tag, post_count)| json!({"tag": tag, "post_count": post_count})).collect())
}

pub async fn search_posts(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let q = ctx
        .query
        .get("q")
        .map(|q| q.trim())
        .ok_or_else(|| ApiError::unprocessable("Il parametro `q` è obbligatorio."))?;
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let limit = int_param(&ctx, "limit", DEFAULT_FEED_LIMIT, 1, MAX_FEED_LIMIT)?;
    let offset = int_param(&ctx, "offset", 0, 0, i64::MAX)?;
    let needle = format!("%{}%", like_escape(q));
    let rows: Vec<PostRow> = sqlx::query_as(&format!(
        "{} WHERE {VISIBLE_SQL} AND {LISTABLE} AND (p.title ILIKE $1 OR p.content ILIKE $1)
         ORDER BY p.published_at DESC NULLS LAST, p.id LIMIT $2 OFFSET $3",
        PostRow::SELECT
    ))
    .bind(needle)
    .bind(limit)
    .bind(offset)
    .fetch_all(&ctx.state.db)
    .await?;
    render(&ctx.state, rows).await
}

// ---- directory dei blog -----------------------------------------------------------------------------------------

pub async fn list_public_blogs(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let st = &ctx.state;
    let limit = int_param(&ctx, "limit", 30, 1, 100)?;
    let offset = int_param(&ctx, "offset", 0, 0, i64::MAX)?;
    // solo blog indicizzabili, stesso criterio del robots.txt generato
    let mut qb = QueryBuilder::<Postgres>::new(format!(
        "SELECT b.id,
            (SELECT count(*) FROM posts p WHERE p.blog_id = b.id AND {VISIBLE_SQL}) AS post_count,
            (SELECT count(*) FROM blog_follows f WHERE f.blog_id = b.id) AS follower_count,
            (SELECT max(p.published_at) FROM posts p WHERE p.blog_id = b.id AND {VISIBLE_SQL}) AS last_published
         FROM blogs b WHERE {LISTABLE} AND b.search_indexing_enabled"
    ));
    if let Some(q) = ctx.query.get("q").map(|q| q.trim()).filter(|q| !q.is_empty()) {
        let needle = format!("%{}%", like_escape(q));
        qb.push(" AND (b.slug ILIKE ").push_bind(needle.clone()).push(" OR b.title ILIKE ").push_bind(needle.clone());
        qb.push(" OR b.subtitle ILIKE ").push_bind(needle).push(")");
    }
    if let Some(locale) = ctx.query.get("locale").filter(|l| !l.is_empty()) {
        qb.push(" AND b.default_locale = ").push_bind(locale.clone());
    }
    qb.push(match ctx.query.get("sort").map(String::as_str) {
        Some("followers") => " ORDER BY follower_count DESC, b.created_at DESC, b.id",
        Some("new") => " ORDER BY b.created_at DESC, b.id",
        _ => " ORDER BY last_published DESC NULLS LAST, b.created_at DESC, b.id",
    });
    qb.push(" LIMIT ").push_bind(limit).push(" OFFSET ").push_bind(offset);
    let rows: Vec<(Uuid, i64, i64, Option<DateTime<Utc>>)> = qb.build_query_as().fetch_all(&st.db).await?;

    let ids: Vec<Uuid> = rows.iter().map(|r| r.0).collect();
    let blogs = noct_blogs::view::fetch(st, &ids).await?;
    let mut rendered = noct_blogs::view::render(st, blogs, None).await?;
    let mut by_id: HashMap<String, Value> =
        rendered.drain(..).filter_map(|v| v["id"].as_str().map(|id| (id.to_string(), v.clone()))).collect();
    Ok(rows
        .into_iter()
        .filter_map(|(id, posts, followers, last)| {
            let mut v = by_id.remove(&id.to_string())?;
            v["post_count"] = json!(posts);
            v["follower_count"] = json!(followers);
            v["last_published_at"] = json!(last);
            Some(v)
        })
        .collect())
}

// ---- follow -------------------------------------------------------------------------------------------------------------

/// I conteggi dei follower compaiono nella directory dei blog (tag `feed`) e nelle pagine del blog/utente.
fn revalidate_follow(ctx: &RouteCtx, mut tags: Vec<String>) {
    tags.push(revalidate::feed());
    ctx.state.services.revalidate.spawn(tags);
}

pub async fn follow_blog(ctx: RouteCtx) -> Result<(), ApiError> {
    let blog = blog_of(&ctx)?;
    sqlx::query("INSERT INTO blog_follows (follower_id, blog_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(user_id(&ctx)?)
        .bind(blog.id)
        .execute(&ctx.state.db)
        .await?;
    revalidate_follow(&ctx, vec![revalidate::blog(&blog.slug)]);
    Ok(())
}

/// Senza `scope`: chi segue un blog che poi diventa privato deve poter smettere comunque.
pub async fn unfollow_blog(ctx: RouteCtx) -> Result<(), ApiError> {
    let slug = ctx.path.get("slug").map(String::as_str).unwrap_or_default();
    let blog = load_blog(&ctx.state, slug).await?.ok_or_else(|| ApiError::not_found("Blog non trovato."))?;
    sqlx::query("DELETE FROM blog_follows WHERE follower_id = $1 AND blog_id = $2")
        .bind(user_id(&ctx)?)
        .bind(blog.id)
        .execute(&ctx.state.db)
        .await?;
    revalidate_follow(&ctx, vec![revalidate::blog(&blog.slug)]);
    Ok(())
}

fn usernames(rows: Vec<String>) -> Vec<Value> {
    rows.into_iter().map(|u| json!({"username": u})).collect()
}

pub async fn list_blog_followers(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let blog = blog_of(&ctx)?;
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT u.username FROM users u JOIN blog_follows f ON f.follower_id = u.id WHERE f.blog_id = $1 ORDER BY u.username",
    )
    .bind(blog.id)
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(usernames(rows))
}

async fn find_user(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    let username = ctx.path.get("username").map(String::as_str).unwrap_or_default();
    sqlx::query_scalar("SELECT id FROM users WHERE lower(username) = lower($1)")
        .bind(username)
        .fetch_optional(&ctx.state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("Utente non trovato."))
}

pub async fn follow_user(ctx: RouteCtx) -> Result<(), ApiError> {
    let me = user_id(&ctx)?;
    let target = find_user(&ctx).await?;
    if target == me {
        return Err(ApiError::bad_request("Non puoi seguire te stesso."));
    }
    sqlx::query("INSERT INTO user_follows (follower_id, followed_user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(me)
        .bind(target)
        .execute(&ctx.state.db)
        .await?;
    revalidate::invalidate_user(&ctx.state, target, &[]).await;
    Ok(())
}

pub async fn unfollow_user(ctx: RouteCtx) -> Result<(), ApiError> {
    let me = user_id(&ctx)?;
    let target = find_user(&ctx).await?;
    sqlx::query("DELETE FROM user_follows WHERE follower_id = $1 AND followed_user_id = $2")
        .bind(me)
        .bind(target)
        .execute(&ctx.state.db)
        .await?;
    revalidate::invalidate_user(&ctx.state, target, &[]).await;
    Ok(())
}

pub async fn list_user_followers(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let target = find_user(&ctx).await?;
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT u.username FROM users u JOIN user_follows f ON f.follower_id = u.id WHERE f.followed_user_id = $1 ORDER BY u.username",
    )
    .bind(target)
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(usernames(rows))
}

pub async fn list_user_following(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let target = find_user(&ctx).await?;
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT u.username FROM users u JOIN user_follows f ON f.followed_user_id = u.id WHERE f.follower_id = $1 ORDER BY u.username",
    )
    .bind(target)
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(usernames(rows))
}

/// Quante persone seguono l'utente in tutto: chi lo segue con lo username reale e chi segue un suo
/// blog, anche quando il blog appare con un alias. Riservato al proprietario: è l'unico posto
/// dove le due cose vengono messe insieme.
pub async fn my_follow_stats(ctx: RouteCtx) -> Result<Value, ApiError> {
    let me = user_id(&ctx)?;
    let user_followers: i64 = sqlx::query_scalar("SELECT count(*) FROM user_follows WHERE followed_user_id = $1")
        .bind(me)
        .fetch_one(&ctx.state.db)
        .await?;
    let rows: Vec<(String, String, Option<String>, i64)> = sqlx::query_as(
        "SELECT b.slug, b.title, b.default_author_display_name,
                (SELECT count(*) FROM blog_follows f WHERE f.blog_id = b.id)
         FROM blogs b WHERE b.owner_id = $1 ORDER BY b.title, b.id",
    )
    .bind(me)
    .fetch_all(&ctx.state.db)
    .await?;
    let total = user_followers + rows.iter().map(|r| r.3).sum::<i64>();
    let blogs: Vec<Value> = rows
        .into_iter()
        .map(|(slug, title, alias, n)| json!({"blog_slug": slug, "blog_title": title, "alias": alias, "followers": n}))
        .collect();
    Ok(json!({"user_followers": user_followers, "blogs": blogs, "total_followers": total}))
}

// ---- gestione del blog --------------------------------------------------------------------------------------------

pub async fn blog_overview(ctx: RouteCtx) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let blog = blog_of(&ctx)?;
    let me = user_id(&ctx)?;
    if blog.owner_id != me && membership_role(st, me, blog.id).await?.is_none() {
        return Err(ApiError::forbidden("Serve essere proprietario o membro del blog."));
    }
    let (total, published, scheduled, draft, in_review, last): (i64, i64, i64, i64, i64, Option<DateTime<Utc>>) =
        sqlx::query_as(
            "SELECT count(*),
                    count(*) FILTER (WHERE status = 'published' AND published_at <= now()),
                    count(*) FILTER (WHERE status = 'published' AND published_at > now()),
                    count(*) FILTER (WHERE status = 'draft'),
                    count(*) FILTER (WHERE status = 'pending_review'),
                    max(published_at) FILTER (WHERE status = 'published' AND published_at <= now())
             FROM posts WHERE blog_id = $1",
        )
        .bind(blog.id)
        .fetch_one(&st.db)
        .await?;

    let today = Utc::now().date_naive();
    let since: NaiveDate = today - Duration::days(29);
    let by_day: HashMap<NaiveDate, i64> = sqlx::query_as::<_, (NaiveDate, i64)>(
        "SELECT r.day, sum(r.reads)::bigint FROM post_reads_daily r JOIN posts p ON p.id = r.post_id
         WHERE p.blog_id = $1 AND r.day >= $2 GROUP BY r.day",
    )
    .bind(blog.id)
    .bind(since)
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .collect();
    let reads: Vec<(NaiveDate, i64)> =
        (0..30).map(|i| since + Duration::days(i)).map(|d| (d, by_day.get(&d).copied().unwrap_or(0))).collect();

    let followers: i64 = sqlx::query_scalar("SELECT count(*) FROM blog_follows WHERE blog_id = $1")
        .bind(blog.id)
        .fetch_one(&st.db)
        .await?;
    let members: i64 = sqlx::query_scalar("SELECT count(*) FROM blog_memberships WHERE blog_id = $1")
        .bind(blog.id)
        .fetch_one(&st.db)
        .await?;
    let media: i64 =
        sqlx::query_scalar("SELECT count(*) FROM post_media pm JOIN posts p ON p.id = pm.post_id WHERE p.blog_id = $1")
            .bind(blog.id)
            .fetch_one(&st.db)
            .await?;
    // i commenti sono di un modulo opzionale
    let (pending, approved): (i64, i64) = if st.module_enabled("comments") {
        sqlx::query_as(
            "SELECT count(*) FILTER (WHERE c.status = 'pending'), count(*) FILTER (WHERE c.status = 'approved')
             FROM comments c JOIN posts p ON p.id = c.post_id WHERE p.blog_id = $1",
        )
        .bind(blog.id)
        .fetch_one(&st.db)
        .await?
    } else {
        (0, 0)
    };
    let storage: i64 =
        sqlx::query_scalar("SELECT coalesce(sum(size_bytes), 0)::bigint FROM media_files WHERE blog_id = $1")
            .bind(blog.id)
            .fetch_one(&st.db)
            .await?;
    let platform = noct_platform::get(&st.db).await?;
    Ok(json!({
        "posts_total": total, "posts_published": published, "posts_scheduled": scheduled,
        "posts_draft": draft, "posts_in_review": in_review, "followers": followers, "members": members,
        "pending_comments": pending, "approved_comments": approved, "media": media,
        "last_published_at": last,
        "reads_30d": reads.iter().map(|(d, n)| json!({"day": d.to_string(), "reads": n})).collect::<Vec<_>>(),
        "reads_total_30d": reads.iter().map(|(_, n)| n).sum::<i64>(),
        "storage_bytes": storage, "storage_limit_mb": platform.max_blog_storage_mb,
    }))
}

pub async fn list_mentionable_users(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let st = &ctx.state;
    let blog = blog_of(&ctx)?;
    let mentions: bool =
        sqlx::query_scalar("SELECT mentions_enabled FROM blogs WHERE id = $1").bind(blog.id).fetch_one(&st.db).await?;
    if !mentions {
        return Ok(Vec::new());
    }
    let limit = int_param(&ctx, "limit", 8, 1, 25)?;
    let prefix = like_escape(
        ctx.query.get("q").map(|q| q.trim().trim_start_matches('@').to_lowercase()).unwrap_or_default().as_str(),
    );
    let rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT u.username, u.display_name, u.avatar_object_key FROM users u
         WHERE u.id IN (SELECT user_id FROM blog_memberships WHERE blog_id = $1
                        UNION SELECT follower_id FROM blog_follows WHERE blog_id = $1
                        UNION SELECT owner_id FROM blogs WHERE id = $1)
           AND ($2 = '' OR lower(u.username) LIKE $2 || '%' OR lower(coalesce(u.display_name, '')) LIKE '%' || $2 || '%')
         ORDER BY u.username LIMIT $3",
    )
    .bind(blog.id)
    .bind(&prefix)
    .bind(limit)
    .fetch_all(&st.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(username, display_name, avatar)| {
            let url = avatar.map(|k| st.services.storage.public_url(&st.config.storage.bucket_avatars, &k));
            json!({"username": username, "display_name": display_name, "avatar_url": url})
        })
        .collect())
}

// ---- letture ------------------------------------------------------------------------------------------------------------

/// Conta una lettura: pubblico, senza sessione né cookie, nessun dato del lettore persistito (solo
/// `+1` su post e giorno UTC). Per non contare i ricaricamenti ravvicinati c'è un limite per IP e
/// post nella cache volatile: oltre il limite la richiesta è accettata ma non contata.
pub async fn record_post_read(ctx: RouteCtx) -> Result<(), ApiError> {
    let st = &ctx.state;
    let post_id = noct_core::parse_uuid(ctx.path.get("post_id").map(String::as_str).unwrap_or_default())?;
    let visible: Option<bool> = sqlx::query_scalar(&format!(
        "SELECT ({VISIBLE_SQL}) AND ({LISTABLE}) FROM posts p JOIN blogs b ON b.id = p.blog_id WHERE p.id = $1"
    ))
    .bind(post_id)
    .fetch_optional(&st.db)
    .await?;
    if visible != Some(true) {
        return Err(ApiError::not_found("Post non trovato."));
    }
    let ip = ctx.meta.ip.as_deref().unwrap_or("unknown");
    let key = format!("read:{ip}:{post_id}");
    if st.limiter.check(&key, 1, std::time::Duration::from_secs(6 * 3600)).await.is_err() {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO post_reads_daily (post_id, day, reads) VALUES ($1, (now() AT TIME ZONE 'UTC')::date, 1)
         ON CONFLICT (post_id, day) DO UPDATE SET reads = post_reads_daily.reads + 1",
    )
    .bind(post_id)
    .execute(&st.db)
    .await?;
    Ok(())
}
