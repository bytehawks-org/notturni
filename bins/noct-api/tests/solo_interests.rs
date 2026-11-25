//! Interessi spenti in modalità solo: nessuna lista, nessuna scrittura, nessun filtro, nessun output.
//! La modalità platform resta invariata (vedi `profiles.rs`).

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

async fn world(env: &[(&str, &str)]) -> (Env, uuid::Uuid) {
    let env = setup(&["interests"], env).await;
    sqlx::query("INSERT INTO interests_settings (id, items) VALUES (1, $1) ON CONFLICT (id) DO UPDATE SET items = EXCLUDED.items")
        .bind(json!([{"key": "music", "translations": {"it": "Musica"}}]))
        .execute(&env.pool)
        .await
        .unwrap();
    let anna = env.user("anna", "utente").await;
    (env, anna)
}

#[tokio::test]
async fn solo_hides_stores_filters_and_returns_no_interests() {
    let (env, anna) = world(&[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let (s, b) = env.get("/interests", None).await;
    assert_eq!((s, b), (StatusCode::OK, json!([])));
    let (s, _) = env.patch("/users/me", Some(anna), json!({"interests": ["music"]})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(env.patch("/users/me", Some(anna), json!({"interests": []})).await.0, StatusCode::OK);
    // un valore già nel DB non esce mai e non filtra
    sqlx::query("UPDATE users SET interests = '{music}', directory_listed = true WHERE username = 'anna'")
        .execute(&env.pool)
        .await
        .unwrap();
    let (_, me) = env.get("/users/me", Some(anna)).await;
    assert_eq!(me["interests"], json!([]));
    assert_eq!(env.get("/users/anna", None).await.1["interests"], json!([]));
    let (_, list) = env.get("/users?interest=nope", None).await;
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 1, "{list:?}");
    assert_eq!(list[0]["interests"], json!([]));
    env.teardown().await;
}

#[tokio::test]
async fn platform_keeps_interests() {
    let (env, anna) = world(&[]).await;
    assert_eq!(env.get("/interests", None).await.1.as_array().unwrap().len(), 1);
    let (s, me) = env.patch("/users/me", Some(anna), json!({"interests": ["music"]})).await;
    assert_eq!((s, me["interests"].clone()), (StatusCode::OK, json!(["music"])));
    env.teardown().await;
}
