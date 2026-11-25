//! Testi dell'interfaccia in modalità solo: la scrittura è bloccata (403) per tutti i ruoli,
//! la lettura pubblica resta attiva. La modalità platform resta invariata.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

const SOLO_MESSAGE: &str = "Site texts are fixed in solo mode.";

#[tokio::test]
async fn solo_rejects_site_text_writes_with_403_even_for_super_admin() {
    let env = setup(&[], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let root = env.user("root", "super_admin").await;

    let (s, b) = env.put("/admin/site-texts/it", Some(root), json!({"texts": {"Home.eyebrow": "Ciao"}})).await;
    assert_eq!((s, detail(&b)), (StatusCode::FORBIDDEN, SOLO_MESSAGE), "{b}");

    // il rifiuto precede la validazione: nulla viene salvato
    let (s, b) = env.get("/site-texts?locale=it", None).await;
    assert_eq!((s, b["texts"].clone()), (StatusCode::OK, json!({})), "{b}");
    env.teardown().await;
}

#[tokio::test]
async fn solo_keeps_public_site_texts_readable() {
    let env = setup(&[], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let (s, b) = env.get("/site-texts?locale=it", None).await;
    assert_eq!((s, b["locale"].clone(), b["texts"].clone()), (StatusCode::OK, json!("it"), json!({})), "{b}");
    env.teardown().await;
}

#[tokio::test]
async fn platform_still_accepts_site_text_writes_from_super_admin() {
    let env = setup(&[], &[]).await;
    let root = env.user("root", "super_admin").await;

    let (s, b) = env.put("/admin/site-texts/it", Some(root), json!({"texts": {"Home.eyebrow": "Ciao"}})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["texts"]["Home.eyebrow"], "Ciao");
    env.teardown().await;
}
