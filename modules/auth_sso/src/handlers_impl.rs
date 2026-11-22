//! Handler `custom` del modulo: avvio e chiusura dell'accesso con un provider esterno.

use crate::providers::{self, ExternalProfile, Provider, percent_encode};
use axum::{
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use noct_audit::{AuditActor, Entry};
use noct_auth::{AuthSettings, session};
use noct_core::{
    ApiError, AppState, RouteCtx,
    security::{
        constant_time_eq,
        cookie::{Cookie, SameSite},
        token::random_secret,
        validate::{RESERVED_USERNAMES, USERNAME_MAX_LENGTH},
    },
};
use serde_json::{Value, json};
use sqlx::Row;
use std::time::Duration;
use uuid::Uuid;

const STATE_COOKIE: &str = "noct_sso_state";
const STATE_PATH: &str = "/api/v1/auth/sso";
const STATE_TTL_SECS: i64 = 600;
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

fn provider_of(ctx: &RouteCtx) -> Option<Provider> {
    ctx.path.get("provider").and_then(|p| Provider::parse(p))
}

fn credentials(ctx: &RouteCtx, p: Provider) -> Option<(String, String)> {
    let id = ctx.config_str(&format!("{}_client_id", p.name())).filter(|s| !s.is_empty())?;
    let secret = ctx.config_str(&format!("{}_client_secret", p.name())).filter(|s| !s.is_empty())?;
    Some((id.to_string(), secret.to_string()))
}

fn frontend(ctx: &RouteCtx) -> String {
    ctx.config_str("frontend_url").unwrap_or("http://localhost:3000").trim_end_matches('/').to_string()
}

fn redirect_uri(ctx: &RouteCtx, p: Provider) -> String {
    let base = ctx.config_str("redirect_base_url").unwrap_or("http://localhost:8000").trim_end_matches('/');
    format!("{base}/api/v1/auth/sso/{}/callback", p.name())
}

fn state_cookie<'a>(s: &'a AuthSettings, value: &'a str, max_age: i64) -> Cookie<'a> {
    // Lax: il ritorno dal provider è una navigazione di primo livello e deve portare il cookie
    Cookie {
        name: STATE_COOKIE,
        value,
        path: STATE_PATH,
        domain: None,
        max_age_secs: max_age,
        http_only: true,
        secure: s.cookie_secure,
        same_site: SameSite::Lax,
    }
}

fn redirect(location: &str, cookies: Vec<String>) -> Result<Response, ApiError> {
    let mut res = Response::new(axum::body::Body::empty());
    *res.status_mut() = StatusCode::SEE_OTHER;
    res.headers_mut().insert(header::LOCATION, HeaderValue::from_str(location).map_err(ApiError::internal)?);
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    for c in cookies {
        res.headers_mut().append(header::SET_COOKIE, HeaderValue::from_str(&c).map_err(ApiError::internal)?);
    }
    Ok(res)
}

/// Un solo esito di errore verso il frontend: i dettagli restano nei log, non nell'URL.
fn failure(ctx: &RouteCtx, s: &AuthSettings, why: &str) -> Result<Response, ApiError> {
    tracing::warn!("accesso SSO non riuscito: {why}");
    let clear = state_cookie(s, "", 0).to_clearing_header().map_err(ApiError::internal)?;
    redirect(&format!("{}/login?sso_error=1", frontend(ctx)), vec![clear])
}

async fn provider_allowed(st: &AppState, p: Provider) -> Result<bool, ApiError> {
    let enabled: Option<Vec<String>> =
        sqlx::query_scalar("SELECT enabled_providers FROM sso_settings WHERE id = 1").fetch_optional(&st.db).await?;
    Ok(match enabled {
        Some(list) if !list.is_empty() => list.iter().any(|n| n == p.name()),
        _ => true,
    })
}

// ---- avvio ----------------------------------------------------------------------------------------------------------

pub async fn sso_login(ctx: RouteCtx) -> Result<Response, ApiError> {
    let s = AuthSettings::from_state(&ctx.state).map_err(ApiError::internal)?;
    let name = ctx.path.get("provider").cloned().unwrap_or_default();
    let Some(p) = provider_of(&ctx) else {
        return Err(ApiError::new(503, format!("Provider '{name}' non configurato.")));
    };
    let Some((client_id, _)) = credentials(&ctx, p) else {
        return Err(ApiError::new(503, format!("Provider '{name}' non configurato.")));
    };
    if !provider_allowed(&ctx.state, p).await? {
        return Err(ApiError::forbidden(format!("Provider '{name}' disattivato dall'amministratore.")));
    }
    let state = random_secret(32);
    let verifier = random_secret(48);
    let ep = providers::endpoints(p, ctx.config_str("endpoint_base").unwrap_or_default());
    let url = providers::authorize_url(
        &ep,
        &client_id,
        &redirect_uri(&ctx, p),
        p.scope(),
        &state,
        &providers::pkce_challenge(&verifier),
    );
    // state e verifier sono legati a questo browser: un callback senza il cookie giusto è respinto
    let cookie =
        state_cookie(&s, &format!("{state}.{verifier}"), STATE_TTL_SECS).to_header().map_err(ApiError::internal)?;
    redirect(&url, vec![cookie])
}

// ---- chiusura -------------------------------------------------------------------------------------------------------

fn http() -> Result<reqwest::Client, ApiError> {
    reqwest::Client::builder().timeout(HTTP_TIMEOUT).user_agent("Notturni-SSO/1.0").build().map_err(ApiError::internal)
}

/// Scambia il codice per un access token direttamente con il provider (TLS, segreto del client).
async fn exchange_code(ctx: &RouteCtx, p: Provider, code: &str, verifier: &str) -> Option<String> {
    let (client_id, secret) = credentials(ctx, p)?;
    let ep = providers::endpoints(p, ctx.config_str("endpoint_base").unwrap_or_default());
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", &redirect_uri(ctx, p)),
        ("client_id", &client_id),
        ("client_secret", &secret),
        ("code_verifier", verifier),
    ];
    let resp = http().ok()?.post(&ep.token).header(header::ACCEPT, "application/json").form(&form).send().await.ok()?;
    let body: Value = resp.error_for_status().ok()?.json().await.ok()?;
    body.get("access_token").and_then(Value::as_str).map(str::to_string)
}

async fn get_json(client: &reqwest::Client, url: &str, token: &str) -> Option<Value> {
    let resp = client.get(url).bearer_auth(token).header(header::ACCEPT, "application/json").send().await.ok()?;
    resp.error_for_status().ok()?.json().await.ok()
}

async fn fetch_profile(ctx: &RouteCtx, p: Provider, token: &str) -> Option<ExternalProfile> {
    let ep = providers::endpoints(p, ctx.config_str("endpoint_base").unwrap_or_default());
    let client = http().ok()?;
    let user = get_json(&client, &ep.userinfo, token).await?;
    match (p, &ep.emails) {
        (Provider::Github, Some(emails_url)) => {
            let emails = get_json(&client, emails_url, token).await?;
            providers::from_github(&user, &emails)
        }
        _ => providers::from_oidc(&user),
    }
}

enum Resolved {
    /// L'utente può entrare.
    User(Uuid),
    /// Serve prima la verifica MFA: URL a cui mandare il browser.
    Challenge(String),
    Denied(&'static str),
}

async fn free_username(st: &AppState, email: &str) -> Result<String, ApiError> {
    let mut base = providers::username_candidate(email);
    if RESERVED_USERNAMES.contains(&base.as_str()) {
        base.push('1');
    }
    let mut candidate = base.clone();
    for suffix in 1..1000 {
        let taken: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE username = $1)")
            .bind(&candidate)
            .fetch_one(&st.db)
            .await?;
        if !taken {
            return Ok(candidate);
        }
        let tail = suffix.to_string();
        let keep = USERNAME_MAX_LENGTH.saturating_sub(tail.len());
        candidate = format!("{}{tail}", base.chars().take(keep).collect::<String>());
    }
    Err(ApiError::internal("nessuno username libero"))
}

/// Applica l'account linking per email: identità già nota → il suo utente; email di un utente
/// esistente → collegamento (solo se l'email è garantita dal provider, e dopo la verifica MFA
/// se attiva); altrimenti nuovo utente, se la piattaforma accetta registrazioni aperte.
async fn resolve(
    ctx: &RouteCtx,
    s: &AuthSettings,
    p: Provider,
    profile: &ExternalProfile,
) -> Result<Resolved, ApiError> {
    let st = &ctx.state;
    let known: Option<(Uuid, bool)> = sqlx::query_as(
        "SELECT u.id, u.is_active FROM sso_identities i JOIN users u ON u.id = i.user_id
         WHERE i.provider = $1::sso_provider AND i.provider_user_id = $2",
    )
    .bind(p.name())
    .bind(&profile.provider_user_id)
    .fetch_optional(&st.db)
    .await?;
    if let Some((id, active)) = known {
        return Ok(if active { Resolved::User(id) } else { Resolved::Denied("utente disattivato") });
    }

    let existing = sqlx::query("SELECT id, is_active FROM users WHERE email = $1")
        .bind(&profile.email)
        .fetch_optional(&st.db)
        .await?;
    if let Some(row) = existing {
        let (id, active): (Uuid, bool) = (row.get("id"), row.get("is_active"));
        if !active {
            return Ok(Resolved::Denied("utente disattivato"));
        }
        // chiunque può dichiarare l'email di un altro presso un provider che non la verifica
        if !profile.email_verified {
            return Ok(Resolved::Denied("email non verificata dal provider"));
        }
        let link = json!({"provider": p.name(), "provider_user_id": profile.provider_user_id, "email": profile.email});
        if let Some(challenge) = st.login_gates.challenge(st, id, Some(link.clone())).await? {
            let token = challenge.get("challenge").and_then(Value::as_str).unwrap_or_default();
            let method = challenge.get("method").and_then(Value::as_str).unwrap_or_default();
            return Ok(Resolved::Challenge(format!(
                "{}/login?mfa_challenge={}&mfa_method={}",
                frontend(ctx),
                percent_encode(token),
                percent_encode(method)
            )));
        }
        link_identity(st, id, p, profile).await?;
        return Ok(Resolved::User(id));
    }

    if !profile.email_verified {
        return Ok(Resolved::Denied("email non verificata dal provider"));
    }
    if s.registration_mode != "open" || s.solo {
        return Ok(Resolved::Denied("registrazioni non aperte"));
    }
    let username = free_username(st, &profile.email).await?;
    let mut tx = st.db.begin().await?;
    let id: Uuid = sqlx::query_scalar("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
        .bind(&username)
        .bind(&profile.email)
        .fetch_one(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO sso_identities (user_id, provider, provider_user_id, email) VALUES ($1, $2::sso_provider, $3, $4)",
    )
    .bind(id)
    .bind(p.name())
    .bind(&profile.provider_user_id)
    .bind(&profile.email)
    .execute(&mut *tx)
    .await?;
    noct_audit::record(
        &mut tx,
        Entry {
            action: "auth.register",
            actor: AuditActor::user(id, &username, &profile.email),
            target: None,
            blog_id: None,
            meta: &ctx.meta,
            payload: json!({"method": format!("sso_{}", p.name())}),
        },
    )
    .await
    .map_err(ApiError::internal)?;
    tx.commit().await?;
    Ok(Resolved::User(id))
}

async fn link_identity(st: &AppState, user: Uuid, p: Provider, profile: &ExternalProfile) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO sso_identities (user_id, provider, provider_user_id, email) VALUES ($1, $2::sso_provider, $3, $4)
         ON CONFLICT (provider, provider_user_id) DO NOTHING",
    )
    .bind(user)
    .bind(p.name())
    .bind(&profile.provider_user_id)
    .bind(&profile.email)
    .execute(&st.db)
    .await?;
    Ok(())
}

pub async fn sso_callback(ctx: RouteCtx) -> Result<Response, ApiError> {
    let st = &ctx.state;
    let s = AuthSettings::from_state(st).map_err(ApiError::internal)?;
    let Some(p) = provider_of(&ctx) else { return failure(&ctx, &s, "provider sconosciuto") };
    if credentials(&ctx, p).is_none() || !provider_allowed(st, p).await? {
        return failure(&ctx, &s, "provider non disponibile");
    }
    // il cookie dice che questo browser ha avviato il flusso: senza, è un callback forgiato
    let cookie = noct_auth::session::cookie_value(&ctx.headers, STATE_COOKIE).unwrap_or_default();
    let (state, verifier) = cookie.split_once('.').unwrap_or_default();
    let query_state = ctx.query.get("state").map(String::as_str).unwrap_or_default();
    if state.is_empty() || !constant_time_eq(state, query_state) {
        return failure(&ctx, &s, "state non valido");
    }
    let Some(code) = ctx.query.get("code").filter(|c| !c.is_empty()) else {
        return failure(&ctx, &s, "codice assente");
    };
    let Some(token) = exchange_code(&ctx, p, code, verifier).await else {
        return failure(&ctx, &s, "scambio del codice fallito");
    };
    let Some(profile) = fetch_profile(&ctx, p, &token).await else { return failure(&ctx, &s, "profilo non leggibile") };

    let user = match resolve(&ctx, &s, p, &profile).await? {
        Resolved::User(id) => id,
        Resolved::Challenge(url) => {
            let clear = state_cookie(&s, "", 0).to_clearing_header().map_err(ApiError::internal)?;
            return redirect(&url, vec![clear]);
        }
        Resolved::Denied(why) => return failure(&ctx, &s, why),
    };

    let (username, email): (String, String) =
        sqlx::query_as("SELECT username, email FROM users WHERE id = $1").bind(user).fetch_one(&st.db).await?;
    let mut tx = st.db.begin().await?;
    noct_audit::record(
        &mut tx,
        Entry {
            action: "auth.login",
            actor: AuditActor::user(user, &username, &email),
            target: None,
            blog_id: None,
            meta: &ctx.meta,
            payload: json!({"method": format!("sso_{}", p.name())}),
        },
    )
    .await
    .map_err(ApiError::internal)?;
    let (_access, refresh) = session::issue_session(&mut tx, st, &s, user).await?;
    tx.commit().await?;

    let mut res = redirect(
        &format!("{}/dashboard", frontend(&ctx)),
        vec![state_cookie(&s, "", 0).to_clearing_header().map_err(ApiError::internal)?],
    )?;
    session::set_session_cookies(&mut res, &s, &refresh)?;
    Ok(res)
}
