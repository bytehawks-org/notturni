//! Library functions behind `noct-ctl user create` and `noct-ctl user reset-password`
//! (SMTP-less recovery): sessions revoked, audit without password, failure cases.

mod common;

use axum::http::StatusCode;
use common::*;
use noct_audit::AuditActor;
use noct_auth::accounts::{NewUser, create_user, reset_password};
use noct_core::RequestMeta;
use serde_json::{Value, json};

const OLD: &str = "vecchia-password-1";
const NEW: &str = "nuova-password-lunga-2";

async fn login(env: &Env, email: &str, password: &str) -> StatusCode {
    env.post("/auth/login", None, json!({"email": email, "password": password})).await.0
}

async fn make(env: &Env, name: &str) -> uuid::Uuid {
    create_user(
        &env.pool,
        NewUser { username: name.into(), email: format!("{name}@example.com"), password: OLD.into() },
        AuditActor::System,
        &RequestMeta::default(),
        "noct-ctl",
    )
    .await
    .unwrap()
    .id
}

#[tokio::test]
async fn reset_changes_the_password_revokes_sessions_and_audits_without_the_password() {
    let env = setup(&[], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let anna = make(&env, "anna").await;
    // an existing session and an access token issued before the reset
    let old_access = noct_core::security::jwt::encode(
        env.state.config.jwt_secret.as_bytes(),
        "access",
        anna,
        chrono::Utc::now().timestamp() - 60,
        900,
    );
    sqlx::query(
        "INSERT INTO user_sessions (user_id, refresh_token_hash, expires_at) VALUES ($1, 'h', now() + interval '1 day')",
    )
    .bind(anna)
    .execute(&env.pool)
    .await
    .unwrap();
    assert_eq!(login(&env, "anna@example.com", OLD).await, StatusCode::OK);

    reset_password(&env.pool, "ANNA@example.com", NEW.into(), AuditActor::System, &RequestMeta::default(), "noct-ctl")
        .await
        .unwrap();

    assert_eq!(login(&env, "anna@example.com", OLD).await, StatusCode::UNAUTHORIZED);
    assert_eq!(login(&env, "anna@example.com", NEW).await, StatusCode::OK);
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM user_sessions WHERE refresh_token_hash = 'h'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(sessions, 0, "old refresh token is gone");
    let req = axum::http::Request::builder()
        .uri("/api/v1/auth/me")
        .header("authorization", format!("Bearer {old_access}"))
        .body(axum::body::Body::empty())
        .unwrap();
    let (s, _, _) = env.send(req).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "access token issued before the reset is invalid");

    let rows: Vec<(String, Value)> =
        sqlx::query_as("SELECT action, payload FROM audit_log WHERE action = 'admin.user_password_reset'")
            .fetch_all(&env.pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), 1);
    let dump = format!("{rows:?}");
    assert!(!dump.contains(OLD) && !dump.contains(NEW) && !dump.contains("argon2"), "{dump}");
    env.teardown().await;
}

#[tokio::test]
async fn reset_failure_cases_do_not_change_anything() {
    let env = setup(&[], &[]).await;
    let _ = make(&env, "anna").await;
    let meta = RequestMeta::default();
    let run = |email: &'static str, pw: &'static str| {
        reset_password(&env.pool, email, pw.into(), AuditActor::System, &meta, "noct-ctl")
    };

    // unknown email: generic 404
    assert_eq!(run("nessuno@example.com", NEW).await.unwrap_err().status, StatusCode::NOT_FOUND);
    // weak password
    assert_eq!(run("anna@example.com", "corta").await.unwrap_err().status, StatusCode::BAD_REQUEST);
    // SSO-only account (no local password)
    sqlx::query("INSERT INTO users (username, email) VALUES ('sso', 'sso@example.com')")
        .execute(&env.pool)
        .await
        .unwrap();
    let e = run("sso@example.com", NEW).await.unwrap_err();
    assert_eq!(e.status, StatusCode::BAD_REQUEST);
    assert!(e.detail.contains("SSO"), "{}", e.detail);
    // inactive account: same generic error as unknown
    sqlx::query("UPDATE users SET is_active = false WHERE username = 'anna'").execute(&env.pool).await.unwrap();
    assert_eq!(run("anna@example.com", NEW).await.unwrap_err().status, StatusCode::NOT_FOUND);

    assert_eq!(login(&env, "anna@example.com", OLD).await, StatusCode::UNAUTHORIZED, "inactive: login refused");
    let audited: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'admin.user_password_reset'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(audited, 0);
    env.teardown().await;
}

#[tokio::test]
async fn create_user_library_function_validates_and_rejects_duplicates() {
    let env = setup(&[], &[]).await;
    let meta = RequestMeta::default();
    let id = make(&env, "anna").await;
    let role: String = sqlx::query_scalar("SELECT platform_role::text FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(role, "utente");
    assert_eq!(login(&env, "anna@example.com", OLD).await, StatusCode::OK);
    let dup = NewUser { username: "anna".into(), email: "altra@example.com".into(), password: OLD.into() };
    assert_eq!(
        create_user(&env.pool, dup, AuditActor::System, &meta, "noct-ctl").await.unwrap_err().status,
        StatusCode::CONFLICT
    );
    let weak = NewUser { username: "bea".into(), email: "bea@example.com".into(), password: "corta".into() };
    assert_eq!(
        create_user(&env.pool, weak, AuditActor::System, &meta, "noct-ctl").await.unwrap_err().status,
        StatusCode::BAD_REQUEST
    );
    env.teardown().await;
}
