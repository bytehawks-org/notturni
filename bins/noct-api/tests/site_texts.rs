//! Testi dell'interfaccia modificabili dagli amministratori: lettura pubblica per lingua, scrittura in blocco
//! solo per amministratori, gruppi ammessi, testo semplice, atomicità e audit.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

#[tokio::test]
async fn admins_override_interface_texts_per_language_and_restore_the_defaults() {
    let env = setup(&[], &[]).await;
    let root = env.user("root", "super_admin").await;
    let admin = env.user("ada", "amministratore").await;
    let anna = env.user("anna", "utente").await;
    async fn put(
        env: &Env,
        who: Option<uuid::Uuid>,
        locale: &str,
        texts: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        env.put(&format!("/admin/site-texts/{locale}"), who, json!({ "texts": texts })).await
    }

    // lettura pubblica: vuota finché nessuno modifica; la lingua è obbligatoria
    let (s, b) = env.get("/site-texts?locale=it", None).await;
    assert_eq!((s, b["texts"].clone(), b["locale"].clone()), (StatusCode::OK, json!({}), json!("it")));
    assert_eq!(env.get("/site-texts", None).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(env.get("/site-texts?locale=ita", None).await.0, StatusCode::BAD_REQUEST);

    // scrittura: solo amministratori
    let edit = json!({"Home.eyebrow": "  Software libero · multilingua  ", "NewsletterSignup.title": "Le novità"});
    assert_eq!(put(&env, None, "it", edit.clone()).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(put(&env, Some(anna), "it", edit.clone()).await.0, StatusCode::FORBIDDEN);
    let (s, b) = put(&env, Some(admin), "it", edit).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["texts"]["Home.eyebrow"], "Software libero · multilingua", "spazi tolti");

    // per lingua: l'inglese non cambia, e la lettura pubblica riflette la scrittura
    let (_, it) = env.get("/site-texts?locale=it", None).await;
    assert_eq!(it["texts"]["NewsletterSignup.title"], "Le novità");
    let (_, en) = env.get("/site-texts?locale=en", None).await;
    assert_eq!(en["texts"], json!({}));
    let (s, _) = put(&env, Some(root), "en", json!({"Home.eyebrow": "Open source"})).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        env.get("/site-texts?locale=it", None).await.1["texts"]["Home.eyebrow"],
        "Software libero · multilingua"
    );

    // null o testo vuoto ripristinano il predefinito (la riga sparisce)
    let (s, b) = put(&env, Some(admin), "it", json!({"Home.eyebrow": null, "NewsletterSignup.title": "  "})).await;
    assert_eq!((s, b["texts"].clone()), (StatusCode::OK, json!({})), "{b}");

    // chiavi fuori dai gruppi ammessi, markup, graffe, vuoto, lingua non supportata: nulla viene applicato
    for bad in [
        json!({"Nav.overview": "x"}),
        json!({"Admin.title": "x"}),
        json!({"Home.eyebrow": "<script>x</script>"}),
        json!({"Home.eyebrow": "ciao {name}"}),
        json!({"Home.eyebrow": "a\nb"}),
        json!({"Home.eyebrow": 3}),
        json!({}),
    ] {
        let (s, b) = put(&env, Some(admin), "it", bad.clone()).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad}: {b}");
    }
    let (s, _) = put(&env, Some(admin), "de", json!({"Home.eyebrow": "x"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "lingua non supportata dall'interfaccia");
    let (s, _) = put(&env, Some(admin), "it", json!({"Home.limits": "ok", "Nav.overview": "no"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(env.get("/site-texts?locale=it", None).await.1["texts"], json!({}), "atomico: nemmeno la chiave valida");

    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'site_texts.updated'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(n, 3, "tre scritture riuscite, tutte nel registro");
    env.teardown().await;
}
