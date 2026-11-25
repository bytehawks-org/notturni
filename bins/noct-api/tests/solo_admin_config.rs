//! `GET|PATCH /admin/config` in solo mode: the extra reserved blog names are not accepted nor reported.
//! The platform mode keeps the old behaviour (covered by the same test file, second test).

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

const MODULES: &[&str] = &["comments", "reports", "auth_sso", "auth_mfa", "gdpr"];

#[tokio::test]
async fn solo_rejects_reserved_blog_names_and_reports_none() {
    let env = setup(MODULES, &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let root = env.user("root", "super_admin").await;

    // a non-empty list is refused and nothing is written
    let (s, j) = env.patch("/admin/config", Some(root), json!({"reserved_blog_names": ["foo"]})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{j}");
    assert!(detail(&j).contains("solo"), "{j}");

    // an empty list is tolerated (the admin UI may still send the draft)
    let (s, j) = env.patch("/admin/config", Some(root), json!({"reserved_blog_names": []})).await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert_eq!(j["reserved_blog_names"], json!([]));

    // footer fields stay editable in solo
    let (s, j) = env.patch("/admin/config", Some(root), json!({"footer_column1_markdown": "x"})).await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert_eq!(j["footer_column1_markdown"], "x");

    let (s, j) = env.get("/admin/config", Some(root)).await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert_eq!(j["reserved_blog_names"], json!([]));
    env.teardown().await;
}

#[tokio::test]
async fn solo_keeps_stored_reserved_names_untouched_and_hidden() {
    let env = setup(MODULES, &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let root = env.user("root", "super_admin").await;
    sqlx::query("INSERT INTO platform_config (id, reserved_blog_names) VALUES (1, '{riservato}') ON CONFLICT (id) DO UPDATE SET reserved_blog_names = '{riservato}'")
        .execute(&env.pool)
        .await
        .unwrap();

    let (s, j) = env.get("/admin/config", Some(root)).await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert_eq!(j["reserved_blog_names"], json!([]));

    // a PATCH that does not touch the list does not change the stored value
    let (s, _) = env.patch("/admin/config", Some(root), json!({"max_blogs_per_user": 7})).await;
    assert_eq!(s, StatusCode::OK);
    let stored: Vec<String> = sqlx::query_scalar("SELECT reserved_blog_names FROM platform_config WHERE id = 1")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(stored, vec!["riservato".to_string()]);
    env.teardown().await;
}

#[tokio::test]
async fn platform_mode_still_accepts_and_reports_reserved_blog_names() {
    let env = setup(MODULES, &[]).await;
    let root = env.user("root", "super_admin").await;

    let (s, j) = env.patch("/admin/config", Some(root), json!({"reserved_blog_names": ["foo"]})).await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert_eq!(j["reserved_blog_names"], json!(["foo"]));

    let (s, j) = env.get("/admin/config", Some(root)).await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert_eq!(j["reserved_blog_names"], json!(["foo"]));
    env.teardown().await;
}
