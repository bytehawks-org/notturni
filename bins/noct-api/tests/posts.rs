//! Post: creazione e validazioni, visibilità, flusso di revisione/pubblicazione, modifica,
//! traduzioni, ricerca, bibliografie, nome dell'autore, eventi.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use uuid::Uuid;

struct World {
    env: Env,
    anna: Uuid,
    bruno: Uuid,
    carla: Uuid,
    rita: Uuid,
}

/// Blog "diario" di anna con bruno co-autore, carla mediatrice, rita revisora.
async fn world() -> World {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    let carla = env.user("carla", "utente").await;
    let rita = env.user("rita", "utente").await;
    let blog = env.blog("diario", anna, "public").await;
    env.member(blog, bruno, "co_autore").await;
    env.member(blog, carla, "mediatore").await;
    env.member(blog, rita, "revisore").await;
    World { env, anna, bruno, carla, rita }
}

fn post(slug: &str) -> Value {
    json!({"slug": slug, "title": format!("Titolo {slug}"), "content": "Corpo del post."})
}

async fn created(w: &World, who: Uuid, body: Value) -> Value {
    let (s, b) = w.env.post("/blogs/diario/posts", Some(who), body).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    b
}

fn id(p: &Value) -> String {
    p["id"].as_str().unwrap().to_string()
}

async fn outbox_subjects(env: &Env) -> Vec<String> {
    sqlx::query_scalar("SELECT subject FROM noct_outbox ORDER BY created_at, id").fetch_all(&env.pool).await.unwrap()
}

#[tokio::test]
async fn creating_a_post_checks_permissions_and_rules() {
    let w = world().await;
    let env = &w.env;
    assert_eq!(env.post("/blogs/diario/posts", None, post("a")).await.0, StatusCode::UNAUTHORIZED);
    let stranger = env.user("estraneo", "utente").await;
    assert_eq!(env.post("/blogs/diario/posts", Some(stranger), post("a")).await.0, StatusCode::FORBIDDEN);
    assert_eq!(
        env.post("/blogs/diario/posts", Some(w.carla), post("a")).await.0,
        StatusCode::FORBIDDEN,
        "mediatore non scrive"
    );
    assert_eq!(
        env.post("/blogs/diario/posts", Some(w.rita), post("a")).await.0,
        StatusCode::FORBIDDEN,
        "revisore non scrive"
    );
    assert_eq!(env.post("/blogs/inesistente/posts", Some(w.anna), post("a")).await.0, StatusCode::NOT_FOUND);

    let p = created(&w, w.bruno, json!({
        "slug": "ciao-mondo", "title": "Ciao", "content": "Testo con #Rust e ![gatto](https://x.org/g.png) e [link](https://y.org).",
        "tags": ["Web_Dev"], "cover_image_url": "https://x.org/c.png", "cover_image_categories": ["nudity"], "cover_image_alt_text": "una cover"
    })).await;
    assert_eq!(p["status"], "draft");
    assert_eq!(p["published_at"], Value::Null);
    assert_eq!(p["locale"], "it");
    assert_eq!(p["translation_group_id"], p["id"]);
    assert_eq!(p["permalink"], "/diario/ciao-mondo");
    assert_eq!(p["blog_slug"], "diario");
    assert_eq!(p["author_id"], w.bruno.to_string());
    assert_eq!(p["author_display_name"], "bruno");
    assert_eq!(p["manual_tags"], json!(["web-dev"]));
    assert_eq!(p["tags"], json!(["web-dev", "rust"]));
    assert_eq!(p["cover_image_is_sensitive"], true, "una categoria forza il flag");
    assert_eq!(p["cover_image_categories"], json!(["nudity"]));
    assert_eq!(p["is_hidden"], false);
    assert_eq!(p["mentions_enabled"], true);
    assert_eq!(p["effective_search_indexing_enabled"], true);
    assert_eq!(p["search_indexing_enabled"], Value::Null);

    // cache di media e link, tag materializzati
    let pid = Uuid::parse_str(p["id"].as_str().unwrap()).unwrap();
    let media: Vec<(String, String)> = sqlx::query_as("SELECT url, alt_text FROM post_media WHERE post_id = $1")
        .bind(pid)
        .fetch_all(&env.pool)
        .await
        .unwrap();
    assert_eq!(media, [("https://x.org/g.png".to_string(), "gatto".to_string())]);
    let links: i64 = sqlx::query_scalar("SELECT count(*) FROM post_links WHERE post_id = $1")
        .bind(pid)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(links, 1);
    let tags: Vec<String> = sqlx::query_scalar(
        "SELECT t.name FROM post_tags pt JOIN tags t ON t.id = pt.tag_id WHERE pt.post_id = $1 ORDER BY 1",
    )
    .bind(pid)
    .fetch_all(&env.pool)
    .await
    .unwrap();
    assert_eq!(tags, ["rust", "web-dev"]);

    let (s, b) = env.post("/blogs/diario/posts", Some(w.anna), post("ciao-mondo")).await;
    assert_eq!((s, detail(&b)), (StatusCode::CONFLICT, "Slug già in uso su questo blog per questa lingua."));
    // stesso slug in un'altra lingua: ammesso
    assert_eq!(
        env.post(
            "/blogs/diario/posts",
            Some(w.anna),
            json!({"slug": "ciao-mondo", "title": "Hi", "content": "x", "locale": "en"})
        )
        .await
        .0,
        StatusCode::CREATED
    );

    for (body, needle) in [
        (json!({"slug": "pagina", "title": "t", "content": "c"}), "riservato"),
        (json!({"slug": "feed.xml", "title": "t", "content": "c"}), "riservato"),
        (json!({"slug": "con spazio", "title": "t", "content": "c"}), "slug del post"),
        (json!({"slug": "a/b", "title": "t", "content": "c"}), "slug del post"),
        (json!({"slug": "ok", "title": "t", "content": "c", "locale": "ita"}), "ISO 639-1"),
        (json!({"slug": "ok", "title": "t", "content": "#a #b #c #d #e #f"}), "Massimo 5"),
        (json!({"slug": "ok", "title": "t", "content": "c", "tags": ["non valido!"]}), "Tag non valido"),
        (
            json!({"slug": "ok", "title": "t", "content": "c", "cover_image_categories": ["boh"]}),
            "Categorie non valide",
        ),
        (json!({"slug": "ok", "title": "  ", "content": "c"}), "titolo"),
        (json!({"slug": "ok", "title": "t".repeat(256), "content": "c"}), "titolo"),
    ] {
        let (s, b) = env.post("/blogs/diario/posts", Some(w.anna), body.clone()).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body}: {b}");
        assert!(detail(&b).contains(needle), "{body}: {b}");
    }
    // il proprietario di un blog privato è l'unico che scrive
    let diary = env.blog("privato", w.anna, "private").await;
    env.member(diary, w.bruno, "co_autore").await;
    assert_eq!(env.post("/blogs/privato/posts", Some(w.bruno), post("a")).await.0, StatusCode::FORBIDDEN);
    assert_eq!(env.post("/blogs/privato/posts", Some(w.anna), post("a")).await.0, StatusCode::CREATED);
    w.env.teardown().await;
}

#[tokio::test]
async fn draft_visibility_and_publication_lifecycle() {
    let w = world().await;
    let env = &w.env;
    let draft = created(&w, w.anna, post("bozza")).await;
    let did = id(&draft);

    // la bozza si vede solo a chi può scrivere: per gli altri è 404, non 403
    for who in [None, Some(w.carla), Some(w.rita)] {
        assert_eq!(env.get(&format!("/posts/{did}"), who).await.0, StatusCode::NOT_FOUND, "{who:?}");
        assert_eq!(env.get("/blogs/diario/posts/bozza", who).await.0, StatusCode::NOT_FOUND);
        assert!(env.get("/blogs/diario/posts", who).await.1.as_array().unwrap().is_empty());
    }
    for who in [w.anna, w.bruno] {
        assert_eq!(env.get(&format!("/posts/{did}"), Some(who)).await.0, StatusCode::OK);
        assert_eq!(env.get("/blogs/diario/posts/bozza", Some(who)).await.0, StatusCode::OK);
        assert_eq!(env.get("/blogs/diario/posts", Some(who)).await.1.as_array().unwrap().len(), 1);
    }

    // pubblicazione: solo chi scrive; idempotente; evento per la newsletter
    assert_eq!(env.post(&format!("/posts/{did}/publish"), Some(w.carla), json!({})).await.0, StatusCode::FORBIDDEN);
    assert_eq!(env.post(&format!("/posts/{did}/publish"), None, json!({})).await.0, StatusCode::UNAUTHORIZED);
    let (s, p) = env.post(&format!("/posts/{did}/publish"), Some(w.bruno), json!({})).await;
    assert_eq!(s, StatusCode::OK, "{p}");
    assert_eq!(p["status"], "published");
    let first = p["published_at"].clone();
    assert!(first.is_string());
    let (_, again) = env.post(&format!("/posts/{did}/publish"), Some(w.anna), json!({})).await;
    assert_eq!(again["published_at"], first, "già pubblicato: la data non cambia");
    // il corpo può mancare del tutto (il frontend non lo invia)
    let (s, _, _) = env.call(axum::http::Method::POST, &format!("/posts/{did}/publish"), Some(w.anna), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        outbox_subjects(env).await.iter().filter(|s| *s == "posts.post.published").count(),
        1,
        "una sola transizione"
    );

    for who in [None, Some(w.carla)] {
        assert_eq!(env.get(&format!("/posts/{did}"), who).await.0, StatusCode::OK);
        assert_eq!(env.get("/blogs/diario/posts/bozza", who).await.0, StatusCode::OK);
        assert_eq!(env.get("/blogs/diario/posts", who).await.1.as_array().unwrap().len(), 1);
    }

    // pianificato nel futuro: non ancora pubblico
    let future = (chrono::Utc::now() + chrono::Duration::days(2)).to_rfc3339();
    let sched = created(&w, w.anna, post("domani")).await;
    let (s, p) =
        env.post(&format!("/posts/{}/publish", id(&sched)), Some(w.anna), json!({"published_at": future})).await;
    assert_eq!((s, p["status"].as_str()), (StatusCode::OK, Some("published")));
    assert_eq!(env.get("/blogs/diario/posts/domani", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/diario/posts/domani", Some(w.anna)).await.0, StatusCode::OK);
    assert_eq!(env.get("/blogs/diario/posts", None).await.1.as_array().unwrap().len(), 1);
    // quando l'orario passa diventa pubblico
    sqlx::query("UPDATE posts SET published_at = now() - interval '1 minute' WHERE slug = 'domani'")
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(env.get("/blogs/diario/posts/domani", None).await.0, StatusCode::OK);

    // nascosto da un admin, blog in pausa, blog sospeso
    sqlx::query("UPDATE posts SET is_hidden = true WHERE slug = 'bozza'").execute(&env.pool).await.unwrap();
    assert_eq!(env.get("/blogs/diario/posts/bozza", None).await.0, StatusCode::NOT_FOUND);
    let (_, hidden) = env.get("/blogs/diario/posts/bozza", Some(w.anna)).await;
    assert_eq!(hidden["is_hidden"], true, "l'autore vede perché non è raggiungibile");
    sqlx::query("UPDATE posts SET is_hidden = false").execute(&env.pool).await.unwrap();
    sqlx::query("UPDATE blogs SET is_paused = true").execute(&env.pool).await.unwrap();
    assert_eq!(env.get("/blogs/diario/posts/bozza", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get(&format!("/posts/{did}"), None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/diario/posts", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/diario/posts", Some(w.anna)).await.0, StatusCode::OK);
    sqlx::query("UPDATE blogs SET is_paused = false, is_suspended = true").execute(&env.pool).await.unwrap();
    assert_eq!(env.get("/blogs/diario/posts", Some(w.anna)).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        env.post(&format!("/posts/{did}/publish"), Some(w.anna), json!({})).await.0,
        StatusCode::FORBIDDEN,
        "sospeso: nessuna scrittura"
    );
    w.env.teardown().await;
}

#[tokio::test]
async fn review_workflow_has_distinct_roles() {
    let w = world().await;
    let env = &w.env;
    let p = created(&w, w.bruno, post("revisione")).await;
    let pid = id(&p);

    // il revisore non pubblica una bozza né la rimanda indietro se non è in revisione
    assert_eq!(env.post(&format!("/posts/{pid}/publish"), Some(w.rita), json!({})).await.0, StatusCode::FORBIDDEN);
    let (s, b) = env.post(&format!("/posts/{pid}/return-to-draft"), Some(w.rita), json!({})).await;
    assert_eq!((s, detail(&b)), (StatusCode::BAD_REQUEST, "Il post non è in revisione."));

    assert_eq!(
        env.post(&format!("/posts/{pid}/submit-for-review"), Some(w.rita), json!({})).await.0,
        StatusCode::FORBIDDEN
    );
    let (s, p2) = env.post(&format!("/posts/{pid}/submit-for-review"), Some(w.bruno), json!({})).await;
    assert_eq!((s, p2["status"].as_str()), (StatusCode::OK, Some("pending_review")));
    let (s, b) = env.post(&format!("/posts/{pid}/submit-for-review"), Some(w.bruno), json!({})).await;
    assert_eq!((s, detail(&b)), (StatusCode::BAD_REQUEST, "Solo una bozza può essere mandata in revisione."));

    // l'autore (co-autore) non rimanda in bozza: lo fanno proprietario e revisore
    assert_eq!(
        env.post(&format!("/posts/{pid}/return-to-draft"), Some(w.bruno), json!({})).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        env.post(&format!("/posts/{pid}/return-to-draft"), Some(w.carla), json!({})).await.0,
        StatusCode::FORBIDDEN
    );
    let (s, p3) = env.post(&format!("/posts/{pid}/return-to-draft"), Some(w.rita), json!({})).await;
    assert_eq!((s, p3["status"].as_str()), (StatusCode::OK, Some("draft")));

    env.post(&format!("/posts/{pid}/submit-for-review"), Some(w.bruno), json!({})).await;
    let (s, p4) = env.post(&format!("/posts/{pid}/publish"), Some(w.rita), json!({})).await;
    assert_eq!((s, p4["status"].as_str()), (StatusCode::OK, Some("published")), "il revisore approva");
    let (_, via_owner) = env.post(&format!("/posts/{pid}/return-to-draft"), Some(w.anna), json!({})).await;
    assert_eq!(via_owner["code"], Value::Null);
    w.env.teardown().await;
}

#[tokio::test]
async fn updating_a_post_follows_the_legacy_semantics() {
    let w = world().await;
    let env = &w.env;
    let p = created(&w, w.anna, json!({"slug": "modifica", "title": "T", "content": "Vecchio #uno ![a](https://x.org/a.png)", "tags": ["manuale"]})).await;
    let url = format!("/posts/{}", id(&p));

    assert_eq!(env.patch(&url, None, json!({"title": "x"})).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.patch(&url, Some(w.carla), json!({"title": "x"})).await.0, StatusCode::FORBIDDEN);
    assert_eq!(
        env.patch(&format!("/posts/{}", Uuid::new_v4()), Some(w.anna), json!({"title": "x"})).await.0,
        StatusCode::NOT_FOUND
    );

    // il contenuto ricalcola hashtag, media e link; i tag manuali restano
    let (s, u) = env
        .patch(&url, Some(w.bruno), json!({"content": "Nuovo #due ![b](https://x.org/b.png) [l](https://y.org)"}))
        .await;
    assert_eq!(s, StatusCode::OK, "{u}");
    assert_eq!(u["tags"], json!(["manuale", "due"]));
    assert_eq!(u["manual_tags"], json!(["manuale"]));
    assert_eq!(u["author_id"], w.anna.to_string(), "l'autore resta chi ha creato il post");
    let pid = Uuid::parse_str(&id(&p)).unwrap();
    let urls: Vec<String> = sqlx::query_scalar("SELECT url FROM post_media WHERE post_id = $1")
        .bind(pid)
        .fetch_all(&env.pool)
        .await
        .unwrap();
    assert_eq!(urls, ["https://x.org/b.png"]);
    // solo i tag: il testo non cambia
    let (_, u) = env.patch(&url, Some(w.anna), json!({"tags": []})).await;
    assert_eq!((u["tags"].clone(), u["manual_tags"].clone()), (json!(["due"]), json!([])));
    let (s, b) = env.patch(&url, Some(w.anna), json!({"tags": ["a", "b", "c", "d", "e", "f"]})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");

    // copertina: nuova + flag; poi solo categorie; poi rimozione con ""
    let (_, u) = env
        .patch(&url, Some(w.anna), json!({"cover_image_url": "https://x.org/c.png", "cover_image_is_sensitive": true}))
        .await;
    assert_eq!(
        (u["cover_image_url"].as_str(), u["cover_image_is_sensitive"].as_bool()),
        (Some("https://x.org/c.png"), Some(true))
    );
    let (_, u) = env.patch(&url, Some(w.anna), json!({"cover_image_is_sensitive": false})).await;
    assert_eq!(u["cover_image_is_sensitive"], true, "da solo il flag non ha effetto");
    let (_, u) = env.patch(&url, Some(w.anna), json!({"cover_image_categories": ["explicit"]})).await;
    assert_eq!(
        (u["cover_image_categories"].clone(), u["cover_image_url"].as_str()),
        (json!(["explicit"]), Some("https://x.org/c.png"))
    );
    let (_, u) = env.patch(&url, Some(w.anna), json!({"cover_image_alt_text": "alt"})).await;
    assert_eq!(u["cover_image_alt_text"], "alt");
    let (_, u) = env.patch(&url, Some(w.anna), json!({"cover_image_alt_text": null})).await;
    assert_eq!(u["cover_image_alt_text"], "", "null esplicito azzera");
    let (_, u) = env.patch(&url, Some(w.anna), json!({"title": "Altro"})).await;
    assert_eq!(u["cover_image_url"], "https://x.org/c.png", "assente: invariata");
    let (_, u) = env.patch(&url, Some(w.anna), json!({"cover_image_url": ""})).await;
    assert_eq!(
        (u["cover_image_url"].clone(), u["cover_image_is_sensitive"].clone(), u["cover_image_categories"].clone()),
        (Value::Null, json!(false), json!([]))
    );

    // opzioni per crawler: assente / null / valore; un blog escluso non si riapre
    let (_, u) =
        env.patch(&url, Some(w.anna), json!({"search_indexing_enabled": false, "ai_crawling_enabled": true})).await;
    assert_eq!(
        (u["search_indexing_enabled"].clone(), u["effective_search_indexing_enabled"].clone()),
        (json!(false), json!(false))
    );
    assert_eq!(u["effective_ai_crawling_enabled"], true);
    let (_, u) = env.patch(&url, Some(w.anna), json!({"title": "x"})).await;
    assert_eq!(u["search_indexing_enabled"], false, "assente: invariato");
    let (_, u) = env.patch(&url, Some(w.anna), json!({"search_indexing_enabled": null})).await;
    assert_eq!(
        (u["search_indexing_enabled"].clone(), u["effective_search_indexing_enabled"].clone()),
        (Value::Null, json!(true))
    );
    env.patch("/blogs/diario", Some(w.anna), json!({"ai_crawling_enabled": false})).await;
    let (_, u) = env.patch(&url, Some(w.anna), json!({"ai_crawling_enabled": true})).await;
    assert_eq!(
        (u["ai_crawling_enabled"].clone(), u["effective_ai_crawling_enabled"].clone()),
        (json!(true), json!(false))
    );
    assert_eq!(
        env.patch(&url, Some(w.anna), json!({"ai_crawling_enabled": "sì"})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // ogni salvataggio pubblica l'evento per il backup, con il testo
    let saved: Vec<Value> = sqlx::query_scalar(
        "SELECT payload FROM noct_outbox WHERE subject = 'posts.post.saved' ORDER BY created_at, id DESC",
    )
    .fetch_all(&env.pool)
    .await
    .unwrap();
    assert!(saved.len() >= 8);
    assert_eq!(saved[0]["data"]["post_id"], id(&p));
    assert_eq!(saved[0]["data"]["owner_id"], w.anna.to_string());
    assert!(saved[0]["data"]["content"].is_string());
    w.env.teardown().await;
}

#[tokio::test]
async fn author_name_follows_alias_blog_default_and_profile() {
    let w = world().await;
    let env = &w.env;
    sqlx::query("UPDATE users SET first_name = 'Bruno', last_name = 'Neri', display_name = 'B. Neri', post_author_name_style = 'full_name' WHERE id = $1")
        .bind(w.bruno).execute(&env.pool).await.unwrap();
    let p = created(&w, w.bruno, post("firma")).await;
    assert_eq!(p["author_display_name"], "Bruno Neri", "preferenza di profilo");
    let pid = id(&p);

    // il nome del blog è imposto e si riflette subito sui post già scritti
    env.patch("/blogs/diario", Some(w.anna), json!({"default_author_display_name": "La redazione"})).await;
    assert_eq!(env.get(&format!("/posts/{pid}"), Some(w.anna)).await.1["author_display_name"], "La redazione");
    // l'alias di membership vince sul nome del blog
    env.patch("/blogs/diario/my-membership", Some(w.bruno), json!({"author_display_name": "Bru"})).await;
    assert_eq!(env.get(&format!("/posts/{pid}"), Some(w.anna)).await.1["author_display_name"], "Bru");
    // senza alias né nome del blog torna il profilo
    env.patch("/blogs/diario/my-membership", Some(w.bruno), json!({"author_display_name": ""})).await;
    env.patch("/blogs/diario", Some(w.anna), json!({"default_author_display_name": ""})).await;
    assert_eq!(env.get(&format!("/posts/{pid}"), Some(w.anna)).await.1["author_display_name"], "Bruno Neri");
    sqlx::query("UPDATE users SET post_author_name_style = 'display_name' WHERE id = $1")
        .bind(w.bruno)
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(env.get(&format!("/posts/{pid}"), Some(w.anna)).await.1["author_display_name"], "B. Neri");
    w.env.teardown().await;
}

#[tokio::test]
async fn translations_share_a_group_and_resolve_by_locale() {
    let w = world().await;
    let env = &w.env;
    let it = created(&w, w.anna, post("ciao")).await;
    let base = format!("/posts/{}/translations", id(&it));

    assert_eq!(
        env.post(&base, None, json!({"slug": "hello", "locale": "en", "title": "Hello", "content": "x"})).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        env.post(&base, Some(w.carla), json!({"slug": "hello", "locale": "en", "title": "Hello", "content": "x"}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (s, en) = env
        .post(&base, Some(w.bruno), json!({"slug": "hello", "locale": "en", "title": "Hello", "content": "Hi #greet"}))
        .await;
    assert_eq!(s, StatusCode::CREATED, "{en}");
    assert_eq!(en["translation_group_id"], it["id"]);
    assert_eq!((en["locale"].as_str(), en["status"].as_str()), (Some("en"), Some("draft")));
    assert_eq!(en["tags"], json!(["greet"]));
    assert_eq!(en["author_id"], w.bruno.to_string());

    let (s, b) =
        env.post(&base, Some(w.anna), json!({"slug": "altro", "locale": "en", "title": "x", "content": "x"})).await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    let (s, b) =
        env.post(&base, Some(w.anna), json!({"slug": "ciao", "locale": "de", "title": "x", "content": "x"})).await;
    assert_eq!(s, StatusCode::CREATED, "stesso slug in un'altra lingua: {b}");
    let (s, b) =
        env.post(&base, Some(w.anna), json!({"slug": "ciao", "locale": "fr1", "title": "x", "content": "x"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(
        env.post(
            &format!("/posts/{}/translations", Uuid::new_v4()),
            Some(w.anna),
            json!({"slug": "z", "locale": "es", "title": "x", "content": "x"})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    // elenco: solo le traduzioni pubblicate, anche per anonimi
    assert!(env.get(&base, None).await.1.as_array().unwrap().is_empty());
    env.post(&format!("/posts/{}/publish", id(&it)), Some(w.anna), json!({})).await;
    env.post(&format!("/posts/{}/publish", id(&en)), Some(w.anna), json!({})).await;
    let (s, list) = env.get(&base, None).await;
    assert_eq!(s, StatusCode::OK);
    let locales: Vec<_> = list.as_array().unwrap().iter().map(|t| t["locale"].as_str().unwrap()).collect();
    assert_eq!(locales, ["en", "it"]);
    assert_eq!(env.get(&format!("/posts/{}/translations", Uuid::new_v4()), None).await.0, StatusCode::NOT_FOUND);

    // due traduzioni con lo stesso slug: il permalink preferisce la lingua principale del blog
    sqlx::query("UPDATE posts SET status = 'published', published_at = now() WHERE slug = 'ciao'")
        .execute(&env.pool)
        .await
        .unwrap();
    let (s, p) = env.get("/blogs/diario/posts/ciao", None).await;
    assert_eq!((s, p["locale"].as_str()), (StatusCode::OK, Some("it")));
    // filtro per lingua nell'elenco
    let (_, only_en) = env.get("/blogs/diario/posts?locale=en", None).await;
    assert_eq!(only_en.as_array().unwrap().len(), 1);
    w.env.teardown().await;
}

#[tokio::test]
async fn search_matches_title_and_content_of_published_posts_only() {
    let w = world().await;
    let env = &w.env;
    for (slug, title, content, publish) in [
        ("uno", "Gatti e cani", "Parliamo di animali", true),
        ("due", "Altro", "Il 100% dei gatti ama _il sole_", true),
        ("tre", "Bozza sui gatti", "ancora da pubblicare", false),
        ("quattro", "Cucina", "Ricette", true),
    ] {
        let p = created(&w, w.anna, json!({"slug": slug, "title": title, "content": content})).await;
        if publish {
            env.post(&format!("/posts/{}/publish", id(&p)), Some(w.anna), json!({})).await;
        }
    }
    let found = |v: &Value| -> Vec<String> {
        v.as_array().unwrap().iter().map(|p| p["slug"].as_str().unwrap().to_string()).collect()
    };
    let (s, r) = env.get("/blogs/diario/search?q=gatti", None).await;
    assert_eq!(s, StatusCode::OK);
    let mut got = found(&r);
    got.sort();
    assert_eq!(got, ["due", "uno"], "la bozza non compare nemmeno per chi può scrivere");
    assert_eq!(
        found(&env.get("/blogs/diario/search?q=GATTI", Some(w.anna)).await.1).len(),
        2,
        "senza distinzione di maiuscole"
    );
    assert_eq!(found(&env.get("/blogs/diario/search?q=%25", None).await.1), ["due"], "% è letterale");
    assert!(found(&env.get("/blogs/diario/search?q=_", None).await.1).len() == 1, "_ è letterale");
    assert!(env.get("/blogs/diario/search?q=%20%20", None).await.1.as_array().unwrap().is_empty());
    assert!(env.get("/blogs/diario/search", None).await.1.as_array().unwrap().is_empty());
    assert_eq!(found(&env.get("/blogs/diario/search?q=gatti&limit=1", None).await.1).len(), 1);
    assert_eq!(found(&env.get("/blogs/diario/search?q=gatti&limit=1&offset=1", None).await.1).len(), 1);
    assert_eq!(env.get("/blogs/diario/search?q=a&limit=abc", None).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    env.blog("riservato", w.anna, "private").await;
    assert_eq!(env.get("/blogs/riservato/search?q=a", None).await.0, StatusCode::NOT_FOUND);
    w.env.teardown().await;
}

#[tokio::test]
async fn bibliographies_group_published_media_and_links() {
    let w = world().await;
    let env = &w.env;
    let a = created(&w, w.anna, json!({"slug": "a", "title": "A", "content":
        "![x](https://i.org/1.png) ![y](https://i.org/2.png \"sensitive:nudity\") [sito](https://s.org) [nota](#nota-1)"})).await;
    let b = created(
        &w,
        w.anna,
        json!({"slug": "b", "title": "B", "content":
        "![x2](https://i.org/1.png \"sensitive\") [sito bis](https://s.org)"}),
    )
    .await;
    created(&w, w.anna, json!({"slug": "bozza", "title": "Bozza", "content": "![z](https://i.org/segreta.png)"})).await;
    for p in [&a, &b] {
        env.post(&format!("/posts/{}/publish", id(p)), Some(w.anna), json!({})).await;
    }
    sqlx::query("UPDATE posts SET published_at = now() - interval '1 day' WHERE slug = 'a'")
        .execute(&env.pool)
        .await
        .unwrap();

    let (s, media) = env.get("/blogs/diario/media-bibliography", None).await;
    assert_eq!(s, StatusCode::OK);
    let media = media.as_array().unwrap();
    assert_eq!(media.len(), 2, "la bozza non è citata");
    let first = media.iter().find(|m| m["url"] == "https://i.org/1.png").unwrap();
    assert_eq!(first["citations"].as_array().unwrap().len(), 2);
    assert_eq!(first["is_sensitive"], true, "una citazione sensibile rende sensibile la voce");
    assert_eq!(first["citations"][0]["post_slug"], "b", "dal più recente");
    assert_eq!(first["citations"][0]["permalink"], "/diario/b");
    assert!(first["citations"][0]["used_at"].is_string());
    let second = media.iter().find(|m| m["url"] == "https://i.org/2.png").unwrap();
    assert_eq!(second["categories"], json!(["nudity"]));

    let (_, links) = env.get("/blogs/diario/links-bibliography", None).await;
    let links = links.as_array().unwrap();
    assert_eq!(links.len(), 1, "l'ancora della nota non è un link citato");
    assert_eq!(links[0]["url"], "https://s.org");
    assert_eq!(links[0]["citations"].as_array().unwrap().len(), 2);

    // segue la visibilità del blog
    env.blog("riservato", w.anna, "private").await;
    assert_eq!(env.get("/blogs/riservato/media-bibliography", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/riservato/links-bibliography", Some(w.anna)).await.0, StatusCode::OK);
    w.env.teardown().await;
}
