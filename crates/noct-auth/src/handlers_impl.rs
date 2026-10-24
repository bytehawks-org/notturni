use crate::{
    AuthSettings,
    jobs::PURPOSE_PASSWORD_RESET,
    util::{
        MSG_BAD_CODE, MSG_BAD_CREDENTIALS, MSG_INACTIVE, REFRESH_COOKIE, clear_refresh_cookie, clear_session_cookies,
        cookie_value, hash_password, set_session_cookies, verify_csrf, verify_dummy, verify_password,
    },
    *,
};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use noct_audit::{AuditActor, Entry};
use noct_core::{
    ApiError, AppState, RouteCtx, TokenOwner,
    bus::{Kind, outbox},
    map_db_error,
    security::{
        constant_time_eq, jwt,
        otp::derive_otp,
        token::{generate_api_token, generate_refresh_token, sha256_hex},
        validate::{normalize_email, validate_email, validate_password_policy, validate_username},
    },
};
use serde_json::json;
use sqlx::{PgConnection, Row};
use std::time::Duration;
use uuid::Uuid;

const USER_OUT_SQL: &str = "SELECT id, username, email, display_name, mfa_enabled, platform_role::text AS platform_role, ui_locale FROM users WHERE id = $1";

// Protezione brute-force sul login: per IP (bot che provano molte email) e per email
// (tentativi mirati da IP diversi). Applicati prima di verificare la password.
const LOGIN_IP_LIMIT: u32 = 20;
const LOGIN_EMAIL_LIMIT: u32 = 5;
const LOGIN_WINDOW: Duration = Duration::from_secs(300);
const RESET_IP_LIMIT: u32 = 20;
const RESET_EMAIL_LIMIT: u32 = 5;
const RESET_WINDOW: Duration = Duration::from_secs(300);
const RESET_CODE_TTL_MINUTES: i64 = 10;

fn settings(ctx: &RouteCtx) -> Result<AuthSettings, ApiError> {
    AuthSettings::from_state(&ctx.state).map_err(ApiError::internal)
}

fn audit_err(e: noct_audit::AuditError) -> ApiError {
    ApiError::internal(e)
}

async fn rate_limit(st: &AppState, key: String, max: u32, window: Duration) -> Result<(), ApiError> {
    st.limiter.check(&key, max, window).await.map_err(ApiError::too_many_requests)
}

async fn user_out(conn: &mut PgConnection, id: Uuid) -> Result<UserOut, ApiError> {
    sqlx::query_as::<_, UserOut>(USER_OUT_SQL)
        .bind(id)
        .fetch_optional(conn)
        .await?
        .ok_or_else(|| ApiError::unauthorized("Utente non valido."))
}

/// Crea una sessione: riga con l'hash del refresh token + access token JWT.
/// Restituisce `(access_token, refresh_token_in_chiaro)`.
pub async fn issue_session(
    conn: &mut PgConnection,
    st: &AppState,
    s: &AuthSettings,
    user_id: Uuid,
) -> Result<(String, String), ApiError> {
    let (refresh, refresh_hash) = generate_refresh_token();
    sqlx::query("INSERT INTO user_sessions (user_id, refresh_token_hash, expires_at) VALUES ($1, $2, now() + make_interval(secs => $3))")
        .bind(user_id)
        .bind(&refresh_hash)
        .bind(s.refresh_ttl_secs() as f64)
        .execute(conn)
        .await?;
    let access = jwt::encode(
        st.config.jwt_secret.as_bytes(),
        "access",
        user_id,
        Utc::now().timestamp(),
        st.config.jwt_access_ttl_minutes * 60,
    );
    Ok((access, refresh))
}

pub fn session_response(access: String, refresh: &str, s: &AuthSettings) -> Result<Response, ApiError> {
    let mut res = noct_core::json_response(200, &SessionResponse { access_token: access, token_type: "bearer".into() });
    set_session_cookies(&mut res, s, refresh)?;
    Ok(res)
}

// ---- registrazione ----------------------------------------------------------------

pub async fn register(ctx: RouteCtx, body: RegisterRequest) -> Result<UserOut, ApiError> {
    let s = settings(&ctx)?;
    let mode: Option<String> = sqlx::query_scalar("SELECT registration_mode FROM platform_config WHERE id = 1")
        .fetch_optional(&ctx.state.db)
        .await?;
    match mode.as_deref().unwrap_or(&s.registration_mode) {
        "open" => {}
        "closed" => return Err(ApiError::forbidden("Le registrazioni sono chiuse su questa piattaforma.")),
        _ => {
            return Err(ApiError::forbidden(
                "Registrazione solo su invito: chiedi un invito a chi gestisce la piattaforma.",
            ));
        }
    }
    let email = normalize_email(&body.email);
    validate_username(&body.username).map_err(ApiError::bad_request)?;
    validate_email(&email).map_err(ApiError::bad_request)?;
    validate_password_policy(&body.password).map_err(ApiError::bad_request)?;
    let hash = hash_password(body.password.clone()).await?;

    let mut tx = ctx.state.db.begin().await?;
    // serializza le registrazioni: il conteggio per "il primo utente" non deve avere corse
    sqlx::query("SELECT pg_advisory_xact_lock(727274001)").execute(&mut *tx).await?;
    let is_first: bool = sqlx::query_scalar("SELECT NOT EXISTS (SELECT 1 FROM users)").fetch_one(&mut *tx).await?;
    // modalità "solo": una sola persona gestisce l'istanza; il primo utente è Super Admin
    // e la registrazione si chiude, senza interventi manuali sul DB
    if s.solo && !is_first {
        return Err(ApiError::conflict(
            "Registrazione non disponibile: questa istanza è configurata per un singolo proprietario.",
        ));
    }
    let role = if s.solo && is_first { "super_admin" } else { "utente" };
    let taken = "Username o email già in uso.";
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (username, email, hashed_password, platform_role) VALUES ($1, $2, $3, $4::platform_role) RETURNING id",
    )
    .bind(&body.username)
    .bind(&email)
    .bind(&hash)
    .bind(role)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| map_db_error(e, &[("uq_users_username", 409, taken), ("uq_users_email", 409, taken)]))?;
    noct_audit::record(
        &mut tx,
        Entry {
            action: "auth.register",
            actor: AuditActor::user(id, &body.username, &email),
            target: Some(("user", id)),
            blog_id: None,
            meta: &ctx.meta,
            payload: json!({ "platform_role": role }),
        },
    )
    .await
    .map_err(audit_err)?;
    let out = user_out(&mut tx, id).await?;
    tx.commit().await?;
    Ok(out)
}

pub async fn username_available(ctx: RouteCtx) -> Result<UsernameAvailability, ApiError> {
    let username =
        ctx.query.get("username").ok_or_else(|| ApiError::unprocessable("Parametro `username` mancante."))?;
    if validate_username(username).is_err() {
        return Ok(UsernameAvailability { available: false, reason: Some("invalid_format".into()) });
    }
    let taken: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE username = $1)")
        .bind(username)
        .fetch_one(&ctx.state.db)
        .await?;
    Ok(UsernameAvailability { available: !taken, reason: taken.then(|| "taken".to_string()) })
}

// ---- login / sessione ---------------------------------------------------------------

pub async fn login(ctx: RouteCtx, body: LoginRequest) -> Result<Response, ApiError> {
    let s = settings(&ctx)?;
    let email = normalize_email(&body.email);
    if let Some(ip) = &ctx.meta.ip {
        rate_limit(&ctx.state, format!("auth:login:ip:{ip}"), LOGIN_IP_LIMIT, LOGIN_WINDOW).await?;
    }
    rate_limit(&ctx.state, format!("auth:login:email:{email}"), LOGIN_EMAIL_LIMIT, LOGIN_WINDOW).await?;

    let row = sqlx::query("SELECT id, username, hashed_password, is_active FROM users WHERE email = $1")
        .bind(&email)
        .fetch_optional(&ctx.state.db)
        .await?;
    let failure = |reason: &'static str, message: &'static str| {
        let st = ctx.state.clone();
        let meta = ctx.meta.clone();
        let email = email.clone();
        async move {
            // l'evento va scritto comunque: transazione dedicata, non quella (assente) del login
            let mut tx = st.db.begin().await?;
            noct_audit::record(
                &mut tx,
                Entry {
                    action: "auth.login_failed",
                    actor: AuditActor::Anonymous { label: Some(email.clone()) },
                    target: None,
                    blog_id: None,
                    meta: &meta,
                    payload: json!({ "email": email, "reason": reason }),
                },
            )
            .await
            .map_err(audit_err)?;
            tx.commit().await?;
            Err::<Response, ApiError>(ApiError::unauthorized(message))
        }
    };

    let Some(row) = row else {
        // stesso costo di una verifica vera: il tempo non rivela se l'email esiste
        verify_dummy(body.password.clone()).await?;
        return failure("unknown_or_wrong_password", MSG_BAD_CREDENTIALS).await;
    };
    let Some(hash) = row.get::<Option<String>, _>("hashed_password") else {
        verify_dummy(body.password.clone()).await?;
        return failure("unknown_or_wrong_password", MSG_BAD_CREDENTIALS).await;
    };
    if !verify_password(body.password.clone(), hash).await? {
        return failure("unknown_or_wrong_password", MSG_BAD_CREDENTIALS).await;
    }
    if !row.get::<bool, _>("is_active") {
        return failure("inactive", MSG_INACTIVE).await;
    }

    let (id, username): (Uuid, String) = (row.get("id"), row.get("username"));
    // un modulo (es. auth_mfa) può chiedere un secondo passaggio prima di emettere la sessione
    if let Some(challenge) = ctx.state.login_gates.challenge(&ctx.state, id, None).await? {
        return Ok(noct_core::json_response(200, &challenge));
    }
    let mut tx = ctx.state.db.begin().await?;
    noct_audit::record(
        &mut tx,
        Entry {
            action: "auth.login",
            actor: AuditActor::user(id, &username, &email),
            target: None,
            blog_id: None,
            meta: &ctx.meta,
            payload: json!({ "method": "password" }),
        },
    )
    .await
    .map_err(audit_err)?;
    let (access, refresh) = issue_session(&mut tx, &ctx.state, &s, id).await?;
    tx.commit().await?;
    session_response(access, &refresh, &s)
}

pub async fn refresh(ctx: RouteCtx) -> Result<Response, ApiError> {
    let s = settings(&ctx)?;
    verify_csrf(&ctx.headers)?;
    let Some(token) = cookie_value(&ctx.headers, REFRESH_COOKIE) else {
        return Err(ApiError::unauthorized("Sessione assente."));
    };
    let hash = sha256_hex(token);

    let fail = |msg: &str| {
        let mut res = ApiError::unauthorized(msg).into_response();
        let _ = clear_refresh_cookie(&mut res, &s);
        res
    };
    let mut tx = ctx.state.db.begin().await?;
    // "rotation": la riga si revoca con un UPDATE atomico, così due refresh concorrenti con
    // lo stesso token non possono riuscire entrambi
    let claimed = sqlx::query(
        "UPDATE user_sessions SET revoked_at = now(), last_used_at = now()
         WHERE refresh_token_hash = $1 AND revoked_at IS NULL AND expires_at > now() RETURNING user_id",
    )
    .bind(&hash)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(claimed) = claimed else {
        let expired: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM user_sessions WHERE refresh_token_hash = $1 AND revoked_at IS NULL)",
        )
        .bind(&hash)
        .fetch_one(&mut *tx)
        .await?;
        return Ok(fail(if expired { "Refresh token scaduto." } else { "Refresh token non valido o revocato." }));
    };
    let user_id: Uuid = claimed.get("user_id");
    let active: Option<bool> =
        sqlx::query_scalar("SELECT is_active FROM users WHERE id = $1").bind(user_id).fetch_optional(&mut *tx).await?;
    if active != Some(true) {
        tx.commit().await?; // la revoca resta
        return Ok(fail("Utente non valido."));
    }
    let (access, new_refresh) = issue_session(&mut tx, &ctx.state, &s, user_id).await?;
    tx.commit().await?;
    session_response(access, &new_refresh, &s)
}

pub async fn logout(ctx: RouteCtx) -> Result<Response, ApiError> {
    let s = settings(&ctx)?;
    verify_csrf(&ctx.headers)?;
    if let Some(token) = cookie_value(&ctx.headers, REFRESH_COOKIE) {
        sqlx::query("UPDATE user_sessions SET revoked_at = now() WHERE refresh_token_hash = $1 AND revoked_at IS NULL")
            .bind(sha256_hex(token))
            .execute(&ctx.state.db)
            .await?;
    }
    let mut res = StatusCode::NO_CONTENT.into_response();
    clear_session_cookies(&mut res, &s)?;
    Ok(res)
}

pub async fn me(ctx: RouteCtx) -> Result<UserOut, ApiError> {
    let actor = ctx.actor.as_ref().ok_or_else(|| ApiError::unauthorized("Not authenticated"))?;
    let mut conn = ctx.state.db.acquire().await?;
    user_out(&mut conn, actor.user_id).await
}

// ---- password dimenticata / cambio password ---------------------------------------------

/// Emette un codice di reset per `user_id` e accoda l'email, nella transazione `conn`. Vale solo
/// l'ultimo codice richiesto. Condiviso con il reset avviato da un amministratore.
pub async fn enqueue_password_reset(conn: &mut PgConnection, user_id: Uuid) -> Result<(), ApiError> {
    sqlx::query("UPDATE password_reset_codes SET consumed_at = now() WHERE user_id = $1 AND consumed_at IS NULL")
        .bind(user_id)
        .execute(&mut *conn)
        .await?;
    let code_id: Uuid = sqlx::query_scalar("INSERT INTO password_reset_codes (user_id, expires_at) VALUES ($1, now() + make_interval(mins => $2)) RETURNING id")
        .bind(user_id)
        .bind(RESET_CODE_TTL_MINUTES as i32)
        .fetch_one(&mut *conn)
        .await?;
    // l'email parte da un job nella stessa transazione: se il bus è giù il messaggio resta
    // nell'outbox, e un'email esistente non risponde diversamente da una inesistente
    outbox::enqueue(conn, Kind::Job, "auth.password_reset.send", &json!({ "code_id": code_id }))
        .await
        .map_err(ApiError::internal)?;
    Ok(())
}

pub async fn forgot_password(ctx: RouteCtx, body: ForgotPasswordRequest) -> Result<(), ApiError> {
    let email = normalize_email(&body.email);
    if let Some(ip) = &ctx.meta.ip {
        rate_limit(&ctx.state, format!("auth:reset:ip:{ip}"), RESET_IP_LIMIT, RESET_WINDOW).await?;
    }
    rate_limit(&ctx.state, format!("auth:reset:email:{email}"), RESET_EMAIL_LIMIT, RESET_WINDOW).await?;

    // risponde sempre 202: l'esistenza di un account non deve essere enumerabile
    let user: Option<(Uuid, bool)> = sqlx::query_as("SELECT id, is_active FROM users WHERE email = $1")
        .bind(&email)
        .fetch_optional(&ctx.state.db)
        .await?;
    let Some((user_id, true)) = user else { return Ok(()) };

    let mut tx = ctx.state.db.begin().await?;
    enqueue_password_reset(&mut tx, user_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn reset_password(ctx: RouteCtx, body: ResetPasswordRequest) -> Result<(), ApiError> {
    let email = normalize_email(&body.email);
    if let Some(ip) = &ctx.meta.ip {
        rate_limit(&ctx.state, format!("auth:reset:ip:{ip}"), RESET_IP_LIMIT, RESET_WINDOW).await?;
    }
    rate_limit(&ctx.state, format!("auth:reset:email:{email}"), RESET_EMAIL_LIMIT, RESET_WINDOW).await?;
    validate_password_policy(&body.new_password).map_err(ApiError::bad_request)?;

    // stesso messaggio per email sconosciuta, codice sbagliato, scaduto o già usato
    let bad = || ApiError::bad_request(MSG_BAD_CODE);
    let user = sqlx::query("SELECT id, username FROM users WHERE email = $1 AND is_active")
        .bind(&email)
        .fetch_optional(&ctx.state.db)
        .await?;
    let Some(user) = user else {
        verify_dummy(body.new_password.clone()).await?;
        return Err(bad());
    };
    let (user_id, username): (Uuid, String) = (user.get("id"), user.get("username"));
    let new_hash = hash_password(body.new_password.clone()).await?;

    let mut tx = ctx.state.db.begin().await?;
    // FOR UPDATE: due reset concorrenti con lo stesso codice non possono riuscire entrambi
    let pending = sqlx::query(
        "SELECT id, (expires_at <= now()) AS expired FROM password_reset_codes
         WHERE user_id = $1 AND consumed_at IS NULL ORDER BY created_at DESC LIMIT 1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(pending) = pending else { return Err(bad()) };
    let code_id: Uuid = pending.get("id");
    let expected = derive_otp(ctx.state.config.jwt_secret.as_bytes(), PURPOSE_PASSWORD_RESET, code_id);
    if pending.get::<bool, _>("expired") || !constant_time_eq(body.code.trim(), &expected) {
        return Err(bad());
    }
    sqlx::query("UPDATE password_reset_codes SET consumed_at = now() WHERE id = $1")
        .bind(code_id)
        .execute(&mut *tx)
        .await?;
    // una password da reimpostare è una password potenzialmente compromessa: fuori ogni sessione
    // (refresh token) e, con `credentials_changed_at`, anche gli access token già emessi
    sqlx::query(
        "UPDATE users SET hashed_password = $2, credentials_changed_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(user_id)
    .bind(&new_hash)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM user_sessions WHERE user_id = $1").bind(user_id).execute(&mut *tx).await?;
    noct_audit::record(
        &mut tx,
        Entry {
            action: "auth.password_reset",
            actor: AuditActor::user(user_id, &username, &email),
            target: Some(("user", user_id)),
            blog_id: None,
            meta: &ctx.meta,
            payload: json!({}),
        },
    )
    .await
    .map_err(audit_err)?;
    tx.commit().await?;
    Ok(())
}

pub async fn change_password(ctx: RouteCtx, body: PasswordChangeRequest) -> Result<StatusOut, ApiError> {
    let actor = ctx.actor.as_ref().ok_or_else(|| ApiError::unauthorized("Not authenticated"))?;
    let row = sqlx::query("SELECT username, email, hashed_password FROM users WHERE id = $1")
        .bind(actor.user_id)
        .fetch_one(&ctx.state.db)
        .await?;
    let Some(current_hash) = row.get::<Option<String>, _>("hashed_password") else {
        return Err(ApiError::bad_request("Questo account non ha una password impostata (accesso solo tramite SSO)."));
    };
    if !verify_password(body.current_password.clone(), current_hash).await? {
        return Err(ApiError::bad_request("Password attuale non corretta."));
    }
    validate_password_policy(&body.new_password).map_err(ApiError::bad_request)?;
    let new_hash = hash_password(body.new_password.clone()).await?;

    let mut tx = ctx.state.db.begin().await?;
    sqlx::query(
        "UPDATE users SET hashed_password = $2, credentials_changed_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(actor.user_id)
    .bind(&new_hash)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM user_sessions WHERE user_id = $1").bind(actor.user_id).execute(&mut *tx).await?;
    noct_audit::record(
        &mut tx,
        Entry {
            action: "user.password_change",
            actor: AuditActor::user(actor.user_id, &row.get::<String, _>("username"), &row.get::<String, _>("email")),
            target: Some(("user", actor.user_id)),
            blog_id: None,
            meta: &ctx.meta,
            payload: json!({}),
        },
    )
    .await
    .map_err(audit_err)?;
    tx.commit().await?;
    Ok(StatusOut { status: "ok".into() })
}

// ---- API token ----------------------------------------------------------------------------

/// Proprietario dei token che la richiesta può gestire e attore da scrivere nell'audit.
async fn token_owner(ctx: &RouteCtx, conn: &mut PgConnection) -> Result<(TokenOwner, AuditActor), ApiError> {
    if let Some(t) = &ctx.token {
        return Ok((t.owner, AuditActor::from_token(t)));
    }
    let actor = ctx.actor.as_ref().ok_or_else(|| ApiError::unauthorized("Not authenticated"))?;
    let row =
        sqlx::query("SELECT username, email FROM users WHERE id = $1").bind(actor.user_id).fetch_one(conn).await?;
    Ok((
        TokenOwner::User(actor.user_id),
        AuditActor::user(actor.user_id, &row.get::<String, _>("username"), &row.get::<String, _>("email")),
    ))
}

fn owner_columns(owner: TokenOwner) -> (&'static str, Option<Uuid>) {
    match owner {
        TokenOwner::Core => ("core", None),
        TokenOwner::User(id) => ("user", Some(id)),
    }
}

/// Emette un token con lo stesso proprietario di chi lo chiede: un token esistente
/// (core o utente) oppure, per il primo token utente, una sessione.
pub async fn create_token(ctx: RouteCtx, body: TokenCreateRequest) -> Result<TokenCreated, ApiError> {
    let mut tx = ctx.state.db.begin().await?;
    let (owner, actor) = token_owner(&ctx, &mut tx).await?;
    let (owner_type, user_id) = owner_columns(owner);
    let (plaintext, prefix, hash) = generate_api_token();
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO api_tokens (name, owner_type, user_id, token_prefix, token_hash) VALUES ($1, $2::api_token_owner, $3, $4, $5) RETURNING id",
    )
    .bind(&body.name)
    .bind(owner_type)
    .bind(user_id)
    .bind(&prefix)
    .bind(&hash)
    .fetch_one(&mut *tx)
    .await?;
    noct_audit::record(
        &mut tx,
        Entry {
            action: "api_token.created",
            actor,
            target: Some(("api_token", id)),
            blog_id: None,
            meta: &ctx.meta,
            payload: json!({ "name": body.name, "owner_type": owner_type }),
        },
    )
    .await
    .map_err(audit_err)?;
    tx.commit().await?;
    Ok(TokenCreated { id, name: body.name, token: plaintext, token_prefix: prefix })
}

pub async fn list_tokens(ctx: RouteCtx) -> Result<Vec<TokenOut>, ApiError> {
    let mut conn = ctx.state.db.acquire().await?;
    let (owner, _) = token_owner(&ctx, &mut conn).await?;
    let (owner_type, user_id) = owner_columns(owner);
    Ok(sqlx::query_as::<_, TokenOut>(
        "SELECT id, name, token_prefix, owner_type::text AS owner_type, created_at, last_used_at, expires_at, revoked_at
         FROM api_tokens WHERE owner_type = $1::api_token_owner AND user_id IS NOT DISTINCT FROM $2 ORDER BY created_at DESC",
    )
    .bind(owner_type)
    .bind(user_id)
    .fetch_all(&mut *conn)
    .await?)
}

pub async fn revoke_token(ctx: RouteCtx) -> Result<(), ApiError> {
    let token_id = noct_core::parse_uuid(ctx.path.get("token_id").map(String::as_str).unwrap_or_default())?;
    let mut tx = ctx.state.db.begin().await?;
    let (owner, actor) = token_owner(&ctx, &mut tx).await?;
    let target =
        sqlx::query("SELECT name, owner_type::text AS owner_type, user_id FROM api_tokens WHERE id = $1 FOR UPDATE")
            .bind(token_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApiError::not_found("Token non trovato."))?;
    let (owner_type, user_id) = owner_columns(owner);
    let is_own =
        target.get::<String, _>("owner_type") == owner_type && target.get::<Option<Uuid>, _>("user_id") == user_id;
    if !is_own {
        return Err(ApiError::forbidden("Non è possibile revocare questo token."));
    }
    sqlx::query("UPDATE api_tokens SET revoked_at = COALESCE(revoked_at, now()), updated_at = now() WHERE id = $1")
        .bind(token_id)
        .execute(&mut *tx)
        .await?;
    noct_audit::record(
        &mut tx,
        Entry {
            action: "api_token.revoked",
            actor,
            target: Some(("api_token", token_id)),
            blog_id: None,
            meta: &ctx.meta,
            payload: json!({ "name": target.get::<String, _>("name") }),
        },
    )
    .await
    .map_err(audit_err)?;
    tx.commit().await?;
    Ok(())
}
