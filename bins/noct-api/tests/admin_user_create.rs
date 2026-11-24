//! `POST /admin/users`: the Super Admin creates collaborator accounts without registration or email.
//! Solo mode keeps `register` closed; the created account can log in and accept a blog invitation.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

const PASSWORD: &str = "una-password-lunga-1";

fn new_user(name: &str) -> Value {
    json!({"username": name, "email": format!("{name}@example.com"), "password": PASSWORD})
}

#[tokio::test]
async fn super_admin_creates_a_user_in_solo_mode_who_can_log_in_and_is_audited_without_password() {
    let env = setup(&[], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let root = env.user("root", "super_admin").await;

    let (s, created) = env.post("/admin/users", Some(root), new_user("carla")).await;
    assert_eq!(s, StatusCode::CREATED, "{created}");
    assert_eq!(created["platform_role"], "utente");
    assert_eq!(created["is_active"], true);
    assert!(created["generated_password"].is_null(), "password chosen by the admin is never echoed");
    assert!(created.get("hashed_password").is_none() && created.get("password").is_none());

    let (s, session) = env.post("/auth/login", None, json!({"email": "carla@example.com", "password": PASSWORD})).await;
    assert_eq!(s, StatusCode::OK, "{session}");
    let (s, me) = env.get("/auth/me", Some(created["id"].as_str().unwrap().parse().unwrap())).await;
    assert_eq!((s, me["platform_role"].as_str()), (StatusCode::OK, Some("utente")));

    let rows: Vec<(String, Value, Option<String>)> =
        sqlx::query_as("SELECT action, payload, actor_label FROM audit_log WHERE action = 'admin.user_created'")
            .fetch_all(&env.pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), 1);
    let dump = format!("{:?}", rows[0]);
    assert!(!dump.contains(PASSWORD) && !dump.contains("argon2"), "{dump}");
    assert!(rows[0].2.as_deref().unwrap().contains("root"), "actor is the admin: {dump}");
    env.teardown().await;
}

#[tokio::test]
async fn omitted_password_is_generated_and_shown_once() {
    let env = setup(&[], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let root = env.user("root", "super_admin").await;
    let (s, created) =
        env.post("/admin/users", Some(root), json!({"username": "dario", "email": "dario@example.com"})).await;
    assert_eq!(s, StatusCode::CREATED, "{created}");
    let generated = created["generated_password"].as_str().expect("generated password").to_string();
    let (s, _) = env.post("/auth/login", None, json!({"email": "dario@example.com", "password": generated})).await;
    assert_eq!(s, StatusCode::OK);
    assert!(noct_security::validate::validate_password_policy(&generated).is_ok() && generated.len() >= 20);
    // a second read never returns it
    let (_, listed) = env.get("/admin/users", Some(root)).await;
    assert!(!listed.to_string().contains(&generated), "{listed}");
    let leaked: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE payload::text LIKE '%' || $1 || '%'")
        .bind(&generated)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(leaked, 0);
    env.teardown().await;
}

#[tokio::test]
async fn only_the_super_admin_can_create_and_input_is_validated() {
    let env = setup(&[], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let root = env.user("root", "super_admin").await;
    let admin = env.user("capo", "amministratore").await;
    let anna = env.user("anna", "utente").await;

    assert_eq!(env.post("/admin/users", None, new_user("x1")).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.post("/admin/users", Some(anna), new_user("x1")).await.0, StatusCode::FORBIDDEN);
    assert_eq!(env.post("/admin/users", Some(admin), new_user("x1")).await.0, StatusCode::FORBIDDEN);

    assert_eq!(env.post("/admin/users", Some(root), new_user("bruno")).await.0, StatusCode::CREATED);
    // duplicate username, duplicate email (case-insensitive)
    assert_eq!(env.post("/admin/users", Some(root), new_user("bruno")).await.0, StatusCode::CONFLICT);
    let dup_email = json!({"username": "altro", "email": "BRUNO@example.com", "password": PASSWORD});
    assert_eq!(env.post("/admin/users", Some(root), dup_email).await.0, StatusCode::CONFLICT);
    // weak password, bad username, bad email
    let weak = json!({"username": "debole", "email": "debole@example.com", "password": "corta"});
    assert_eq!(env.post("/admin/users", Some(root), weak).await.0, StatusCode::BAD_REQUEST);
    let bad_name = json!({"username": "a b", "email": "ab@example.com", "password": PASSWORD});
    assert_eq!(env.post("/admin/users", Some(root), bad_name).await.0, StatusCode::BAD_REQUEST);
    let bad_mail = json!({"username": "malmail", "email": "nope", "password": PASSWORD});
    assert_eq!(env.post("/admin/users", Some(root), bad_mail).await.0, StatusCode::BAD_REQUEST);

    let created: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'admin.user_created'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(created, 1, "only the successful call is audited");
    env.teardown().await;
}

#[tokio::test]
async fn register_stays_closed_in_solo_and_the_created_user_can_accept_an_invitation() {
    let env = setup(&[], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let (s, first) = env.post("/auth/register", None, new_user("anna")).await;
    assert_eq!((s, first["platform_role"].as_str()), (StatusCode::CREATED, Some("super_admin")));
    let anna: uuid::Uuid = first["id"].as_str().unwrap().parse().unwrap();
    env.blog("diario", anna, "public").await;

    // regression: registration is refused, before and after creating accounts
    assert_eq!(env.post("/auth/register", None, new_user("carla")).await.0, StatusCode::CONFLICT);
    let (s, created) = env.post("/admin/users", Some(anna), new_user("carla")).await;
    assert_eq!(s, StatusCode::CREATED, "{created}");
    let carla: uuid::Uuid = created["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(env.post("/auth/register", None, new_user("dario")).await.0, StatusCode::CONFLICT);

    let (s, inv) =
        env.post("/blogs/diario/invitations", Some(anna), json!({"username": "carla", "role": "mediatore"})).await;
    assert_eq!(s, StatusCode::CREATED, "{inv}");
    let (_, received) = env.get("/blogs/received-invitations", Some(carla)).await;
    assert_eq!(received.as_array().unwrap().len(), 1);
    let id = inv["id"].as_str().unwrap();
    let (s, acc) = env.post(&format!("/blogs/received-invitations/{id}/accept"), Some(carla), json!({})).await;
    assert_eq!((s, acc["status"].as_str()), (StatusCode::OK, Some("accepted")), "{acc}");
    let role: String = sqlx::query_scalar("SELECT role::text FROM blog_memberships WHERE user_id = $1")
        .bind(carla)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(role, "mediatore");
    env.teardown().await;
}

#[tokio::test]
async fn sensitive_responses_are_not_cacheable() {
    let env = setup(&[], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let root = env.user("root", "super_admin").await;
    let (s, _, headers) = env
        .call(
            axum::http::Method::POST,
            "/admin/users",
            Some(root),
            Some(json!({"username": "erica", "email": "erica@example.com"})),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(headers.get("cache-control").and_then(|v| v.to_str().ok()), Some("no-store"));
    env.teardown().await;
}
