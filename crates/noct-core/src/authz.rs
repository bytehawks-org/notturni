use crate::{ApiError, AppState, Policy, RateKey, RouteSpec};
use axum::http::HeaderMap;
use chrono::{DateTime, Utc};
use sqlx::Row;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Actor {
    pub user_id: Uuid,
    pub platform_role: String,
}

/// Esito dell'identificazione della richiesta.
#[derive(Debug, Clone)]
pub enum Identity {
    Anonymous,
    /// Credenziali presenti ma non valide.
    Invalid,
    User(Actor),
}

#[derive(Debug, Clone)]
pub struct BlogRef {
    pub id: Uuid,
    pub slug: String,
    pub owner_id: Uuid,
    pub visibility: String,
    pub is_suspended: bool,
    pub is_paused: bool,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct Access {
    pub actor: Option<Actor>,
    pub blog: Option<BlogRef>,
}

/// Identifica chi chiama.
///
/// TEMPORANEO (Fase 1): con `NOCT_DEV_AUTH=1` accetta `Authorization: Bearer
/// dev:<uuid-utente>`. JWT, sessioni e API token arrivano con il crate auth;
/// questa funzione è l'unico punto da sostituire.
pub async fn authenticate(st: &AppState, headers: &HeaderMap) -> Result<Identity, ApiError> {
    let Some(value) = headers.get(axum::http::header::AUTHORIZATION) else {
        return Ok(Identity::Anonymous);
    };
    let Some(token) = value.to_str().ok().and_then(|v| v.strip_prefix("Bearer ")) else {
        return Ok(Identity::Invalid);
    };
    if !st.config.dev_auth {
        return Ok(Identity::Invalid);
    }
    let Some(id) = token.strip_prefix("dev:").and_then(|s| Uuid::parse_str(s).ok()) else {
        return Ok(Identity::Invalid);
    };
    let row = sqlx::query("SELECT platform_role::text AS role FROM users WHERE id = $1 AND is_active")
        .bind(id)
        .fetch_optional(&st.db)
        .await?;
    Ok(match row {
        Some(r) => Identity::User(Actor { user_id: id, platform_role: r.get("role") }),
        None => Identity::Invalid,
    })
}

fn client_ip(headers: &HeaderMap) -> String {
    // dietro un reverse proxy fidato (Traefik) il primo elemento è il client reale
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| headers.get("x-real-ip").and_then(|v| v.to_str().ok()).map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Pipeline comune a ogni rotta: identità → rate limit → policy di piattaforma
/// → blog nello scope (404) → visibilità/scrittura.
pub async fn enforce(
    st: &AppState,
    headers: &HeaderMap,
    route: &RouteSpec,
    blog_slug: Option<&str>,
) -> Result<Access, ApiError> {
    let identity = authenticate(st, headers).await?;
    let actor = match &identity {
        Identity::User(a) => Some(a.clone()),
        _ => None,
    };

    if let Some(rate) = &route.rate {
        let who = match (rate.key, &actor) {
            (RateKey::User, Some(a)) => format!("user:{}", a.user_id),
            _ => format!("ip:{}", client_ip(headers)),
        };
        let key = format!("{}:{}:{who}", route.module, route.id);
        st.limiter.check(&key, rate.max, Duration::from_secs(rate.window_secs)).map_err(ApiError::too_many_requests)?;
    }

    let needs_login = !matches!(route.policy, Policy::Public);
    if needs_login && actor.is_none() {
        return Err(ApiError::unauthorized("Not authenticated"));
    }
    if let (Policy::PlatformRole(roles), Some(a)) = (&route.policy, &actor)
        && !roles.contains(&a.platform_role.as_str())
    {
        return Err(ApiError::forbidden("Permessi insufficienti."));
    }

    let mut blog = None;
    if let Some(slug) = blog_slug {
        let b = load_blog(st, slug).await?.ok_or_else(|| ApiError::not_found("Blog non trovato."))?;
        match route.policy {
            Policy::BlogWrite => {
                let user = actor.as_ref().map(|a| a.user_id);
                if !can_write_blog(st, user, &b).await? {
                    return Err(ApiError::forbidden(
                        "Serve essere proprietario del blog o avere ruolo autore/co-autore.",
                    ));
                }
            }
            _ => {
                if !can_view_blog(st, actor.as_ref().map(|a| a.user_id), &b).await? {
                    // un blog non visibile non rivela di esistere
                    return Err(ApiError::not_found("Blog non trovato."));
                }
            }
        }
        blog = Some(b);
    }
    Ok(Access { actor, blog })
}

async fn load_blog(st: &AppState, slug: &str) -> Result<Option<BlogRef>, ApiError> {
    let row = sqlx::query(
        "SELECT id, slug, owner_id, visibility::text AS visibility, is_suspended, is_paused, deleted_at
         FROM blogs WHERE slug = $1",
    )
    .bind(slug)
    .fetch_optional(&st.db)
    .await?;
    Ok(row.map(|r| BlogRef {
        id: r.get("id"),
        slug: r.get("slug"),
        owner_id: r.get("owner_id"),
        visibility: r.get("visibility"),
        is_suspended: r.get("is_suspended"),
        is_paused: r.get("is_paused"),
        deleted_at: r.get("deleted_at"),
    }))
}

async fn membership_role(st: &AppState, user: Uuid, blog: Uuid) -> Result<Option<String>, ApiError> {
    Ok(sqlx::query_scalar("SELECT role::text FROM blog_memberships WHERE user_id = $1 AND blog_id = $2")
        .bind(user)
        .bind(blog)
        .fetch_optional(&st.db)
        .await?)
}

/// Chi può scrivere: non sospeso/cancellato; il proprietario sempre; un blog
/// privato è un diario (solo proprietario); altrimenti ruolo autore/co-autore.
pub async fn can_write_blog(st: &AppState, user: Option<Uuid>, blog: &BlogRef) -> Result<bool, ApiError> {
    let Some(user) = user else { return Ok(false) };
    if blog.is_suspended || blog.deleted_at.is_some() {
        return Ok(false);
    }
    if blog.owner_id == user {
        return Ok(true);
    }
    if blog.visibility == "private" {
        return Ok(false);
    }
    Ok(matches!(membership_role(st, user, blog.id).await?.as_deref(), Some("autore" | "co_autore")))
}

/// Chi può vedere: sospeso → nessuno; in cancellazione → solo il proprietario;
/// in pausa → proprietario e membri; poi per visibilità public/members/private.
pub async fn can_view_blog(st: &AppState, user: Option<Uuid>, blog: &BlogRef) -> Result<bool, ApiError> {
    if blog.is_suspended {
        return Ok(false);
    }
    if blog.deleted_at.is_some() {
        return Ok(user == Some(blog.owner_id));
    }
    if blog.is_paused {
        let Some(user) = user else { return Ok(false) };
        return Ok(blog.owner_id == user || membership_role(st, user, blog.id).await?.is_some());
    }
    if blog.visibility == "public" {
        return Ok(true);
    }
    let Some(user) = user else { return Ok(false) };
    if blog.visibility == "members" {
        return Ok(true);
    }
    Ok(blog.owner_id == user || membership_role(st, user, blog.id).await?.is_some())
}
