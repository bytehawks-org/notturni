//! Mappa dei collegamenti: nodi e archi da tag, categorie, link citati nei post e link interni, limitati a ciò che
//! chi chiede può vedere; scope della piattaforma solo agli amministratori.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use uuid::Uuid;

async fn post(env: &Env, who: Uuid, blog: &str, slug: &str, content: &str, publish: bool, extra: Value) -> Value {
    let mut body = json!({"slug": slug, "title": format!("Titolo {slug}"), "content": content});
    body.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    let (s, p) = env.post(&format!("/blogs/{blog}/posts"), Some(who), body).await;
    assert_eq!(s, StatusCode::CREATED, "{p}");
    if publish {
        let (s, b) = env.post(&format!("/posts/{}/publish", p["id"].as_str().unwrap()), Some(who), json!({})).await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    p
}

fn ids(g: &Value, key: &str, ty: &str) -> Vec<String> {
    let mut v: Vec<String> = g[key]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["type"] == ty)
        .map(|n| n["label"].as_str().or(n["target"].as_str()).unwrap_or_default().to_string())
        .collect();
    v.sort();
    v
}

fn labels(g: &Value, ty: &str) -> Vec<String> {
    ids(g, "nodes", ty)
}

fn links(g: &Value) -> Vec<(String, String, String)> {
    let label = |id: &str| {
        g["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == id)
            .map_or("?".into(), |n| n["label"].as_str().unwrap().to_string())
    };
    let mut v: Vec<_> = g["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "link")
        .map(|e| {
            (
                label(e["source"].as_str().unwrap()),
                label(e["target"].as_str().unwrap()),
                e["via"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    v.sort();
    v
}

#[tokio::test]
async fn the_graph_links_posts_through_tags_categories_urls_and_wiki_links() {
    let env = setup(&["graph", "taxonomy", "wiki"], &[("NOCT_EXPERIMENTAL_FEATURES", "wiki")]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    let admin = env.user("ada", "amministratore").await;
    env.blog("diario", anna, "public").await;
    env.blog("riservato", anna, "private").await;
    env.blog("altrui", bruno, "public").await;
    let (s, _) = env.post("/blogs", Some(anna), json!({"slug": "guida", "title": "Guida", "kind": "wiki"})).await;
    assert_eq!(s, StatusCode::CREATED);
    let (_, cat) =
        env.post("/blogs/diario/categories", Some(anna), json!({"name": "Appunti", "slug": "appunti"})).await;

    post(
        &env,
        anna,
        "diario",
        "a",
        "Vedi [b](https://notturni.eu/diario/b) e [fuori](https://notturni.eu/altrui/x) #rust #uno",
        true,
        json!({"category_id": cat["id"]}),
    )
    .await;
    post(&env, anna, "diario", "b", "Rimando ad [a](/diario/a) #rust", true, json!({"category_id": cat["id"]})).await;
    post(&env, anna, "diario", "bozza", "Una bozza #segreto", false, json!({})).await;
    post(&env, anna, "guida", "home", "Vai a [[install]]", true, json!({})).await;
    post(&env, anna, "guida", "install", "Fine", true, json!({})).await;
    post(&env, anna, "riservato", "privato", "Privato #rust", true, json!({})).await;
    post(&env, bruno, "altrui", "x", "Di Bruno #rust", true, json!({})).await;
    let g = |who, q: &str| {
        let env = &env;
        let path = format!("/graph{q}");
        async move { env.get(&path, who).await }
    };

    assert_eq!(g(None, "").await.0, StatusCode::UNAUTHORIZED);

    // i propri Spazi, solo pubblicato: nessuna bozza, nessun contenuto altrui
    let (s, graph) = g(Some(anna), "").await;
    assert_eq!(s, StatusCode::OK, "{graph}");
    assert_eq!((graph["scope"].as_str(), graph["truncated"].as_bool()), (Some("mine"), Some(false)));
    assert_eq!(labels(&graph, "post"), ["Titolo a", "Titolo b", "Titolo home", "Titolo install", "Titolo privato"]);
    assert_eq!(labels(&graph, "blog"), ["Guida", "diario", "riservato"]);
    assert_eq!(labels(&graph, "tag"), ["#rust", "#uno"], "il tag della bozza e quello di Bruno non compaiono");
    assert_eq!(labels(&graph, "category"), ["Appunti"]);
    assert_eq!(
        links(&graph),
        [
            ("Titolo a".into(), "Titolo b".into(), "url".into()),
            ("Titolo b".into(), "Titolo a".into(), "url".into()),
            ("Titolo home".into(), "Titolo install".into(), "wiki".into()),
        ],
        "il link verso lo Spazio di Bruno non è nell'insieme"
    );

    // bozze solo su richiesta; filtri per Spazio e tipo; Spazio sconosciuto → 404
    assert_eq!(labels(&g(Some(anna), "?drafts=true").await.1, "post").len(), 6);
    assert_eq!(labels(&g(Some(anna), "?drafts=true").await.1, "tag"), ["#rust", "#segreto", "#uno"]);
    assert_eq!(labels(&g(Some(anna), "?blog=diario").await.1, "post"), ["Titolo a", "Titolo b"]);
    assert_eq!(labels(&g(Some(anna), "?kind=wiki").await.1, "post"), ["Titolo home", "Titolo install"]);
    assert_eq!(g(Some(anna), "?blog=altrui").await.0, StatusCode::NOT_FOUND, "non è un suo Spazio");
    for bad in ["?scope=tutto", "?kind=forum", "?limit=abc"] {
        assert_eq!(g(Some(anna), bad).await.0, StatusCode::BAD_REQUEST, "{bad}");
    }
    let limited = g(Some(anna), "?limit=2").await.1;
    assert_eq!((labels(&limited, "post").len(), limited["truncated"].as_bool()), (2, Some(true)));

    // un Super Admin o amministratore vede il pubblico di tutti (mai bozze né Spazi privati); gli altri no
    assert_eq!(g(Some(anna), "?scope=platform").await.0, StatusCode::FORBIDDEN);
    let (s, all) = g(Some(admin), "?scope=platform&drafts=true").await;
    assert_eq!(s, StatusCode::OK, "{all}");
    assert_eq!(labels(&all, "post"), ["Titolo a", "Titolo b", "Titolo home", "Titolo install", "Titolo x"]);
    assert!(
        links(&all).contains(&("Titolo a".into(), "Titolo x".into(), "url".into())),
        "ora il link verso Bruno esiste"
    );
    // Bruno vede solo il suo
    assert_eq!(labels(&g(Some(bruno), "").await.1, "post"), ["Titolo x"]);
    env.teardown().await;
}
