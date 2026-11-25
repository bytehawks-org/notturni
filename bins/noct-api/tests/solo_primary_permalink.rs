//! Permalink dei post in modalità solo: il post del blog primario usa `/post/<slug>`, quello
//! degli altri spazi resta `/<blog-slug>/<post-slug>`. La modalità platform resta invariata.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

const SOLO: &[(&str, &str)] = &[("NOCT_DEPLOYMENT_MODE", "solo")];

#[tokio::test]
async fn solo_post_of_the_primary_blog_has_post_permalink() {
    let env = setup(&[], SOLO).await;
    let (s, u) = env
        .post(
            "/auth/register",
            None,
            json!({"username": "anna", "email": "anna@example.com", "password": "una-password-lunga-1"}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{u}");
    let anna: uuid::Uuid = u["id"].as_str().unwrap().parse().unwrap();
    let (s, p) = env
        .post(
            "/blogs/anna/posts",
            Some(anna),
            json!({"slug": "ciao-mondo", "title": "Ciao", "content": "Corpo del post."}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{p}");
    assert_eq!(p["blog_slug"], "anna");
    assert_eq!(p["permalink"], "/post/ciao-mondo");
}

#[tokio::test]
async fn solo_post_of_an_extra_space_keeps_blog_slug_permalink() {
    let env = setup(&[], SOLO).await;
    let anna = env.user("anna", "utente").await;
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "Diario"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    let (s, p) = env
        .post("/blogs/diario/posts", Some(anna), json!({"slug": "ciao", "title": "Ciao", "content": "Corpo."}))
        .await;
    assert_eq!(s, StatusCode::CREATED, "{p}");
    assert_eq!(p["permalink"], "/diario/ciao");
}

#[tokio::test]
async fn solo_sitemaps_use_root_and_post_permalinks_for_the_primary_blog() {
    let env = setup(&["branding"], SOLO).await;
    let (s, u) = env
        .post(
            "/auth/register",
            None,
            json!({"username": "anna", "email": "anna@example.com", "password": "una-password-lunga-1"}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{u}");
    let anna: uuid::Uuid = u["id"].as_str().unwrap().parse().unwrap();
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "Diario"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    for (blog, slug) in [("anna", "ciao"), ("diario", "altro")] {
        let (s, p) = env
            .post(&format!("/blogs/{blog}/posts"), Some(anna), json!({"slug": slug, "title": "T", "content": "Corpo."}))
            .await;
        assert_eq!(s, StatusCode::CREATED, "{p}");
        let (s, p2) = env.post(&format!("/posts/{}/publish", p["id"].as_str().unwrap()), Some(anna), json!({})).await;
        assert_eq!(s, StatusCode::OK, "{p2}");
    }
    let (s, g) = env.get("/seo/sitemap", None).await;
    assert_eq!(s, StatusCode::OK, "{g}");
    let paths: Vec<&str> = g["entries"].as_array().unwrap().iter().map(|e| e["path"].as_str().unwrap()).collect();
    assert_eq!(paths.iter().filter(|p| **p == "/").count(), 1, "{paths:?}");
    assert!(paths.contains(&"/post/ciao"), "{paths:?}");
    assert!(paths.contains(&"/diario") && paths.contains(&"/diario/altro"), "{paths:?}");
    assert!(!paths.contains(&"/anna") && !paths.contains(&"/anna/ciao"), "{paths:?}");

    let (s, sp) = env.get("/blogs/anna/sitemap", None).await;
    assert_eq!(s, StatusCode::OK, "{sp}");
    let paths: Vec<&str> = sp["entries"].as_array().unwrap().iter().map(|e| e["path"].as_str().unwrap()).collect();
    assert_eq!(paths, vec!["/", "/post/ciao"]);
    let (_, sp) = env.get("/blogs/diario/sitemap", None).await;
    let paths: Vec<&str> = sp["entries"].as_array().unwrap().iter().map(|e| e["path"].as_str().unwrap()).collect();
    assert_eq!(paths, vec!["/diario", "/diario/altro"]);
}
