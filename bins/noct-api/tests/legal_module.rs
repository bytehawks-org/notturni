//! Informativa privacy e cookie policy: modelli per lingua, dati del titolare, adattamento alle funzioni attive,
//! pubblicazione come pagine del sito, lingue aggiunte e protezioni (dati mancanti, pagina preesistente).

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

fn doc<'a>(overview: &'a Value, doc: &str, locale: &str) -> &'a Value {
    overview["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["doc"] == doc && d["locale"] == locale)
        .expect("documento")
}

fn complete() -> Value {
    json!({
        "controller_name": "Associazione Esempio APS",
        "controller_address": "Via Roma 1, 00100 Roma (RM), Italia",
        "controller_email": "privacy@example.com",
        "hosting_description": "Hetzner Online GmbH (Germania, UE) per server e storage",
        // l'archivio dell'audit è attivo di default: il testo deve dire per quanto si conserva
        "audit_archive_retention": "24 mesi",
    })
}

#[tokio::test]
async fn templates_adapt_to_active_features_and_publish_as_site_pages() {
    let env = setup(&["legal", "pages", "newsletter", "comments"], &[]).await;
    let root = env.user("root", "super_admin").await;
    let anna = env.user("anna", "utente").await;

    // solo gli amministratori
    assert_eq!(env.get("/admin/legal", None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.get("/admin/legal", Some(anna)).await.0, StatusCode::FORBIDDEN);

    // i quattro modelli ci sono subito, non pubblicati e incompleti; le funzioni attive sono rilevate
    let (s, ov) = env.get("/admin/legal", Some(root)).await;
    assert_eq!(s, StatusCode::OK, "{ov}");
    assert_eq!(ov["documents"].as_array().unwrap().len(), 4);
    assert_eq!(ov["template_locales"], json!(["it", "en"]));
    assert_eq!(
        (ov["flags"]["newsletter"].clone(), ov["flags"]["sso"].clone(), ov["flags"]["backup"].clone()),
        (json!(true), json!(false), json!(false))
    );
    let privacy_it = doc(&ov, "privacy", "it");
    assert_eq!((privacy_it["published"].clone(), privacy_it["customized"].clone()), (json!(false), json!(false)));
    for name in
        ["controller_name", "controller_address", "controller_email", "hosting_description", "audit_archive_retention"]
    {
        assert!(privacy_it["missing"].as_array().unwrap().contains(&json!(name)), "{name} manca");
    }
    assert_eq!(privacy_it["issues"], json!([]), "i modelli forniti non hanno errori");
    assert_eq!(ov["values"]["audit_days"], "105");

    // senza i dati del titolare non si pubblica
    let (s, b) = env.post("/admin/legal/privacy/it/publish", Some(root), json!({})).await;
    assert_eq!((s, detail(&b).contains("dati mancanti")), (StatusCode::BAD_REQUEST, true), "{b}");
    assert_eq!(env.get("/pages/privacy?locale=it", None).await.0, StatusCode::NOT_FOUND);

    // dati non validi
    for bad in [
        json!({"controller_email": "non-una-email"}),
        json!({"controller_name": "a{{b}}c"}),
        json!({"controller_name": "a\nb"}),
        json!({"minimum_age": 5}),
    ] {
        assert_eq!(env.put("/admin/legal/settings", Some(root), bad.clone()).await.0, StatusCode::BAD_REQUEST, "{bad}");
    }
    let (s, ov) = env.put("/admin/legal/settings", Some(root), complete()).await;
    assert_eq!(s, StatusCode::OK, "{ov}");
    assert_eq!(ov["settings"]["controller_name"], "Associazione Esempio APS");
    assert_eq!(doc(&ov, "privacy", "it")["missing"], json!([]));

    // pubblicazione: il testo risolto diventa una pagina pubblica, adattata ai moduli attivi
    let (s, d) = env.post("/admin/legal/privacy/it/publish", Some(root), json!({})).await;
    assert_eq!(s, StatusCode::OK, "{d}");
    assert_eq!((d["published"].clone(), d["stale"].clone()), (json!(true), json!(false)));
    let (s, page) = env.get("/pages/privacy?locale=it", None).await;
    assert_eq!(s, StatusCode::OK, "{page}");
    let text = page["content"].as_str().unwrap();
    assert_eq!(page["title"], "Informativa sulla privacy");
    assert!(
        text.contains("Associazione Esempio APS") && text.contains("privacy@example.com") && text.contains("Hetzner")
    );
    assert!(text.contains("**Newsletter.**") && text.contains("**Commenti.**"), "funzioni attive descritte");
    assert!(
        !text.contains("Accesso con un account esterno") && !text.contains("Analisi automatica delle immagini"),
        "funzioni spente assenti"
    );
    assert!(
        text.contains("non ha designato un Responsabile") && text.contains("105 giorni"),
        "valori dedotti e DPO assente"
    );
    assert!(!text.contains("{{") && !text.contains("}}"), "nessun segnaposto residuo");
    assert!(!text.starts_with("# "), "il titolo lo mostra la pagina: il testo non lo ripete");

    // la cookie policy elenca i cookie realmente usati e non quelli di funzioni spente
    env.post("/admin/legal/cookie/it/publish", Some(root), json!({})).await;
    let cookie = env.get("/pages/cookie-policy?locale=it", None).await.1["content"].as_str().unwrap().to_string();
    for name in ["noct_refresh_token", "noct_csrf_token", "notturni_ui_locale", "notturni_theme_mode"] {
        assert!(cookie.contains(name), "{name}");
    }
    assert!(!cookie.contains("noct_sso_state") && !cookie.contains("Turnstile"), "SSO e Turnstile non sono attivi");

    // elenco pubblico per i link del footer; lingua inesistente → la migliore disponibile
    let (s, links) = env.get("/legal/public?locale=it", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        links.as_array().unwrap().iter().map(|l| l["slug"].as_str().unwrap()).collect::<Vec<_>>(),
        ["privacy", "cookie-policy"]
    );
    assert_eq!(env.get("/legal/public?locale=fr", None).await.1[0]["locale"], "it");

    // cambiare i dati riscrive le pagine pubblicate; svuotare un dato che usano è rifiutato
    let mut renamed = complete();
    renamed["controller_name"] = json!("Nuova Associazione");
    assert_eq!(env.put("/admin/legal/settings", Some(root), renamed).await.0, StatusCode::OK);
    assert!(
        env.get("/pages/privacy?locale=it", None).await.1["content"].as_str().unwrap().contains("Nuova Associazione")
    );
    let (s, b) = env.put("/admin/legal/settings", Some(root), json!({"controller_name": "X"})).await;
    assert_eq!((s, detail(&b).contains("pubblicato")), (StatusCode::BAD_REQUEST, true), "{b}");

    let audit: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action LIKE 'legal.%'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert!(audit >= 4, "{audit}");
    env.teardown().await;

    // le funzioni opzionali compaiono solo se attive: stessa installazione senza newsletter né commenti
    let env = setup(&["legal", "pages"], &[]).await;
    let root = env.user("root", "super_admin").await;
    env.put("/admin/legal/settings", Some(root), complete()).await;
    env.post("/admin/legal/privacy/it/publish", Some(root), json!({})).await;
    let text = env.get("/pages/privacy?locale=it", None).await.1["content"].as_str().unwrap().to_string();
    assert!(
        !text.contains("**Newsletter.**") && !text.contains("**Commenti.**"),
        "senza i moduli il testo non li cita"
    );
    env.teardown().await;
}

#[tokio::test]
async fn editing_translations_stale_detection_and_page_conflicts() {
    let env = setup(&["legal", "pages"], &[]).await;
    let root = env.user("root", "super_admin").await;
    env.put("/admin/legal/settings", Some(root), complete()).await;
    let url = "/admin/legal/privacy";

    // una modifica rende il documento «personalizzato»; il ripristino lo riporta al modello
    let (_, d) = env.get(&format!("{url}/it"), Some(root)).await;
    let original = d["content"].as_str().unwrap().to_string();
    let (s, d) = env
        .put(
            &format!("{url}/it"),
            Some(root),
            json!({"title": "Privacy", "content": format!("{original}\nUna riga in più.\n")}),
        )
        .await;
    assert_eq!((s, d["customized"].clone()), (StatusCode::OK, json!(true)), "{d}");
    let (_, d) = env.post(&format!("{url}/it/reset"), Some(root), json!({})).await;
    assert_eq!((d["customized"].clone(), d["content"].as_str().unwrap()), (json!(false), original.as_str()));

    // un refuso nel modello (variabile inesistente) blocca la pubblicazione
    let (_, d) = env
        .put(
            &format!("{url}/it"),
            Some(root),
            json!({"title": "Privacy", "content": "Ciao {{non_esiste}} e {{#if boh}}x{{/if}}"}),
        )
        .await;
    assert_eq!(d["issues"].as_array().unwrap().len(), 2, "{d}");
    let (s, b) = env.post(&format!("{url}/it/publish"), Some(root), json!({})).await;
    assert_eq!((s, detail(&b).contains("errori nel testo")), (StatusCode::BAD_REQUEST, true), "{b}");
    env.post(&format!("{url}/it/reset"), Some(root), json!({})).await;

    // pubblicato → modificato = da ripubblicare
    assert_eq!(env.post(&format!("{url}/it/publish"), Some(root), json!({})).await.0, StatusCode::OK);
    let (_, d) = env
        .put(
            &format!("{url}/it"),
            Some(root),
            json!({"title": "Informativa", "content": format!("{original}\nIntegrazione.\n")}),
        )
        .await;
    assert_eq!((d["published"].clone(), d["stale"].clone()), (json!(true), json!(true)));
    let (_, d) = env.post(&format!("{url}/it/publish"), Some(root), json!({})).await;
    assert_eq!(d["stale"], false);
    assert_eq!(env.get("/pages/privacy?locale=it", None).await.1["title"], "Informativa");

    // nuova lingua: parte dalla copia di un'altra, si pubblica come traduzione della stessa pagina
    let (s, b) = env.post(&format!("{url}/de/reset"), Some(root), json!({})).await;
    assert_eq!((s, detail(&b).contains("from")), (StatusCode::BAD_REQUEST, true), "{b}");
    let (s, d) = env.post(&format!("{url}/de/reset"), Some(root), json!({"from": "en"})).await;
    assert_eq!(
        (s, d["has_template"].clone(), d["customized"].clone()),
        (StatusCode::OK, json!(false), json!(true)),
        "{d}"
    );
    assert_eq!(env.post(&format!("{url}/de/publish"), Some(root), json!({})).await.0, StatusCode::OK);
    assert_eq!(env.get("/pages/privacy?locale=de", None).await.0, StatusCode::OK);
    let page_id = env.get("/pages/privacy?locale=it", None).await.1["id"].as_str().unwrap().to_string();
    let (_, tr) = env.get(&format!("/pages/{page_id}/translations"), None).await;
    let mut langs: Vec<_> = tr.as_array().unwrap().iter().map(|p| p["locale"].as_str().unwrap().to_string()).collect();
    langs.sort();
    assert_eq!(langs, ["de", "it"]);
    assert_eq!(env.get("/legal/public?locale=de", None).await.1[0]["locale"], "de");

    // le lingue del modello non si eliminano; quella aggiunta sì, con la sua pagina
    assert_eq!(env.delete(&format!("{url}/it"), Some(root)).await, StatusCode::BAD_REQUEST);
    assert_eq!(env.delete(&format!("{url}/de"), Some(root)).await, StatusCode::NO_CONTENT);
    assert_eq!(env.get("/pages/privacy?locale=de", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get(&format!("{url}/de"), Some(root)).await.0, StatusCode::NOT_FOUND);

    // ritirare la pagina la toglie dal sito ma tiene il testo
    let (_, d) = env.post(&format!("{url}/it/unpublish"), Some(root), json!({})).await;
    assert_eq!(d["published"], false);
    assert_eq!(env.get("/pages/privacy?locale=it", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/legal/public?locale=it", None).await.1.as_array().unwrap().len(), 0);

    // una pagina con lo stesso indirizzo creata a mano non si sovrascrive senza conferma
    let (s, _) = env
        .post("/pages", Some(root), json!({"slug": "cookie-policy", "locale": "it", "title": "Vecchia", "content": "testo manuale", "is_published": true}))
        .await;
    assert_eq!(s, StatusCode::CREATED);
    let (s, b) = env.post("/admin/legal/cookie/it/publish", Some(root), json!({})).await;
    assert_eq!((s, detail(&b).contains("conferma")), (StatusCode::CONFLICT, true), "{b}");
    assert_eq!(env.get("/pages/cookie-policy?locale=it", None).await.1["content"], "testo manuale");
    assert_eq!(
        env.post("/admin/legal/cookie/it/publish", Some(root), json!({"replace_existing": true})).await.0,
        StatusCode::OK
    );
    assert!(
        env.get("/pages/cookie-policy?locale=it", None).await.1["content"]
            .as_str()
            .unwrap()
            .contains("noct_refresh_token")
    );

    // documento o lingua sconosciuti
    assert_eq!(env.get("/admin/legal/termini/it", Some(root)).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/admin/legal/privacy/ita", Some(root)).await.0, StatusCode::BAD_REQUEST);
    env.teardown().await;
}
