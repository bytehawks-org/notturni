//! Modulo `wiki`: albero delle pagine, cronologia delle revisioni con differenze, link interni e
//! backlink. Router completo + Postgres vero.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use uuid::Uuid;

/// Un utente con uno Spazio del tipo `kind`.
async fn space(env: &Env, owner: Uuid, slug: &str, kind: &str) {
    env.blog(slug, owner, "public").await;
    sqlx::query("UPDATE blogs SET kind = $2::space_kind WHERE slug = $1")
        .bind(slug)
        .bind(kind)
        .execute(&env.pool)
        .await
        .unwrap();
}

async fn page(env: &Env, who: Uuid, blog: &str, slug: &str, extra: Value) -> Value {
    let mut body = json!({"slug": slug, "title": format!("Pagina {slug}"), "content": format!("testo di {slug}")});
    body.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    let (s, b) = env.post(&format!("/blogs/{blog}/posts"), Some(who), body).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    b
}

async fn publish(env: &Env, who: Uuid, p: &Value) {
    let (s, b) = env.post(&format!("/posts/{}/publish", id(p)), Some(who), json!({})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

fn id(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

fn slugs(outline: &Value) -> Vec<(String, i64)> {
    outline
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["slug"].as_str().unwrap().to_string(), n["depth"].as_i64().unwrap()))
        .collect()
}

fn pairs(list: &[(&str, i64)]) -> Vec<(String, i64)> {
    list.iter().map(|(s, d)| (s.to_string(), *d)).collect()
}

#[tokio::test]
async fn pages_form_a_tree_that_can_be_moved_and_reordered() {
    let env = setup(&["wiki"], &[]).await;
    let anna = env.user("anna", "utente").await;
    space(&env, anna, "manuale", "docs").await;

    let intro = page(&env, anna, "manuale", "intro", json!({})).await;
    assert_eq!(
        (intro["parent_id"].clone(), intro["position"].clone(), intro["revisions"].clone()),
        (Value::Null, json!(0), json!(1))
    );
    let setup_p = page(&env, anna, "manuale", "install", json!({"parent_id": id(&intro)})).await;
    assert_eq!(setup_p["parent_id"], json!(id(&intro)));
    let deep = page(&env, anna, "manuale", "deep", json!({"parent_id": id(&setup_p)})).await;
    let other = page(&env, anna, "manuale", "api", json!({})).await;
    assert_eq!(other["position"], 1, "una nuova radice va in coda ai fratelli");

    let (_, o) = env.get("/blogs/manuale/outline", Some(anna)).await;
    assert_eq!(slugs(&o), pairs(&[("intro", 0), ("install", 1), ("deep", 2), ("api", 0)]));

    // cicli, padri di altri Spazi, profondità
    let (s, b) = env.patch(&format!("/posts/{}", id(&intro)), Some(anna), json!({"parent_id": id(&deep)})).await;
    assert_eq!((s, detail(&b).contains("sotto sé stessa")), (StatusCode::BAD_REQUEST, true), "{b}");
    let (s, _) = env.patch(&format!("/posts/{}", id(&intro)), Some(anna), json!({"parent_id": id(&intro)})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = env.patch(&format!("/posts/{}", id(&intro)), Some(anna), json!({"parent_id": Uuid::new_v4()})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    // sposta `deep` sotto `api`, poi riordina in blocco
    let (s, b) = env.patch(&format!("/posts/{}", id(&deep)), Some(anna), json!({"parent_id": id(&other)})).await;
    assert_eq!((s, b["parent_id"].clone()), (StatusCode::OK, json!(id(&other))), "{b}");
    let items = json!({"items": [
        {"id": id(&other), "parent_id": null},
        {"id": id(&deep), "parent_id": id(&other)},
        {"id": id(&intro), "parent_id": null},
        {"id": id(&setup_p), "parent_id": id(&intro)},
    ]});
    let (s, o) = env.put("/blogs/manuale/outline", Some(anna), items).await;
    assert_eq!(s, StatusCode::OK, "{o}");
    assert_eq!(slugs(&o), pairs(&[("api", 0), ("deep", 1), ("intro", 0), ("install", 1)]));

    // l'elenco deve coincidere con i post dello Spazio
    let partial = json!({"items": [{"id": id(&other), "parent_id": null}]});
    assert_eq!(env.put("/blogs/manuale/outline", Some(anna), partial).await.0, StatusCode::CONFLICT);
    let cycle = json!({"items": [
        {"id": id(&other), "parent_id": id(&deep)}, {"id": id(&deep), "parent_id": id(&other)},
        {"id": id(&intro), "parent_id": null}, {"id": id(&setup_p), "parent_id": null},
    ]});
    assert_eq!(env.put("/blogs/manuale/outline", Some(anna), cycle).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(env.put("/blogs/manuale/outline", None, json!({"items": []})).await.0, StatusCode::UNAUTHORIZED);

    // eliminare un padre porta i figli alla radice
    assert_eq!(env.delete(&format!("/posts/{}", id(&other)), Some(anna)).await, StatusCode::NO_CONTENT);
    let (_, o) = env.get("/blogs/manuale/outline", Some(anna)).await;
    assert_eq!(slugs(&o), pairs(&[("deep", 0), ("intro", 0), ("install", 1)]));

    // audit
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'wiki.outline_updated'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(n, 1);
    env.teardown().await;
}

#[tokio::test]
async fn blogs_have_no_hierarchy_and_readers_see_only_published_pages() {
    let env = setup(&["wiki"], &[]).await;
    let anna = env.user("anna", "utente").await;
    space(&env, anna, "diario", "blog").await;
    let (s, b) = env
        .post(
            "/blogs/diario/posts",
            Some(anna),
            json!({"slug": "a", "title": "A", "content": "x", "parent_id": Uuid::new_v4()}),
        )
        .await;
    assert_eq!((s, detail(&b).contains("wiki e docs")), (StatusCode::BAD_REQUEST, true), "{b}");
    let p = page(&env, anna, "diario", "a", json!({})).await;
    assert_eq!(p["revisions"], 0, "nei blog non ci sono revisioni");
    assert_eq!(env.put("/blogs/diario/outline", Some(anna), json!({"items": []})).await.0, StatusCode::BAD_REQUEST);

    space(&env, anna, "wiki-uno", "wiki").await;
    let a = page(&env, anna, "wiki-uno", "pubblica", json!({})).await;
    let b = page(&env, anna, "wiki-uno", "bozza", json!({"parent_id": id(&a)})).await;
    let c = page(&env, anna, "wiki-uno", "figlia", json!({"parent_id": id(&b)})).await;
    publish(&env, anna, &a).await;
    publish(&env, anna, &c).await;
    let (_, public) = env.get("/blogs/wiki-uno/outline", None).await;
    assert_eq!(
        slugs(&public),
        pairs(&[("pubblica", 0), ("figlia", 0)]),
        "la figlia sale alla radice se il padre non è visibile"
    );
    let (_, writer) = env.get("/blogs/wiki-uno/outline", Some(anna)).await;
    assert_eq!(slugs(&writer), pairs(&[("pubblica", 0), ("bozza", 1), ("figlia", 2)]));
    assert_eq!(env.get("/blogs/wiki-uno/outline?locale=en", Some(anna)).await.1, json!([]));
    env.teardown().await;
}

#[tokio::test]
async fn revisions_record_changes_with_diffs_and_are_for_writers_only() {
    let env = setup(&["wiki"], &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    space(&env, anna, "guida", "wiki").await;
    let p = page(&env, anna, "guida", "home", json!({"content": "uno\ndue\ntre"})).await;
    let url = format!("/posts/{}", id(&p));

    // lo stesso testo non crea una revisione; una modifica sì, con il riassunto
    let (s, b) = env.patch(&url, Some(anna), json!({"content": "uno\ndue\ntre"})).await;
    assert_eq!((s, b["revisions"].clone()), (StatusCode::OK, json!(1)), "{b}");
    let (_, b) = env
        .patch(&url, Some(anna), json!({"content": "uno\nDUE\ntre\nquattro", "revision_summary": "maiuscole"}))
        .await;
    assert_eq!(b["revisions"], 2);
    let (_, b) = env.patch(&url, Some(anna), json!({"title": "Nuovo titolo"})).await;
    assert_eq!(b["revisions"], 3);
    let (s, _) = env.patch(&url, Some(anna), json!({"content": "x", "revision_summary": 5})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    let (s, list) = env.get(&format!("{url}/revisions"), Some(anna)).await;
    assert_eq!(s, StatusCode::OK, "{list}");
    let nums: Vec<_> = list.as_array().unwrap().iter().map(|r| r["revision"].as_i64().unwrap()).collect();
    assert_eq!(nums, [3, 2, 1]);
    assert_eq!((list[1]["summary"].clone(), list[1]["editor"].clone()), (json!("maiuscole"), json!("anna")));
    assert_eq!(list[2]["title"], "Pagina home");

    let (_, r2) = env.get(&format!("{url}/revisions/2"), Some(anna)).await;
    assert_eq!(r2["content"], "uno\nDUE\ntre\nquattro");
    let (s, d) = env.get(&format!("{url}/revisions/2/diff"), Some(anna)).await;
    assert_eq!(s, StatusCode::OK, "{d}");
    assert_eq!((d["from"].clone(), d["to"].clone()), (json!(1), json!(2)));
    assert_eq!(
        d["hunks"],
        json!([
            {"op": "equal", "text": "uno"}, {"op": "delete", "text": "due"}, {"op": "insert", "text": "DUE"},
            {"op": "equal", "text": "tre"}, {"op": "insert", "text": "quattro"},
        ])
    );
    let (_, first) = env.get(&format!("{url}/revisions/1/diff"), Some(anna)).await;
    assert_eq!(first["from"], 0, "la prima revisione si confronta con il testo vuoto");
    assert_eq!(env.get(&format!("{url}/revisions/9"), Some(anna)).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get(&format!("{url}/revisions/0"), Some(anna)).await.0, StatusCode::BAD_REQUEST);

    // ripristinare = salvare di nuovo il vecchio testo: nasce una nuova revisione
    let (_, r1) = env.get(&format!("{url}/revisions/1"), Some(anna)).await;
    let (_, b) =
        env.patch(&url, Some(anna), json!({"content": r1["content"].clone(), "revision_summary": "ripristino"})).await;
    assert_eq!(b["revisions"], 4);

    assert_eq!(env.get(&format!("{url}/revisions"), None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.get(&format!("{url}/revisions"), Some(bruno)).await.0, StatusCode::FORBIDDEN);
    env.teardown().await;
}

#[tokio::test]
async fn wikilinks_produce_backlinks_for_visible_pages_only() {
    let env = setup(&["wiki"], &[]).await;
    let anna = env.user("anna", "utente").await;
    space(&env, anna, "guida", "wiki").await;
    let target = page(&env, anna, "guida", "avvio-rapido", json!({})).await;
    let from_pub =
        page(&env, anna, "guida", "home", json!({"content": "Vai a [[Avvio rapido]] e a [[nonesiste|niente]]."})).await;
    let from_draft =
        page(&env, anna, "guida", "bozza", json!({"content": "Bozza: [[avvio-rapido]]\n```\n[[avvio-rapido]]\n```"}))
            .await;
    publish(&env, anna, &target).await;
    publish(&env, anna, &from_pub).await;
    let url = format!("/posts/{}/backlinks", id(&target));

    let (s, public) = env.get(&url, None).await;
    assert_eq!(s, StatusCode::OK, "{public}");
    let names =
        |v: &Value| v.as_array().unwrap().iter().map(|b| b["slug"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(names(&public), ["home"], "la bozza non compare ai lettori");
    let (_, writer) = env.get(&url, Some(anna)).await;
    assert_eq!(names(&writer), ["bozza", "home"]);

    // modificare il contenuto riscrive i link
    env.patch(&format!("/posts/{}", id(&from_draft)), Some(anna), json!({"content": "niente"})).await;
    let (_, writer) = env.get(&url, Some(anna)).await;
    assert_eq!(names(&writer), ["home"]);

    // la pagina di destinazione non pubblicata è invisibile ai lettori
    let hidden = page(&env, anna, "guida", "nascosta", json!({})).await;
    assert_eq!(env.get(&format!("/posts/{}/backlinks", id(&hidden)), None).await.0, StatusCode::NOT_FOUND);
    env.teardown().await;
}
