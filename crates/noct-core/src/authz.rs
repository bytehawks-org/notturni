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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenOwner {
    /// Motore core (machine-to-machine): nessun utente associato.
    Core,
    User(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenPrincipal {
    pub id: Uuid,
    pub owner: TokenOwner,
}

/// Esito dell'identificazione della richiesta.
#[derive(Debug, Clone)]
pub enum Identity {
    Anonymous,
    /// Credenziali presenti ma non valide (scadute, revocate, manomesse...).
    Invalid,
    /// Sessione utente (JWT).
    User(Actor),
    Token(TokenPrincipal),
}

/// Dati della richiesta utili a audit e rate limit.
#[derive(Debug, Clone, Default)]
pub struct RequestMeta {
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

/// Header interno con l'IP del peer TCP: lo imposta il middleware di `noct-api`
/// (sovrascrivendo sempre quello inviato dal client).
pub const PEER_IP_HEADER: &str = "x-noct-peer-ip";

/// IP del client. Con `trusted_proxies = N` si prende l'N-esimo valore da destra di
/// `X-Forwarded-For` (quello scritto dall'ultimo hop fidato: i valori a sinistra li
/// controlla il client e si possono falsificare). Senza abbastanza valori, o con
/// 0 proxy fidati, si usa l'IP del peer. Un valore che non è un IP viene scartato.
pub fn client_ip(headers: &HeaderMap, trusted_proxies: usize) -> Option<String> {
    let from_xff = (trusted_proxies > 0)
        .then(|| {
            let parts: Vec<&str> = headers
                .get("x-forwarded-for")?
                .to_str()
                .ok()?
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .collect();
            parts.len().checked_sub(trusted_proxies).map(|i| parts[i].to_string())
        })
        .flatten();
    from_xff
        .or_else(|| headers.get(PEER_IP_HEADER).and_then(|v| v.to_str().ok()).map(str::to_string))
        .filter(|ip| ip.parse::<std::net::IpAddr>().is_ok())
}

pub fn request_meta(headers: &HeaderMap, trusted_proxies: usize) -> RequestMeta {
    RequestMeta {
        ip: client_ip(headers, trusted_proxies),
        user_agent: headers
            .get(axum::http::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.chars().take(500).collect()),
    }
}

#[derive(Debug, Clone)]
pub struct Access {
    pub actor: Option<Actor>,
    pub token: Option<TokenPrincipal>,
    pub blog: Option<BlogRef>,
    pub meta: RequestMeta,
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

/// Identifica chi chiama: API token (`noct_…`) oppure access token JWT.
pub async fn authenticate(st: &AppState, headers: &HeaderMap) -> Result<Identity, ApiError> {
    let Some(value) = headers.get(axum::http::header::AUTHORIZATION) else {
        return Ok(Identity::Anonymous);
    };
    let Some(token) = value.to_str().ok().and_then(|v| v.strip_prefix("Bearer ")) else {
        return Ok(Identity::Invalid);
    };
    if noct_security::token::is_api_token(token) {
        token_identity(st, token).await
    } else {
        jwt_identity(st, token).await
    }
}

async fn jwt_identity(st: &AppState, token: &str) -> Result<Identity, ApiError> {
    let now = Utc::now().timestamp();
    let Ok(claims) = noct_security::jwt::decode(st.config.jwt_secret.as_bytes(), token, "access", now) else {
        return Ok(Identity::Invalid);
    };
    let row =
        sqlx::query("SELECT platform_role::text AS role, is_active, credentials_changed_at FROM users WHERE id = $1")
            .bind(claims.sub)
            .fetch_optional(&st.db)
            .await?;
    let Some(row) = row else { return Ok(Identity::Invalid) };
    if !row.get::<bool, _>("is_active") {
        return Ok(Identity::Invalid);
    }
    // un cambio di password invalida gli access token emessi prima. `iat` è in secondi
    // interi: si confronta con il cambio troncato al secondo, altrimenti un login subito
    // dopo il cambio (stesso secondo) verrebbe respinto.
    if let Some(changed) = row.get::<Option<DateTime<Utc>>, _>("credentials_changed_at")
        && claims.iat < changed.timestamp()
    {
        return Ok(Identity::Invalid);
    }
    Ok(Identity::User(Actor { user_id: claims.sub, platform_role: row.get("role") }))
}

async fn token_identity(st: &AppState, token: &str) -> Result<Identity, ApiError> {
    let hash = noct_security::token::sha256_hex(token);
    let row = sqlx::query(
        "SELECT t.id, t.owner_type::text AS owner_type, t.user_id,
                (t.expires_at IS NOT NULL AND t.expires_at <= now()) AS expired,
                (t.revoked_at IS NOT NULL) AS revoked,
                COALESCE(u.is_active, true) AS owner_active
         FROM api_tokens t LEFT JOIN users u ON u.id = t.user_id
         WHERE t.token_hash = $1",
    )
    .bind(&hash)
    .fetch_optional(&st.db)
    .await?;
    let Some(row) = row else { return Ok(Identity::Invalid) };
    if row.get::<bool, _>("expired") || row.get::<bool, _>("revoked") || !row.get::<bool, _>("owner_active") {
        return Ok(Identity::Invalid);
    }
    let id: Uuid = row.get("id");
    let owner = match (row.get::<String, _>("owner_type").as_str(), row.get::<Option<Uuid>, _>("user_id")) {
        ("core", None) => TokenOwner::Core,
        ("user", Some(uid)) => TokenOwner::User(uid),
        _ => return Ok(Identity::Invalid), // riga incoerente: meglio rifiutare
    };
    // traccia l'ultimo uso senza una scrittura per ogni richiesta
    sqlx::query("UPDATE api_tokens SET last_used_at = now() WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < now() - interval '1 minute')")
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Identity::Token(TokenPrincipal { id, owner }))
}

/// Pipeline comune a ogni rotta: identità → rate limit → policy → blog nello
/// scope (404) → visibilità/scrittura.
pub async fn enforce(
    st: &AppState,
    headers: &HeaderMap,
    route: &RouteSpec,
    blog_slug: Option<&str>,
) -> Result<Access, ApiError> {
    let meta = request_meta(headers, st.config.trusted_proxies);
    let identity = authenticate(st, headers).await?;
    let (actor, token) = match identity {
        Identity::User(a) => (Some(a), None),
        Identity::Token(t) => (None, Some(t)),
        Identity::Anonymous | Identity::Invalid => (None, None),
    };

    if let Some(rate) = &route.rate {
        let who = match (rate.key, &actor) {
            (RateKey::User, Some(a)) => format!("user:{}", a.user_id),
            _ => format!("ip:{}", meta.ip.as_deref().unwrap_or("unknown")),
        };
        let key = format!("{}:{}:{who}", route.module, route.id);
        st.limiter
            .check(&key, rate.max, Duration::from_secs(rate.window_secs))
            .await
            .map_err(ApiError::too_many_requests)?;
    }

    let authenticated = match route.policy {
        Policy::Public => true,
        Policy::User | Policy::BlogWrite | Policy::PlatformRole(_) => actor.is_some(),
        Policy::Token => token.is_some(),
        Policy::UserOrToken => actor.is_some() || token.is_some(),
    };
    if !authenticated {
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
    Ok(Access { actor, token, blog, meta })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn h(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut m = HeaderMap::new();
        for (k, v) in pairs {
            m.insert(axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(), v.parse().unwrap());
        }
        m
    }

    #[test]
    fn client_ip_trusts_only_the_hop_we_control() {
        // un client malevolo antepone un IP falso; Traefik aggiunge quello reale in coda
        let headers = h(&[("x-forwarded-for", "6.6.6.6, 203.0.113.9")]);
        assert_eq!(client_ip(&headers, 1).as_deref(), Some("203.0.113.9"));
        // due hop fidati (CDN + ingress): il secondo da destra
        let headers = h(&[("x-forwarded-for", "6.6.6.6, 198.51.100.7, 10.0.0.2")]);
        assert_eq!(client_ip(&headers, 2).as_deref(), Some("198.51.100.7"));
    }

    #[test]
    fn client_ip_falls_back_to_the_peer() {
        assert_eq!(client_ip(&h(&[(PEER_IP_HEADER, "192.0.2.1")]), 1).as_deref(), Some("192.0.2.1"));
        // 0 proxy fidati: l'header del client è ignorato del tutto
        let headers = h(&[("x-forwarded-for", "6.6.6.6"), (PEER_IP_HEADER, "192.0.2.1")]);
        assert_eq!(client_ip(&headers, 0).as_deref(), Some("192.0.2.1"));
        // meno valori dei proxy attesi: l'header non è affidabile
        let headers = h(&[("x-forwarded-for", "6.6.6.6"), (PEER_IP_HEADER, "192.0.2.1")]);
        assert_eq!(client_ip(&headers, 2).as_deref(), Some("192.0.2.1"));
        assert_eq!(client_ip(&HeaderMap::new(), 1), None);
    }

    #[test]
    fn client_ip_rejects_garbage() {
        assert_eq!(client_ip(&h(&[("x-forwarded-for", "non-un-ip")]), 1), None);
        assert_eq!(client_ip(&h(&[("x-forwarded-for", "1.2.3.4; DROP TABLE")]), 1), None);
        assert_eq!(client_ip(&h(&[("x-forwarded-for", "2001:db8::1")]), 1).as_deref(), Some("2001:db8::1"));
    }

    #[test]
    fn user_agent_is_truncated() {
        let long = "x".repeat(900);
        let m = request_meta(&h(&[("user-agent", &long)]), 1);
        assert_eq!(m.user_agent.unwrap().len(), 500);
    }
}
