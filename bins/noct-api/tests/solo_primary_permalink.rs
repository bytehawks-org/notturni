//! Permalink dei post in modalità solo: il post del blog primario usa `/post/<slug>`, quello
//! degli altri spazi resta `/<blog-slug>/<post-slug>`. La modalità platform resta invariata.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;

const SOLO: &[(&str, &str)] = &[("NOCT_DEPLOYMENT_MODE", "solo")];

#[tokio::test]
async fn solo_post_of_the_primary_blog_has_post_permalink() {
    let env = setup(&[], SOLO).await;
    let (s, u) = env
        .post(
            "/auth/register",
            None,
            json!({"username": "anna", "email": "anna@example.com", "password": "una-password-lunga-1"}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{u}");
    let anna: uuid::Uuid = u["id"].as_str().unwrap().parse().unwrap();
    let (s, p) = env
        .post(
            "/blogs/anna/posts",
            Some(anna),
            json!({"slug": "ciao-mondo", "title": "Ciao", "content": "Corpo del post."}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{p}");
    assert_eq!(p["blog_slug"], "anna");
    assert_eq!(p["permalink"], "/post/ciao-mondo");
}

#[tokio::test]
async fn solo_post_of_an_extra_space_keeps_blog_slug_permalink() {
    let env = setup(&[], SOLO).await;
    let anna = env.user("anna", "utente").await;
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "Diario"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    let (s, p) = env
        .post("/blogs/diario/posts", Some(anna), json!({"slug": "ciao", "title": "Ciao", "content": "Corpo."}))
        .await;
    assert_eq!(s, StatusCode::CREATED, "{p}");
    assert_eq!(p["permalink"], "/diario/ciao");
}

#[tokio::test]
async fn solo_sitemaps_use_root_and_post_permalinks_for_the_primary_blog() {
    let env = setup(&["branding"], SOLO).await;
    let (s, u) = env
        .post(
            "/auth/register",
            None,
            json!({"username": "anna", "email": "anna@example.com", "password": "una-password-lunga-1"}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{u}");
    let anna: uuid::Uuid = u["id"].as_str().unwrap().parse().unwrap();
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "Diario"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    for (blog, slug) in [("anna", "ciao"), ("diario", "altro")] {
        let (s, p) = env
            .post(&format!("/blogs/{blog}/posts"), Some(anna), json!({"slug": slug, "title": "T", "content": "Corpo."}))
            .await;
        assert_eq!(s, StatusCode::CREATED, "{p}");
        let (s, p2) = env.post(&format!("/posts/{}/publish", p["id"].as_str().unwrap()), Some(anna), json!({})).await;
        assert_eq!(s, StatusCode::OK, "{p2}");
    }
    let (s, g) = env.get("/seo/sitemap", None).await;
    assert_eq!(s, StatusCode::OK, "{g}");
    let paths: Vec<&str> = g["entries"].as_array().unwrap().iter().map(|e| e["path"].as_str().unwrap()).collect();
    assert_eq!(paths.iter().filter(|p| **p == "/").count(), 1, "{paths:?}");
    assert!(paths.contains(&"/post/ciao"), "{paths:?}");
    assert!(paths.contains(&"/diario") && paths.contains(&"/diario/altro"), "{paths:?}");
    assert!(!paths.contains(&"/anna") && !paths.contains(&"/anna/ciao"), "{paths:?}");

    let (s, sp) = env.get("/blogs/anna/sitemap", None).await;
    assert_eq!(s, StatusCode::OK, "{sp}");
    let paths: Vec<&str> = sp["entries"].as_array().unwrap().iter().map(|e| e["path"].as_str().unwrap()).collect();
    assert_eq!(paths, vec!["/", "/post/ciao"]);
    let (_, sp) = env.get("/blogs/diario/sitemap", None).await;
    let paths: Vec<&str> = sp["entries"].as_array().unwrap().iter().map(|e| e["path"].as_str().unwrap()).collect();
    assert_eq!(paths, vec!["/diario", "/diario/altro"]);
}

async fn wait_mail(env: &Env, count: usize) -> Vec<noct_core::mail::Email> {
    for _ in 0..200 {
        if env.mailer.sent().len() >= count {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let sent = env.mailer.sent();
    assert_eq!(sent.len(), count, "{sent:#?}");
    sent
}

async fn register_primary(env: &Env) -> uuid::Uuid {
    let (s, u) = env
        .post(
            "/auth/register",
            None,
            json!({"username": "anna", "email": "anna@example.com", "password": "una-password-lunga-1"}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{u}");
    u["id"].as_str().unwrap().parse().unwrap()
}

fn id_of(v: &serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn solo_fragment_permalink_uses_post_prefix_for_the_primary_blog() {
    let env = setup(&["fragments"], SOLO).await;
    let anna = register_primary(&env).await;
    let lettore = env.user("lettore", "utente").await;
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "Diario"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    for (blog, slug) in [("anna", "ciao"), ("diario", "altro")] {
        let (s, p) = env
            .post(
                &format!("/blogs/{blog}/posts"),
                Some(anna),
                json!({"slug": slug, "title": "T", "content": "a".repeat(200)}),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{p}");
        let (s, p2) = env.post(&format!("/posts/{}/publish", id_of(&p)), Some(anna), json!({})).await;
        assert_eq!(s, StatusCode::OK, "{p2}");
        let (s, f) = env.post(&format!("/posts/{}/fragments", id_of(&p)), Some(lettore), json!({"text": "aaa"})).await;
        assert_eq!(s, StatusCode::CREATED, "{f}");
    }
    let (s, coll) = env.get("/users/me/fragments", Some(lettore)).await;
    assert_eq!(s, StatusCode::OK, "{coll}");
    let permalinks: Vec<&str> = coll.as_array().unwrap().iter().map(|f| f["permalink"].as_str().unwrap()).collect();
    assert!(permalinks.contains(&"/post/ciao"), "{permalinks:?}");
    assert!(permalinks.contains(&"/diario/altro"), "{permalinks:?}");
}

#[tokio::test]
async fn solo_bibliography_permalink_uses_post_prefix_for_the_primary_blog() {
    let env = setup(&["notes"], SOLO).await;
    let anna = register_primary(&env).await;
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "Diario"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    for (blog, slug) in [("anna", "ciao"), ("diario", "altro")] {
        let (s, p) = env
            .post(
                &format!("/blogs/{blog}/posts"),
                Some(anna),
                json!({"slug": slug, "title": "T", "content": "Corpo.", "notes": [{"idx": 1, "content": "Nota uno"}]}),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{p}");
    }
    let (s, bib) = env.get("/users/me/bibliography", Some(anna)).await;
    assert_eq!(s, StatusCode::OK, "{bib}");
    let permalinks: Vec<&str> = bib
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|e| e["citations"].as_array().unwrap().iter())
        .map(|c| c["permalink"].as_str().unwrap())
        .collect();
    assert!(permalinks.contains(&"/post/ciao"), "{permalinks:?}");
    assert!(permalinks.contains(&"/diario/altro"), "{permalinks:?}");
}

#[tokio::test]
async fn solo_publication_chapter_permalink_uses_post_prefix_for_the_primary_blog() {
    let env = setup(&["publications"], SOLO).await;
    let anna = register_primary(&env).await;
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "Diario"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    for (blog, slug, name) in [("anna", "ciao", "guida-anna"), ("diario", "altro", "guida-diario")] {
        let (s, pubr) =
            env.post(&format!("/blogs/{blog}/publications"), Some(anna), json!({"name": name, "title": "G"})).await;
        assert_eq!(s, StatusCode::CREATED, "{pubr}");
        let (s, p) = env
            .post(&format!("/blogs/{blog}/posts"), Some(anna), json!({"slug": slug, "title": "T", "content": "Corpo."}))
            .await;
        assert_eq!(s, StatusCode::CREATED, "{p}");
        let (s, p2) =
            env.patch(&format!("/posts/{}", id_of(&p)), Some(anna), json!({"publication_id": id_of(&pubr)})).await;
        assert_eq!(s, StatusCode::OK, "{p2}");
        let (s, detail) = env.get(&format!("/blogs/{blog}/publications/{name}"), Some(anna)).await;
        assert_eq!(s, StatusCode::OK, "{detail}");
        let expected = if blog == "anna" { "/post/ciao" } else { "/diario/altro" };
        assert_eq!(detail["chapters"][0]["permalink"], expected, "{detail}");
    }
}

#[tokio::test]
async fn solo_newsletter_post_notification_uses_post_prefix_for_the_primary_blog() {
    use std::time::Duration;
    let env = setup(&["newsletter"], SOLO).await;
    let anna = register_primary(&env).await;
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "Diario"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();

    // iscrizione e conferma: restituisce il link di conferma inviato per email
    for (ip, email, blog) in
        [("203.0.113.10", "uno@example.com", "anna"), ("203.0.113.11", "due@example.com", "diario")]
    {
        let before = env.mailer.sent().len();
        let req = axum::http::Request::builder()
            .method(axum::http::Method::POST)
            .uri("/api/v1/newsletter/subscribe")
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .header("x-forwarded-for", ip)
            .body(axum::body::Body::from(json!({"email": email, "blog_slug": blog}).to_string()))
            .unwrap();
        let (s, _, j) = env.send(req).await;
        assert_eq!(s, StatusCode::ACCEPTED, "{j}");
        let mail = wait_mail(&env, before + 1).await.pop().unwrap();
        let start = mail.text.find("https://").unwrap();
        let url = mail.text[start..].split_whitespace().next().unwrap().trim_end_matches('.');
        let token = url.split_once("token=").unwrap().1;
        let (s, b) = env.get(&format!("/newsletter/confirm?token={token}"), None).await;
        assert_eq!((s, b["status"].as_str()), (StatusCode::OK, Some("confirmed")), "{b}");
    }

    for (blog, slug, expected) in [("anna", "primo", "/post/primo"), ("diario", "altro", "/diario/altro")] {
        let (_, p) = env
            .post(
                &format!("/blogs/{blog}/posts"),
                Some(anna),
                json!({"slug": slug, "title": "Il post", "content": "Un testo abbastanza lungo da avere un estratto"}),
            )
            .await;
        let before = env.mailer.sent().len();
        assert_eq!(env.post(&format!("/posts/{}/publish", id_of(&p)), Some(anna), json!({})).await.0, StatusCode::OK);
        let mails = wait_mail(&env, before + 1).await;
        let m = mails.last().unwrap();
        let link = format!("https://notturni.eu{expected}");
        assert!(m.text.contains(&format!("Leggi tutto: {link}")), "{expected}: {}", m.text);
        assert!(m.html.as_ref().unwrap().contains(&format!("href=\"{link}\"")), "{expected}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    background.stop().await;
    env.teardown().await;
}

fn png_bytes(extra: usize) -> Vec<u8> {
    let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
    v.extend(std::iter::repeat_n(7u8, extra));
    v
}

async fn upload_png(env: &Env, blog: &str, who: uuid::Uuid, data: &[u8]) -> serde_json::Value {
    let boundary = "XBOUNDARYX";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"foto\"\r\nContent-Type: image/png\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let req = axum::http::Request::builder()
        .method(axum::http::Method::POST)
        .uri(format!("/api/v1/blogs/{blog}/media"))
        .header(axum::http::header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {}", env.access_token(who)))
        .body(axum::body::Body::from(body))
        .unwrap();
    let (s, _, j) = env.send(req).await;
    assert_eq!(s, StatusCode::CREATED, "{j}");
    j
}

#[tokio::test]
async fn solo_media_usage_uses_root_permalink_for_the_primary_blog() {
    let env = setup(&[], SOLO).await;
    let anna = register_primary(&env).await;
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "Diario"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    let up_primary = upload_png(&env, "anna", anna, &png_bytes(10)).await;
    let up_extra = upload_png(&env, "diario", anna, &png_bytes(20)).await;
    let (url_p, url_e) = (up_primary["url"].as_str().unwrap(), up_extra["url"].as_str().unwrap());
    let (s, p) = env
        .post(
            "/blogs/anna/posts",
            Some(anna),
            json!({"slug": "ciao", "title": "Ciao", "content": format!("![x]({url_p})")}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{p}");
    let (s, p) = env
        .post(
            "/blogs/diario/posts",
            Some(anna),
            json!({"slug": "altro", "title": "Altro", "content": format!("![x]({url_e})")}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{p}");

    let (s, lib) = env.get("/blogs/anna/media", Some(anna)).await;
    assert_eq!(s, StatusCode::OK, "{lib}");
    let item = lib["items"].as_array().unwrap().iter().find(|i| i["id"] == up_primary["media_id"]).unwrap();
    assert_eq!(item["used_in"][0]["permalink"], "/post/ciao", "{item}");
    let (s, lib) = env.get("/blogs/diario/media", Some(anna)).await;
    assert_eq!(s, StatusCode::OK, "{lib}");
    let item = lib["items"].as_array().unwrap().iter().find(|i| i["id"] == up_extra["media_id"]).unwrap();
    assert_eq!(item["used_in"][0]["permalink"], "/diario/altro", "{item}");
}

#[tokio::test]
async fn solo_user_comments_use_root_permalink_for_the_primary_blog() {
    let env = setup(&["comments"], SOLO).await;
    let anna = register_primary(&env).await;
    let bruno = env.user("bruno", "utente").await;
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "Diario"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    for (blog, slug, expected) in [("anna", "ciao", "/post/ciao"), ("diario", "altro", "/diario/altro")] {
        let (s, p) = env
            .post(&format!("/blogs/{blog}/posts"), Some(anna), json!({"slug": slug, "title": "T", "content": "Corpo."}))
            .await;
        assert_eq!(s, StatusCode::CREATED, "{p}");
        let (s, p2) = env.post(&format!("/posts/{}/publish", id_of(&p)), Some(anna), json!({})).await;
        assert_eq!(s, StatusCode::OK, "{p2}");
        let (s, c) =
            env.post(&format!("/posts/{}/comments", id_of(&p)), Some(bruno), json!({"content": "Bel post"})).await;
        assert_eq!(s, StatusCode::CREATED, "{c}");
        let (s, list) = env.get("/users/bruno/comments", None).await;
        assert_eq!(s, StatusCode::OK, "{list}");
        assert!(list.as_array().unwrap().iter().any(|c| c["permalink"] == expected), "{expected}: {list}");
    }
}

#[tokio::test]
async fn solo_blog_pages_use_root_pagina_path_for_the_primary_blog() {
    let env = setup(&["pages"], SOLO).await;
    let anna = register_primary(&env).await;
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "Diario"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    for blog in ["anna", "diario"] {
        let (s, b) = env.patch(&format!("/blogs/{blog}"), Some(anna), json!({"static_pages_enabled": true})).await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    let page =
        |slug: &str| json!({"slug": slug, "locale": "it", "title": "Chi", "content": "Testo.", "is_published": true});
    let (s, chi) = env.post("/blogs/anna/pages", Some(anna), page("chi")).await;
    assert_eq!(s, StatusCode::CREATED, "{chi}");
    assert_eq!(chi["permalink"], "/pagina/chi", "{chi}");
    let (s, chi) = env.post("/blogs/diario/pages", Some(anna), page("chi")).await;
    assert_eq!(s, StatusCode::CREATED, "{chi}");
    assert_eq!(chi["permalink"], "/diario/pagina/chi", "{chi}");
    let (_, list) = env.get("/blogs/anna/pages?locale=it", Some(anna)).await;
    assert_eq!(list[0]["permalink"], "/pagina/chi", "{list}");
}

#[tokio::test]
async fn solo_space_sitemap_lists_primary_pages_and_publications_at_the_root() {
    let env = setup(&["pages", "publications"], SOLO).await;
    let anna = register_primary(&env).await;
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "Diario"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    for (blog, slug, name) in [("anna", "ciao", "guida-anna"), ("diario", "altro", "guida-diario")] {
        let (s, b) = env.patch(&format!("/blogs/{blog}"), Some(anna), json!({"static_pages_enabled": true})).await;
        assert_eq!(s, StatusCode::OK, "{b}");
        let page = json!({"slug": "chi", "locale": "it", "title": "Chi", "content": "Testo.", "is_published": true});
        let (s, p) = env.post(&format!("/blogs/{blog}/pages"), Some(anna), page).await;
        assert_eq!(s, StatusCode::CREATED, "{p}");
        let (s, pubr) =
            env.post(&format!("/blogs/{blog}/publications"), Some(anna), json!({"name": name, "title": "G"})).await;
        assert_eq!(s, StatusCode::CREATED, "{pubr}");
        let (s, p) = env
            .post(&format!("/blogs/{blog}/posts"), Some(anna), json!({"slug": slug, "title": "T", "content": "Corpo."}))
            .await;
        assert_eq!(s, StatusCode::CREATED, "{p}");
        let (s, p2) =
            env.patch(&format!("/posts/{}", id_of(&p)), Some(anna), json!({"publication_id": id_of(&pubr)})).await;
        assert_eq!(s, StatusCode::OK, "{p2}");
        let (s, p3) = env.post(&format!("/posts/{}/publish", id_of(&p)), Some(anna), json!({})).await;
        assert_eq!(s, StatusCode::OK, "{p3}");
    }
    for (blog, expected) in [
        ("anna", ["/", "/post/ciao", "/pagina/chi", "/pub/guida-anna"]),
        ("diario", ["/diario", "/diario/altro", "/diario/pagina/chi", "/diario/pub/guida-diario"]),
    ] {
        let (s, sp) = env.get(&format!("/blogs/{blog}/sitemap"), None).await;
        assert_eq!(s, StatusCode::OK, "{sp}");
        let paths: Vec<&str> = sp["entries"].as_array().unwrap().iter().map(|e| e["path"].as_str().unwrap()).collect();
        assert_eq!(paths, expected);
    }
    env.teardown().await;
}

/// robots.txt matching (RFC 9309): a rule is a path prefix, a final `$` anchors the end of the URL.
fn blocked(rules: &[&str], url: &str) -> bool {
    rules.iter().any(|r| r.strip_suffix('$').map_or_else(|| url.starts_with(r), |exact| url == exact))
}

/// URLs that serve content of the primary blog at the root.
const PRIMARY_URLS: &[&str] = &[
    "/",
    "/?category=x",
    "/post/ciao",
    "/search",
    "/search?q=x",
    "/media",
    "/media?page=2",
    "/link",
    "/link?page=2",
    "/bibliografia",
    "/bibliografia?page=2",
    "/pub",
    "/pub?page=2",
    "/pub/guida",
    "/pagina/chi",
    "/feed.xml",
    "/atom.xml",
];

/// URLs that a primary blog opt-out must leave open. Some share a text prefix with a primary route.
const OTHER_URLS: &[&str] = &[
    "/diario",
    "/diario/altro",
    "/diario/pagina/chi",
    "/postille",
    "/searching",
    "/media-lab",
    "/linkati",
    "/bibliografia-viva",
    "/pubblico",
    "/pagina-due",
    "/p/privacy",
    "/tag/rust",
    "/u/anna",
    "/blogs",
    "/sitemap.xml",
    "/sitemaps/0.xml",
    "/robots.txt",
];

async fn directives(env: &Env) -> (Vec<String>, Vec<String>) {
    let (s, d) = env.get("/seo/crawl-directives", None).await;
    assert_eq!(s, StatusCode::OK, "{d}");
    let list = |k: &str| d[k].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    (list("search_disallow"), list("ai_disallow"))
}

fn assert_only_primary_blocked(list: &[String]) {
    let rules: Vec<&str> = list.iter().map(String::as_str).collect();
    assert!(!rules.contains(&"/"), "{rules:?}");
    for url in PRIMARY_URLS {
        assert!(blocked(&rules, url), "{url} must be blocked: {rules:?}");
    }
    for url in OTHER_URLS {
        assert!(!blocked(&rules, url), "{url} must stay open: {rules:?}");
    }
}

async fn primary_and_public_space(env: &Env) -> uuid::Uuid {
    let anna = register_primary(env).await;
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "Diario"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    anna
}

#[tokio::test]
async fn solo_primary_search_opt_out_blocks_only_its_root_urls() {
    let env = setup(&[], SOLO).await;
    let anna = primary_and_public_space(&env).await;
    assert_eq!(directives(&env).await, (vec![], vec![]));
    let (s, b) = env.patch("/blogs/anna", Some(anna), json!({"search_indexing_enabled": false})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (search, ai) = directives(&env).await;
    assert_only_primary_blocked(&search);
    assert_only_primary_blocked(&ai);
    env.teardown().await;
}

#[tokio::test]
async fn solo_primary_ai_opt_out_leaves_search_engines_open() {
    let env = setup(&[], SOLO).await;
    let anna = primary_and_public_space(&env).await;
    let (s, b) = env.patch("/blogs/anna", Some(anna), json!({"ai_crawling_enabled": false})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (search, ai) = directives(&env).await;
    assert!(search.is_empty(), "{search:?}");
    assert_only_primary_blocked(&ai);
    env.teardown().await;
}

#[tokio::test]
async fn solo_extra_space_opt_out_keeps_the_blog_slug_rule() {
    let env = setup(&[], SOLO).await;
    let anna = primary_and_public_space(&env).await;
    let (s, b) = env.patch("/blogs/diario", Some(anna), json!({"search_indexing_enabled": false})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(directives(&env).await, (vec!["/diario".to_string()], vec!["/diario".to_string()]));
    env.teardown().await;
}

#[tokio::test]
async fn platform_opt_out_keeps_the_blog_slug_rule_with_a_leftover_primary_marker() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "diario", "title": "Diario"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    sqlx::query(
        "UPDATE platform_config SET primary_blog_id = (SELECT id FROM blogs WHERE slug = 'diario') WHERE id = 1",
    )
    .execute(&env.pool)
    .await
    .unwrap();
    let (s, b) = env.patch("/blogs/diario", Some(anna), json!({"search_indexing_enabled": false})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(directives(&env).await, (vec!["/diario".to_string()], vec!["/diario".to_string()]));
    let (s, b) = env.patch("/blogs/diario", Some(anna), json!({"search_indexing_enabled": true})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, sp) = env.get("/blogs/diario/sitemap", None).await;
    assert_eq!(s, StatusCode::OK, "{sp}");
    assert_eq!(sp["entries"][0]["path"], "/diario");
    env.teardown().await;
}
