//! Moduli di contenuto agganciati a blog e post: categorie (`taxonomy`) e pubblicazioni
//! (`publications`).

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use uuid::Uuid;

async fn world(modules: &[&str]) -> (Env, Uuid) {
    let env = setup(modules, &[]).await;
    let anna = env.user("anna", "utente").await;
    env.blog("diario", anna, "public").await;
    (env, anna)
}

async fn new_post(env: &Env, who: Uuid, slug: &str, extra: Value) -> Value {
    let mut body = json!({"slug": slug, "title": format!("T {slug}"), "content": "uno due tre"});
    body.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    let (s, b) = env.post("/blogs/diario/posts", Some(who), body).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    b
}

fn id(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

async fn publish(env: &Env, who: Uuid, post: &Value) {
    let (s, b) = env.post(&format!("/posts/{}/publish", id(post)), Some(who), json!({})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

// ---- taxonomy ----------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn post_category_is_tristate_validated_and_inherited() {
    let (env, anna) = world(&["taxonomy", "publications", "pages"]).await;
    let (_, cat) = env.post("/blogs/diario/categories", Some(anna), json!({"name": "Viaggi", "slug": "viaggi"})).await;
    let cat_id = id(&cat);

    let p = new_post(&env, anna, "uno", json!({"category_id": cat_id})).await;
    assert_eq!(p["category_id"], cat_id.as_str());
    assert_eq!(p["category"], json!({"id": cat_id, "name": "Viaggi", "slug": "viaggi"}));
    assert_eq!(new_post(&env, anna, "senza", json!({})).await["category"], Value::Null);

    // categoria di un altro blog, o non UUID: 400 e nessun post creato
    let bruno = env.user("bruno", "utente").await;
    env.blog("altro", bruno, "public").await;
    let (_, other) = env.post("/blogs/altro/categories", Some(bruno), json!({"name": "X", "slug": "x"})).await;
    let body = json!({"slug": "due", "title": "2", "content": "c", "category_id": id(&other)});
    let (s, e) = env.post("/blogs/diario/posts", Some(anna), body).await;
    assert_eq!((s, detail(&e)), (StatusCode::BAD_REQUEST, "Categoria non valida per questo blog."));
    let body = json!({"slug": "due", "title": "2", "content": "c", "category_id": "non-un-uuid"});
    assert_eq!(env.post("/blogs/diario/posts", Some(anna), body).await.0, StatusCode::BAD_REQUEST);
    let n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM posts WHERE slug = 'due'").fetch_one(&env.pool).await.unwrap();
    assert_eq!(n, 0);

    // PATCH: assente = invariata, null = rimossa
    let path = format!("/posts/{}", id(&p));
    let (_, u) = env.patch(&path, Some(anna), json!({"title": "Nuovo"})).await;
    assert_eq!(u["category_id"], cat_id.as_str());
    let (_, u) = env.patch(&path, Some(anna), json!({"category_id": null})).await;
    assert_eq!(u["category"], Value::Null);
    let (_, u) = env.patch(&path, Some(anna), json!({"category_id": cat_id})).await;
    assert_eq!(u["category"]["slug"], "viaggi");

    // la traduzione eredita la categoria, a meno di una propria
    let tr = json!({"slug": "one", "locale": "en", "title": "One", "content": "c"});
    let (s, t) = env.post(&format!("/posts/{}/translations", id(&p)), Some(anna), tr).await;
    assert_eq!((s, t["category"]["slug"].as_str()), (StatusCode::CREATED, Some("viaggi")), "{t}");
    let (_, cat2) = env.post("/blogs/diario/categories", Some(anna), json!({"name": "Cibo", "slug": "cibo"})).await;
    let tr = json!({"slug": "uno-fr", "locale": "fr", "title": "Un", "content": "c", "category_id": id(&cat2)});
    let (_, t) = env.post(&format!("/posts/{}/translations", id(&p)), Some(anna), tr).await;
    assert_eq!(t["category"]["slug"], "cibo");

    // elenco: tutte le categorie in una query; cancellare la categoria lascia il post senza
    let (_, list) = env.get("/blogs/diario/posts", Some(anna)).await;
    let with_cat = list.as_array().unwrap().iter().filter(|p| p["category"].is_object()).count();
    assert!(with_cat >= 3, "{list}");
    assert_eq!(env.delete(&format!("/blogs/diario/categories/{cat_id}"), Some(anna)).await, StatusCode::NO_CONTENT);
    let (_, after) = env.get(&path, Some(anna)).await;
    assert_eq!((after["category"].clone(), after["category_id"].clone()), (Value::Null, Value::Null));
    env.teardown().await;
}

#[tokio::test]
async fn category_field_is_ignored_when_taxonomy_is_disabled() {
    let (env, anna) = world(&["publications"]).await;
    let p = new_post(&env, anna, "uno", json!({"category_id": Uuid::new_v4().to_string()})).await;
    assert!(p.get("category").is_none() && p.get("category_id").is_none());
    assert!(p.get("publication").is_some(), "le altre estensioni restano");
    env.teardown().await;
}

// ---- publications ------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn publication_crud_validation_and_visibility() {
    let (env, anna) = world(&["taxonomy", "publications", "pages"]).await;
    let base = "/blogs/diario/publications";
    let ok = json!({"name": "guida", "title": " La guida ", "description": "  "});
    assert_eq!(env.post(base, None, ok.clone()).await.0, StatusCode::UNAUTHORIZED);
    let stranger = env.user("estraneo", "utente").await;
    assert_eq!(env.post(base, Some(stranger), ok.clone()).await.0, StatusCode::FORBIDDEN);
    for bad in
        [json!({"name": "A B", "title": "t"}), json!({"name": "x", "title": "t"}), json!({"name": "ok", "title": "  "})]
    {
        assert_eq!(env.post(base, Some(anna), bad).await.0, StatusCode::BAD_REQUEST);
    }

    let (s, pubr) = env.post(base, Some(anna), ok.clone()).await;
    assert_eq!(s, StatusCode::CREATED, "{pubr}");
    assert_eq!((pubr["title"].as_str(), &pubr["description"]), (Some("La guida"), &Value::Null));
    assert_eq!((pubr["chapters_total"].clone(), pubr["chapters_published"].clone()), (json!(0), json!(0)));
    let (s, e) = env.post(base, Some(anna), ok).await;
    assert_eq!((s, detail(&e)), (StatusCode::CONFLICT, "Esiste già una pubblicazione con questo nome."));

    // senza capitoli pubblicati i lettori non la vedono; chi scrive sì
    assert_eq!(env.get(base, None).await.1, json!([]));
    assert_eq!(env.get(base, Some(anna)).await.1.as_array().unwrap().len(), 1);
    assert_eq!(env.get(&format!("{base}/guida"), None).await.0, StatusCode::NOT_FOUND);

    let pid = id(&pubr);
    let (s, u) = env
        .patch(&format!("{base}/{pid}"), Some(anna), json!({"name": "manuale", "description": "Per iniziare"}))
        .await;
    assert_eq!(
        (s, u["name"].as_str(), u["description"].as_str()),
        (StatusCode::OK, Some("manuale"), Some("Per iniziare"))
    );
    assert_eq!(
        env.get(&format!("{base}/guida"), Some(anna)).await.0,
        StatusCode::NOT_FOUND,
        "il vecchio nome non risolve"
    );
    env.post(base, Some(anna), json!({"name": "altra", "title": "Altra"})).await;
    let (s, _) = env.patch(&format!("{base}/altra"), Some(anna), json!({"name": "manuale"})).await;
    assert_eq!(s, StatusCode::CONFLICT);

    assert_eq!(env.delete(&format!("{base}/altra"), Some(stranger)).await, StatusCode::FORBIDDEN);
    assert_eq!(env.delete(&format!("{base}/altra"), Some(anna)).await, StatusCode::NO_CONTENT);
    assert_eq!(env.delete(&format!("{base}/altra"), Some(anna)).await, StatusCode::NOT_FOUND);
    env.teardown().await;
}

#[tokio::test]
async fn chapters_are_ordered_filtered_and_follow_the_post() {
    let (env, anna) = world(&["taxonomy", "publications", "pages"]).await;
    let base = "/blogs/diario/publications";
    let (_, pubr) = env.post(base, Some(anna), json!({"name": "guida", "title": "Guida"})).await;
    let pid = id(&pubr);

    let a = new_post(&env, anna, "a", json!({"publication_id": pid})).await;
    let b = new_post(&env, anna, "b", json!({"publication_id": pid})).await;
    let c = new_post(&env, anna, "c", json!({"publication_id": pid})).await;
    assert_eq!(a["publication"], json!({"id": pid, "name": "guida", "title": "Guida"}));
    assert_eq!(a["chapter_order"], Value::Null);
    publish(&env, anna, &a).await;
    publish(&env, anna, &b).await;

    // lettore: solo i pubblicati; autore: tutti, con is_public
    let (s, d) = env.get(&format!("{base}/guida"), None).await;
    assert_eq!(s, StatusCode::OK, "{d}");
    let slugs = |d: &Value| {
        d["chapters"].as_array().unwrap().iter().map(|c| c["slug"].as_str().unwrap().to_string()).collect::<Vec<_>>()
    };
    assert_eq!(slugs(&d), ["a", "b"]);
    assert_eq!((d["chapters_total"].clone(), d["chapters_published"].clone()), (json!(3), json!(2)));
    assert_eq!(d["chapters"][0]["n"], 1);
    assert_eq!(d["chapters"][0]["permalink"], "/diario/a");
    assert_eq!(d["chapters"][0]["reading_minutes"], 1);
    let (_, d) = env.get(&format!("{base}/{pid}"), Some(anna)).await;
    // cronologico: una bozza conta dalla creazione, i pubblicati dalla pubblicazione (come nella v1)
    assert_eq!(slugs(&d), ["c", "a", "b"]);
    assert_eq!(d["chapters"][0]["is_public"], false);
    assert_eq!(env.get(base, None).await.1.as_array().unwrap().len(), 1, "ora ha capitoli pubblicati");

    // ordine esplicito: i non elencati seguono in ordine cronologico
    let stranger = env.user("estraneo", "utente").await;
    let order = json!({"post_ids": [id(&c), id(&b)]});
    assert_eq!(env.put(&format!("{base}/guida/order"), Some(stranger), order.clone()).await.0, StatusCode::FORBIDDEN);
    let (s, d) = env.put(&format!("{base}/guida/order"), Some(anna), order).await;
    assert_eq!(s, StatusCode::OK, "{d}");
    assert_eq!(slugs(&d), ["c", "b", "a"]);
    let (_, one) = env.get(&format!("/posts/{}", id(&c)), Some(anna)).await;
    assert_eq!(one["chapter_order"], 1);

    // un post estraneo alla pubblicazione, o un id non valido, annulla il riordino
    let other = new_post(&env, anna, "fuori", json!({})).await;
    let (s, e) = env.put(&format!("{base}/guida/order"), Some(anna), json!({"post_ids": [id(&other)]})).await;
    assert_eq!((s, detail(&e)), (StatusCode::BAD_REQUEST, "Alcuni post non appartengono a questa pubblicazione."));
    assert_eq!(
        env.put(&format!("{base}/guida/order"), Some(anna), json!({"post_ids": ["x"]})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (_, d) = env.get(&format!("{base}/guida"), Some(anna)).await;
    assert_eq!(slugs(&d), ["c", "b", "a"], "ordine invariato");

    // tri-stato sul post; cambiare pubblicazione azzera la posizione
    let path = format!("/posts/{}", id(&c));
    let (_, u) = env.patch(&path, Some(anna), json!({"title": "X"})).await;
    assert_eq!(u["chapter_order"], 1, "assente: invariata");
    let (_, p2) = env.post(base, Some(anna), json!({"name": "seconda", "title": "Seconda"})).await;
    let (_, u) = env.patch(&path, Some(anna), json!({"publication_id": id(&p2)})).await;
    assert_eq!((u["publication"]["name"].as_str(), &u["chapter_order"]), (Some("seconda"), &Value::Null));
    let (_, u) = env.patch(&path, Some(anna), json!({"publication_id": null})).await;
    assert_eq!((u["publication"].clone(), u["publication_id"].clone()), (Value::Null, Value::Null));

    // la traduzione resta nella pubblicazione dell'originale; pubblicazione di un altro blog: 400
    let tr = json!({"slug": "a-en", "locale": "en", "title": "A", "content": "c"});
    let (_, t) = env.post(&format!("/posts/{}/translations", id(&a)), Some(anna), tr).await;
    assert_eq!(t["publication"]["name"], "guida");
    let bruno = env.user("bruno", "utente").await;
    env.blog("altro", bruno, "public").await;
    let (_, foreign) = env.post("/blogs/altro/publications", Some(bruno), json!({"name": "mia", "title": "Mia"})).await;
    let (s, _) = env.patch(&path, Some(anna), json!({"publication_id": id(&foreign)})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    // cancellare la pubblicazione lascia i post, senza pubblicazione
    assert_eq!(env.delete(&format!("{base}/guida"), Some(anna)).await, StatusCode::NO_CONTENT);
    let (_, after) = env.get(&format!("/posts/{}", id(&a)), Some(anna)).await;
    assert_eq!((after["publication"].clone(), after["chapter_order"].clone()), (Value::Null, Value::Null));
    env.teardown().await;
}

// ---- pages -------------------------------------------------------------------------------------------------------------

fn page(slug: &str, locale: &str, published: bool) -> Value {
    json!({"slug": slug, "locale": locale, "title": format!("Titolo {slug}"), "content": "Testo.", "is_published": published})
}

#[tokio::test]
async fn blog_pages_need_the_flag_and_follow_draft_visibility() {
    let (env, anna) = world(&["taxonomy", "publications", "pages"]).await;
    let base = "/blogs/diario/pages";
    let stranger = env.user("estraneo", "utente").await;

    // funzione spenta per default; solo il proprietario la attiva
    assert_eq!(env.get("/blogs/diario", None).await.1["static_pages_enabled"], false);
    assert_eq!(env.post(base, None, page("chi", "it", true)).await.0, StatusCode::UNAUTHORIZED);
    let (s, e) = env.post(base, Some(anna), page("chi", "it", true)).await;
    assert_eq!((s, detail(&e)), (StatusCode::FORBIDDEN, "Le pagine statiche non sono attive per questo blog."));
    assert_eq!(
        env.patch("/blogs/diario", Some(stranger), json!({"static_pages_enabled": true})).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        env.patch("/blogs/diario", Some(anna), json!({"static_pages_enabled": "si"})).await.0,
        StatusCode::BAD_REQUEST
    );
    let (_, b) = env.patch("/blogs/diario", Some(anna), json!({"static_pages_enabled": true})).await;
    assert_eq!(b["static_pages_enabled"], true);

    assert_eq!(env.post(base, Some(stranger), page("chi", "it", true)).await.0, StatusCode::FORBIDDEN);
    for bad in [page("Chi Siamo", "it", true), page("chi", "ita", true)] {
        assert_eq!(env.post(base, Some(anna), bad).await.0, StatusCode::BAD_REQUEST);
    }
    let (s, chi) = env.post(base, Some(anna), page("chi-siamo", "it", true)).await;
    assert_eq!(s, StatusCode::CREATED, "{chi}");
    assert_eq!(chi["permalink"], "/diario/pagina/chi-siamo");
    assert_eq!((chi["mentions_enabled"].clone(), chi["blog_id"].is_string()), (json!(true), true));
    let (s, e) = env.post(base, Some(anna), page("chi-siamo", "it", false)).await;
    assert_eq!((s, detail(&e)), (StatusCode::CONFLICT, "Slug già in uso per questa lingua."));
    let (_, bozza) = env.post(base, Some(anna), page("bozza", "it", false)).await;

    // lettura: le bozze solo a chi scrive; `locale` obbligatorio
    assert_eq!(env.get(&format!("{base}/chi-siamo?locale=it"), None).await.0, StatusCode::OK);
    assert_eq!(env.get(&format!("{base}/chi-siamo?locale=en"), None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get(&format!("{base}/bozza?locale=it"), None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get(&format!("{base}/bozza?locale=it"), Some(stranger)).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get(&format!("{base}/bozza?locale=it"), Some(anna)).await.0, StatusCode::OK);
    assert_eq!(env.get(&format!("{base}/bozza"), Some(anna)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(env.get(&format!("{base}?locale=it"), None).await.1.as_array().unwrap().len(), 1);
    assert_eq!(env.get(&format!("{base}?locale=it"), Some(anna)).await.1.as_array().unwrap().len(), 2);
    let by_id = format!("{base}/by-id/{}", id(&bozza));
    assert_eq!(env.get(&by_id, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.get(&by_id, Some(stranger)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(env.get(&by_id, Some(anna)).await.1["slug"], "bozza");

    // traduzioni: una per lingua, elencate solo le pubblicate
    let tr = |slug: &str, locale: &str, p: bool| page(slug, locale, p);
    let url = format!("{base}/{}/translations", id(&chi));
    let (s, en) = env.post(&url, Some(anna), tr("about", "en", true)).await;
    assert_eq!((s, en["translation_group_id"].clone()), (StatusCode::CREATED, chi["translation_group_id"].clone()));
    let (s, e) = env.post(&url, Some(anna), tr("about-2", "en", true)).await;
    assert_eq!((s, detail(&e)), (StatusCode::CONFLICT, "Traduzione già presente per questa lingua."));
    env.post(&url, Some(anna), tr("a-fr", "fr", false)).await;
    let (_, list) = env.get(&url, None).await;
    let locales: Vec<_> = list.as_array().unwrap().iter().map(|t| t["locale"].as_str().unwrap()).collect();
    assert_eq!(locales, ["en", "it"]);

    // modifica: conflitto di slug; resta possibile a funzione spenta
    let path = format!("{base}/{}", id(&bozza));
    assert_eq!(env.patch(&path, Some(anna), json!({"slug": "chi-siamo"})).await.0, StatusCode::CONFLICT);
    let (_, u) = env.patch(&path, Some(anna), json!({"slug": "nuova", "is_published": true, "title": "N"})).await;
    assert_eq!(
        (u["slug"].as_str(), u["is_published"].clone(), u["permalink"].as_str()),
        (Some("nuova"), json!(true), Some("/diario/pagina/nuova"))
    );
    env.patch("/blogs/diario", Some(anna), json!({"static_pages_enabled": false})).await;
    assert_eq!(env.patch(&path, Some(anna), json!({"title": "Ancora"})).await.0, StatusCode::OK);
    assert_eq!(env.post(base, Some(anna), page("x", "it", true)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(env.delete(&path, Some(stranger)).await, StatusCode::FORBIDDEN);
    assert_eq!(env.delete(&path, Some(anna)).await, StatusCode::NO_CONTENT);
    assert_eq!(env.delete(&path, Some(anna)).await, StatusCode::NOT_FOUND);

    let actions: Vec<String> =
        sqlx::query_scalar("SELECT action FROM audit_log WHERE action LIKE 'page.%' ORDER BY occurred_at, id")
            .fetch_all(&env.pool)
            .await
            .unwrap();
    assert_eq!(actions.iter().filter(|a| *a == "page.created").count(), 2);
    assert!(actions.contains(&"page.updated".to_string()) && actions.contains(&"page.deleted".to_string()));
    env.teardown().await;
}

#[tokio::test]
async fn platform_pages_are_managed_by_admins_and_read_publicly() {
    let (env, anna) = world(&["taxonomy", "publications", "pages"]).await;
    let admin = env.user("admin", "amministratore").await;
    assert_eq!(env.post("/pages", None, page("privacy", "it", true)).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.post("/pages", Some(anna), page("privacy", "it", true)).await.0, StatusCode::FORBIDDEN);

    let (s, p) = env.post("/pages", Some(admin), page("privacy", "it", true)).await;
    assert_eq!(s, StatusCode::CREATED, "{p}");
    assert_eq!(
        (p["permalink"].as_str(), &p["blog_id"], p["mentions_enabled"].clone()),
        (Some("/p/privacy"), &Value::Null, json!(true))
    );
    assert_eq!(env.post("/pages", Some(admin), page("privacy", "it", true)).await.0, StatusCode::CONFLICT);
    env.post("/pages", Some(admin), page("bozza", "it", false)).await;
    // lo stesso slug in un blog non collide con quello del sito
    env.patch("/blogs/diario", Some(anna), json!({"static_pages_enabled": true})).await;
    assert_eq!(env.post("/blogs/diario/pages", Some(anna), page("privacy", "it", true)).await.0, StatusCode::CREATED);

    assert_eq!(env.get("/pages/privacy?locale=it", None).await.0, StatusCode::OK);
    assert_eq!(env.get("/pages/bozza?locale=it", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/pages/bozza?locale=it", Some(anna)).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/pages/bozza?locale=it", Some(admin)).await.0, StatusCode::OK);
    assert_eq!(env.get("/pages?locale=it", None).await.1.as_array().unwrap().len(), 1);
    assert_eq!(env.get("/pages?locale=it", Some(admin)).await.1.as_array().unwrap().len(), 2);
    assert_eq!(env.get("/pages?locale=it&q=PRIV", Some(admin)).await.1.as_array().unwrap().len(), 1);
    assert_eq!(env.get("/pages?locale=it&q=%25", Some(admin)).await.1, json!([]), "il % è letterale");

    let url = format!("/pages/{}/translations", id(&p));
    assert_eq!(env.post(&url, Some(anna), page("privacy-en", "en", true)).await.0, StatusCode::FORBIDDEN);
    let (s, _) = env.post(&url, Some(admin), page("privacy-en", "en", true)).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(env.post(&url, Some(admin), page("altro", "en", true)).await.0, StatusCode::CONFLICT);
    assert_eq!(env.get(&url, None).await.1.as_array().unwrap().len(), 2);
    assert_eq!(env.get("/pages/non-uuid/translations", None).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    let one = format!("/pages/{}", id(&p));
    assert_eq!(env.patch(&one, Some(anna), json!({"title": "X"})).await.0, StatusCode::FORBIDDEN);
    let (_, u) = env.patch(&one, Some(admin), json!({"title": "Nuovo", "is_published": false})).await;
    assert_eq!((u["title"].as_str(), u["is_published"].clone()), (Some("Nuovo"), json!(false)));
    assert_eq!(
        env.patch(&format!("/pages/{}", Uuid::new_v4()), Some(admin), json!({"title": "X"})).await.0,
        StatusCode::NOT_FOUND
    );
    env.teardown().await;
}

#[tokio::test]
async fn blog_flag_is_ignored_when_pages_is_disabled() {
    let (env, anna) = world(&["taxonomy"]).await;
    let (_, b) = env.patch("/blogs/diario", Some(anna), json!({"static_pages_enabled": true})).await;
    assert!(b.get("static_pages_enabled").is_none());
    assert_eq!(env.get("/pages?locale=it", None).await.0, StatusCode::NOT_FOUND);
    env.teardown().await;
}

// ---- link_preview ------------------------------------------------------------------------------------------------------

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// Server HTTP minimo: `/ok` pagina con Open Graph, `/go` redirect, `/json` non HTML, il resto 404.
async fn serve() -> (String, Arc<AtomicUsize>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { return };
            let counter = counter.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                counter.fetch_add(1, Ordering::SeqCst);
                let (status, ctype, extra, body) = match path.as_str() {
                    "/ok" => (
                        "200 OK",
                        "text/html; charset=utf-8",
                        "",
                        "<html><head><title>Grezzo</title><meta property=\"og:title\" content=\"Titolo OG\">\
                         <meta property=\"og:description\" content=\"Una descrizione\">\
                         <meta property=\"og:image\" content=\"https://x.org/i.png\"></head><body>corpo</body></html>",
                    ),
                    "/go" => ("302 Found", "text/html", "Location: http://127.0.0.1:1/segreto\r\n", ""),
                    "/json" => ("200 OK", "application/json", "", "{\"a\":1}"),
                    _ => ("404 Not Found", "text/html", "", "<title>no</title>"),
                };
                let resp = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            });
        }
    });
    (base, hits)
}

fn q(url: &str) -> String {
    format!("/link-preview?url={}", url.replace(':', "%3A").replace('/', "%2F").replace(' ', "%20"))
}

#[tokio::test]
async fn link_preview_rejects_unsafe_urls_by_default() {
    let (env, _) = world(&["link_preview"]).await;
    let (base, hits) = serve().await;
    assert_eq!(env.get("/link-preview", None).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    for bad in ["ftp://example.com/", "javascript:alert(1)", "non un url", "http://169.254.169.254/latest/meta-data"] {
        assert_eq!(env.get(&q(bad), None).await.0, StatusCode::BAD_REQUEST, "{bad}");
    }
    // il server di prova è su loopback: senza consenso esplicito non viene contattato
    let (s, e) = env.get(&q(&format!("{base}/ok")), None).await;
    assert_eq!((s, detail(&e)), (StatusCode::BAD_REQUEST, "URL non raggiungibile per l'anteprima."));
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    env.teardown().await;
}

#[tokio::test]
async fn link_preview_fetches_caches_and_refreshes() {
    let env = setup(&["link_preview"], &[("NOCT_LINK_PREVIEW_ALLOW_PRIVATE", "true")]).await;
    let (base, hits) = serve().await;
    let url = format!("{base}/ok");

    let (s, p) = env.get(&q(&url), None).await;
    assert_eq!(s, StatusCode::OK, "{p}");
    assert_eq!(
        p,
        json!({"url": url, "title": "Titolo OG", "description": "Una descrizione", "image": "https://x.org/i.png"})
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    let (_, again) = env.get(&q(&url), None).await;
    assert_eq!((again, hits.load(Ordering::SeqCst)), (p.clone(), 1), "cache calda: nessun nuovo fetch");

    // senza cache calda risponde la tabella, ancora senza fetch
    let key = format!("link-preview:{}", sha_hex(&url));
    env.state.services.cache.delete(&key).await.unwrap();
    assert_eq!(env.get(&q(&url), None).await.1, p);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    let ok: bool = sqlx::query_scalar("SELECT fetch_ok FROM link_preview_cache").fetch_one(&env.pool).await.unwrap();
    assert!(ok);

    // scaduta (più di 7 giorni): rifà il fetch e aggiorna la stessa riga
    sqlx::query("UPDATE link_preview_cache SET updated_at = now() - interval '8 days'")
        .execute(&env.pool)
        .await
        .unwrap();
    env.state.services.cache.delete(&key).await.unwrap();
    assert_eq!(env.get(&q(&url), None).await.1, p);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM link_preview_cache").fetch_one(&env.pool).await.unwrap();
    assert_eq!(rows, 1, "una riga per URL");

    // errori, non HTML e redirect: anteprima con il solo url (i redirect non si seguono)
    for path in ["missing", "json", "go"] {
        let u = format!("{base}/{path}");
        let (s, p) = env.get(&q(&u), None).await;
        assert_eq!((s, p), (StatusCode::OK, json!({"url": u, "title": null, "description": null, "image": null})));
    }
    let empty: i64 = sqlx::query_scalar("SELECT count(*) FROM link_preview_cache WHERE NOT fetch_ok")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(empty, 3);
    // un fallimento si riprova dopo 6 ore, non prima
    let before = hits.load(Ordering::SeqCst);
    let missing = format!("{base}/missing");
    env.state.services.cache.delete(&format!("link-preview:{}", sha_hex(&missing))).await.unwrap();
    env.get(&q(&missing), None).await;
    assert_eq!(hits.load(Ordering::SeqCst), before, "ancora fresca");
    sqlx::query("UPDATE link_preview_cache SET updated_at = now() - interval '7 hours' WHERE NOT fetch_ok")
        .execute(&env.pool)
        .await
        .unwrap();
    env.state.services.cache.delete(&format!("link-preview:{}", sha_hex(&missing))).await.unwrap();
    env.get(&q(&missing), None).await;
    assert_eq!(hits.load(Ordering::SeqCst), before + 1);
    env.teardown().await;
}

fn sha_hex(url: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(url.trim().as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}
