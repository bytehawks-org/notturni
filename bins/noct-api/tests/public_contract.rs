//! Rotte pubbliche della v1 che completano il contratto: configurazione e footer di piattaforma, SEO
//! (robots/sitemap) ed esportazione del blog.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use common::*;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::io::Read;
use tower::ServiceExt;
use uuid::Uuid;

const MODULES: &[&str] = &["comments", "pages", "taxonomy", "notes", "auth_sso", "profiles"];

async fn published(env: &Env, who: Uuid, blog: &str, slug: &str, extra: Value) -> String {
    let mut body = json!({"slug": slug, "title": slug, "content": format!("testo di {slug}")});
    body.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
    let (s, p) = env.post(&format!("/blogs/{blog}/posts"), Some(who), body).await;
    assert_eq!(s, StatusCode::CREATED, "{p}");
    let id = p["id"].as_str().unwrap().to_string();
    let (s, b) = env.post(&format!("/posts/{id}/publish"), Some(who), json!({})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    id
}

#[tokio::test]
async fn public_config_and_footer_are_readable_without_login_and_follow_the_settings() {
    let env = setup(
        MODULES,
        &[
            ("NOCT_OAUTH_GITHUB_CLIENT_ID", "id"),
            ("NOCT_OAUTH_GITHUB_CLIENT_SECRET", "s"),
            ("NOCT_OAUTH_GOOGLE_CLIENT_ID", "id"),
            ("NOCT_OAUTH_GOOGLE_CLIENT_SECRET", "s"),
        ],
    )
    .await;
    let root = env.user("root", "super_admin").await;
    let (s, cfg) = env.get("/config", None).await;
    assert_eq!(s, StatusCode::OK, "{cfg}");
    assert_eq!(cfg["deployment_mode"], "platform");
    assert_eq!(cfg["default_locale"], "it");
    assert_eq!(cfg["registration_mode"], "open");
    assert!(cfg["turnstile_site_key"].is_null());
    // nessun provider scelto: tutti quelli configurati
    assert_eq!(cfg["sso_providers"], json!(["github", "google"]));
    env.patch(
        "/admin/config",
        Some(root),
        json!({"sso_providers": ["github"], "default_locale": "en", "registration_mode": "closed"}),
    )
    .await;
    let (_, cfg) = env.get("/config", None).await;
    assert_eq!(cfg["sso_providers"], json!(["github"]));
    assert_eq!((cfg["default_locale"].as_str(), cfg["registration_mode"].as_str()), (Some("en"), Some("closed")));

    let (s, f) = env.get("/footer", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(f, json!({"column1": null, "column2": null, "column3": null, "bottom_bar": null}));
    env.patch(
        "/admin/config",
        Some(root),
        json!({"footer_column1_markdown": "**ciao**", "footer_bottom_bar_markdown": "© 2026"}),
    )
    .await;
    let (_, f) = env.get("/footer", None).await;
    assert_eq!(
        (f["column1"].as_str(), f["bottom_bar"].as_str(), f["column2"].is_null()),
        (Some("**ciao**"), Some("© 2026"), true)
    );
    env.teardown().await;

    // senza il modulo SSO l'elenco è comunque presente e vuoto
    let env = setup(&[], &[]).await;
    let (_, cfg) = env.get("/config", None).await;
    assert_eq!(cfg["sso_providers"], json!([]));
    env.teardown().await;
}

#[tokio::test]
async fn crawl_directives_apply_blog_and_post_opt_outs_separately_for_search_and_ai() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    env.blog("aperto", anna, "public").await;
    env.blog("chiuso", anna, "public").await;
    env.blog("riservato", anna, "private").await;
    let mixed = env.blog("misto", anna, "public").await;
    sqlx::query("UPDATE blogs SET search_indexing_enabled = FALSE, ai_crawling_enabled = FALSE WHERE slug = 'chiuso'")
        .execute(&env.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE blogs SET ai_crawling_enabled = FALSE WHERE id = $1")
        .bind(mixed)
        .execute(&env.pool)
        .await
        .unwrap();
    for (blog, slug) in [
        ("aperto", "libero"),
        ("aperto", "no-ia"),
        ("aperto", "no-ricerca"),
        ("chiuso", "dentro"),
        ("misto", "x"),
        ("misto", "forza"),
    ] {
        published(&env, anna, blog, slug, json!({})).await;
    }
    sqlx::query("UPDATE posts SET ai_crawling_enabled = FALSE WHERE slug = 'no-ia'").execute(&env.pool).await.unwrap();
    sqlx::query("UPDATE posts SET search_indexing_enabled = FALSE WHERE slug = 'no-ricerca'")
        .execute(&env.pool)
        .await
        .unwrap();
    // un override non riapre un blog escluso
    sqlx::query("UPDATE posts SET ai_crawling_enabled = TRUE WHERE slug = 'forza'").execute(&env.pool).await.unwrap();
    // una bozza e un post nascosto non compaiono
    env.post("/blogs/aperto/posts", Some(anna), json!({"slug": "bozza", "title": "b", "content": "c"})).await;

    let (s, d) = env.get("/seo/crawl-directives", None).await;
    assert_eq!(s, StatusCode::OK, "{d}");
    assert_eq!(d["search_disallow"], json!(["/aperto/no-ricerca", "/chiuso"]));
    assert_eq!(
        d["ai_disallow"],
        json!(["/aperto/no-ia", "/aperto/no-ricerca", "/chiuso", "/misto", "/misto/forza", "/misto/x"])
    );
    assert!(d["ai_user_agents"].as_array().unwrap().iter().any(|a| a == "GPTBot"));

    let (_, map) = env.get("/seo/sitemap-entries", None).await;
    let blogs: Vec<&str> = map["blogs"].as_array().unwrap().iter().map(|b| b["slug"].as_str().unwrap()).collect();
    assert_eq!(blogs, vec!["aperto", "misto"]);
    let posts: Vec<&str> = map["posts"].as_array().unwrap().iter().map(|p| p["permalink"].as_str().unwrap()).collect();
    assert_eq!(posts, vec!["/aperto/libero", "/aperto/no-ia", "/misto/forza", "/misto/x"]);
    assert!(map["posts"][0]["updated_at"].is_string());
    env.teardown().await;
}

async fn download(env: &Env, path: &str, who: Option<Uuid>) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut req = Request::builder().uri(format!("/api/v1{path}"));
    if let Some(u) = who {
        req = req.header(header::AUTHORIZATION, format!("Bearer {}", env.access_token(u)));
    }
    let res = env.app.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    (status, headers, res.into_body().collect().await.unwrap().to_bytes().to_vec())
}

#[tokio::test]
async fn blog_export_is_a_zip_of_open_formats_for_the_owner_only() {
    let env = setup(MODULES, &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    let blog = env.blog("diario", anna, "public").await;
    env.member(blog, bruno, "co_autore").await;
    let (s, c) = env.post("/blogs/diario/categories", Some(anna), json!({"name": "Viaggi", "slug": "viaggi"})).await;
    assert_eq!(s, StatusCode::CREATED, "{c}");
    let post =
        published(&env, anna, "diario", "notte", json!({"tags": ["luna"], "content": "Prima riga: con due punti"}))
            .await;
    published(&env, anna, "diario", "alba", json!({})).await;
    sqlx::query("INSERT INTO blog_pages (blog_id, slug, locale, title, content) VALUES ($1, 'chi-sono', 'it', 'Chi sono: io', 'sono io')")
        .bind(blog)
        .execute(&env.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO comments (post_id, author_display_name, content, status) VALUES ($1::uuid, 'ospite', 'bel pezzo', 'approved')")
        .bind(&post)
        .execute(&env.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO post_links (post_id, position, url, link_text) VALUES ($1::uuid, 0, 'https://example.com/a', 'a')",
    )
    .bind(&post)
    .execute(&env.pool)
    .await
    .unwrap();

    // solo il proprietario
    assert_eq!(download(&env, "/blogs/diario/export", None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(download(&env, "/blogs/diario/export", Some(bruno)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(download(&env, "/blogs/inesistente/export", Some(anna)).await.0, StatusCode::NOT_FOUND);

    let (s, headers, bytes) = download(&env, "/blogs/diario/export", Some(anna)).await;
    assert_eq!(s, StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    assert_eq!(headers[header::CONTENT_TYPE], "application/zip");
    assert!(headers[header::CONTENT_DISPOSITION].to_str().unwrap().contains("notturni-diario.zip"));
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut names: Vec<String> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().unwrap().to_string()).collect();
    names.sort();
    for expected in [
        "blog.json",
        "categories.json",
        "comments.json",
        "links.json",
        "media.json",
        "notes.json",
        "pages/it-chi-sono.md",
        "posts/it-alba.md",
        "posts/it-notte.md",
    ] {
        assert!(names.iter().any(|n| n == expected), "{expected} in {names:?}");
    }
    let mut read = |name: &str| {
        let mut out = String::new();
        zip.by_name(name).unwrap().read_to_string(&mut out).unwrap();
        out
    };
    let md = read("posts/it-notte.md");
    assert!(md.starts_with("---\ntitle: notte\nslug: notte\nlocale: it\nstatus: published\n"), "{md}");
    assert!(md.contains("tags: [\"luna\"]") && md.contains("Prima riga: con due punti"), "{md}");
    let blog_json: Value = serde_json::from_str(&read("blog.json")).unwrap();
    assert_eq!((blog_json["slug"].as_str(), blog_json["visibility"].as_str()), (Some("diario"), Some("public")));
    let comments: Value = serde_json::from_str(&read("comments.json")).unwrap();
    assert_eq!(comments[0]["content"], "bel pezzo");
    let links: Value = serde_json::from_str(&read("links.json")).unwrap();
    assert_eq!(links[0]["url"], "https://example.com/a");
    let cats: Value = serde_json::from_str(&read("categories.json")).unwrap();
    assert_eq!(cats[0]["name"], "Viaggi");
    let page = read("pages/it-chi-sono.md");
    assert!(page.contains("title: \"Chi sono: io\"") && page.contains("is_published: false"), "{page}");
    env.teardown().await;

    // senza i moduli opzionali restano i file del core
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    env.blog("solo-core", anna, "public").await;
    let (s, _, bytes) = download(&env, "/blogs/solo-core/export", Some(anna)).await;
    assert_eq!(s, StatusCode::OK);
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let names: Vec<String> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().unwrap().to_string()).collect();
    assert!(names.iter().any(|n| n == "blog.json") && !names.iter().any(|n| n == "comments.json"), "{names:?}");
    env.teardown().await;
}
