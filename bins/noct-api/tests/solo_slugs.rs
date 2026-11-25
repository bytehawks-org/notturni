//! Regole degli slug dei blog in modalità solo: solo formato e collisione con le rotte reali
//! del frontend. La modalità platform resta invariata.

mod common;

use axum::http::StatusCode;
use common::*;
use noct_blogs::rules::SOLO_ROUTE_COLLISIONS;
use serde_json::json;
use std::path::Path;

fn new_blog(slug: &str) -> serde_json::Value {
    json!({"slug": slug, "title": format!("Blog {slug}")})
}

#[tokio::test]
async fn solo_accepts_short_and_platform_reserved_slugs_that_are_not_routes() {
    let env = setup(&[], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let anna = env.user("anna", "utente").await;
    for slug in ["ab", "www", "mail", "blog"] {
        let (s, b) = env.post("/blogs", Some(anna), new_blog(slug)).await;
        assert_eq!(s, StatusCode::CREATED, "{slug}: {b}");
        assert_eq!(b["slug"], slug);
    }
    env.teardown().await;
}

#[tokio::test]
async fn solo_rejects_frontend_route_collisions_with_400() {
    let env = setup(&[], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let anna = env.user("anna", "utente").await;
    for slug in ["post", "admin", "login", "u", "p", "api"] {
        let (s, b) = env.post("/blogs", Some(anna), new_blog(slug)).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{slug}: {b}");
        assert!(detail(&b).contains(&format!("'{slug}' collides with a site page.")), "{slug}: {b}");
    }
    env.teardown().await;
}

#[tokio::test]
async fn solo_still_checks_the_format() {
    let env = setup(&[], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let anna = env.user("anna", "utente").await;
    for slug in ["Diario", "-diario", "dia_rio"] {
        let (s, b) = env.post("/blogs", Some(anna), new_blog(slug)).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{slug}: {b}");
        assert!(detail(&b).contains("minuscole"), "{slug}: {b}");
    }
    env.teardown().await;
}

#[tokio::test]
async fn platform_keeps_its_slug_rules() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    // "www" (3 caratteri) cade prima sul limite di lunghezza
    let (s, b) = env.post("/blogs", Some(anna), new_blog("www")).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert!(detail(&b).contains("almeno 4"), "{b}");
    let (s, b) = env.post("/blogs", Some(anna), new_blog("admin")).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert!(detail(&b).contains("riservato"), "{b}");
    let (s, b) = env.post("/blogs", Some(anna), new_blog("abc")).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert!(detail(&b).contains("almeno 4"), "{b}");
    env.teardown().await;
}

/// Ogni cartella di primo livello di `frontend/src/app` che corrisponde a una rotta statica
/// deve comparire in `SOLO_ROUTE_COLLISIONS`, così l'elenco non si scosta dal frontend.
#[test]
fn route_collisions_cover_every_static_frontend_route() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../frontend/src/app");
    let entries = std::fs::read_dir(&app).unwrap_or_else(|e| panic!("{}: {e}", app.display()));
    let mut missing = Vec::new();
    for entry in entries {
        let entry = entry.expect("voce di directory");
        if !entry.file_type().expect("tipo file").is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        // rotte dinamiche/di gruppo e interne di Next.js: non sono nomi di spazio fissi
        // i nomi con il punto (feed.xml, sitemap.xml...) non passano il formato dello slug
        if name.starts_with(['[', '(', '_']) || name.contains('.') {
            continue;
        }
        if !SOLO_ROUTE_COLLISIONS.contains(&name.as_str()) {
            missing.push(name);
        }
    }
    assert!(missing.is_empty(), "rotte del frontend non presenti in SOLO_ROUTE_COLLISIONS: {missing:?}");
}
