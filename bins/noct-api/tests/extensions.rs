//! Estensioni di blog e post dei moduli: campi letti dal corpo, scritti nella stessa
//! transazione del core, aggiunti alle risposte; assenti se il modulo non è attivo.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

#[tokio::test]
async fn blog_fields_of_an_active_module_are_stored_and_returned() {
    let env = setup(&["sandbox"], &[]).await;
    let anna = env.user("anna", "utente").await;

    let (s, b) =
        env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "D", "sandbox_theme": "scuro"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    assert_eq!(b["sandbox_theme"], "scuro");
    let (s, b2) = env.post("/blogs", Some(anna), json!({"slug": "altro", "title": "A"})).await;
    assert_eq!((s, b2["sandbox_theme"].as_str()), (StatusCode::CREATED, Some("chiaro")), "default del modulo");

    // un valore non valido annulla l'intera richiesta: nessun blog a metà
    let (s, e) =
        env.post("/blogs", Some(anna), json!({"slug": "rotto", "title": "R", "sandbox_theme": "arcobaleno"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
    assert!(detail(&e).contains("chiaro, scuro"));
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM blogs WHERE slug = 'rotto')")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert!(!exists, "rollback del blog");

    // aggiornamento, dettaglio ed elenchi
    let (_, u) = env.patch("/blogs/altro", Some(anna), json!({"sandbox_theme": "scuro", "title": "Nuovo"})).await;
    assert_eq!((u["sandbox_theme"].as_str(), u["title"].as_str()), (Some("scuro"), Some("Nuovo")));
    let (_, u) = env.patch("/blogs/altro", Some(anna), json!({"title": "Ancora"})).await;
    assert_eq!(u["sandbox_theme"], "scuro", "assente: invariato");
    assert_eq!(env.patch("/blogs/altro", Some(anna), json!({"sandbox_theme": 3})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(env.get("/blogs/diario", None).await.1["sandbox_theme"], "scuro");
    let (_, mine) = env.get("/blogs/mine", Some(anna)).await;
    let themes: Vec<_> = mine.as_array().unwrap().iter().map(|b| b["sandbox_theme"].as_str().unwrap()).collect();
    assert_eq!(themes, ["scuro", "scuro"], "una query per elenco, tutti i blog arricchiti");
    env.teardown().await;
}

#[tokio::test]
async fn post_fields_are_validated_atomically_and_inherited_by_translations() {
    let env = setup(&["sandbox"], &[]).await;
    let anna = env.user("anna", "utente").await;
    env.blog("diario", anna, "public").await;

    let (s, p) = env
        .post(
            "/blogs/diario/posts",
            Some(anna),
            json!({"slug": "uno", "title": "1", "content": "c", "sandbox_mood": "calmo"}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{p}");
    assert_eq!(p["sandbox_mood"], "calmo");
    let (_, plain) =
        env.post("/blogs/diario/posts", Some(anna), json!({"slug": "due", "title": "2", "content": "c"})).await;
    assert_eq!(plain["sandbox_mood"], Value::Null);

    let (s, e) = env
        .post(
            "/blogs/diario/posts",
            Some(anna),
            json!({"slug": "tre", "title": "3", "content": "c #tag", "sandbox_mood": "furioso"}),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
    let leftovers: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM posts WHERE slug = 'tre'), (SELECT count(*) FROM tags WHERE name = 'tag'),
                (SELECT count(*) FROM noct_outbox WHERE payload->'data'->>'title' = '3')",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert_eq!(leftovers, (0, 0, 0), "nessun residuo: né post, né tag, né eventi");

    let url = format!("/posts/{}", p["id"].as_str().unwrap());
    let (_, u) = env.patch(&url, Some(anna), json!({"sandbox_mood": "allegro"})).await;
    assert_eq!(u["sandbox_mood"], "allegro");
    let (_, u) = env.patch(&url, Some(anna), json!({"title": "solo titolo"})).await;
    assert_eq!(u["sandbox_mood"], "allegro", "assente: invariato");
    assert_eq!(env.patch(&url, Some(anna), json!({"sandbox_mood": null})).await.0, StatusCode::BAD_REQUEST);

    // la traduzione eredita il valore dell'originale, o ne ha uno proprio
    let (_, en) = env
        .post(
            &format!("{url}/translations"),
            Some(anna),
            json!({"slug": "one", "locale": "en", "title": "1", "content": "c"}),
        )
        .await;
    assert_eq!(en["sandbox_mood"], "allegro");
    let (_, de) = env
        .post(
            &format!("{url}/translations"),
            Some(anna),
            json!({"slug": "eins", "locale": "de", "title": "1", "content": "c", "sandbox_mood": "curioso"}),
        )
        .await;
    assert_eq!(de["sandbox_mood"], "curioso");

    // elenco: tutti arricchiti con una sola query del modulo
    let (_, list) = env.get("/blogs/diario/posts", Some(anna)).await;
    assert_eq!(list.as_array().unwrap().len(), 4);
    assert!(list.as_array().unwrap().iter().all(|p| p.get("sandbox_mood").is_some()));
    env.teardown().await;
}

#[tokio::test]
async fn an_inactive_module_adds_nothing_and_ignores_its_fields() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    let (s, b) =
        env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "D", "sandbox_theme": "scuro"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    assert!(b.get("sandbox_theme").is_none(), "il campo di un modulo spento non esiste");
    let (s, p) = env
        .post(
            "/blogs/diario/posts",
            Some(anna),
            json!({"slug": "uno", "title": "1", "content": "c", "sandbox_mood": "x"}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "campo sconosciuto ignorato, come in v1: {p}");
    assert!(p.get("sandbox_mood").is_none());
    env.teardown().await;
}
