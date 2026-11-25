//! Blog primario della modalità solo: creato in modo idempotente alla prima registrazione e
//! all'avvio, marcato in `platform_config.primary_blog_id` ed esposto da `GET /config`.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

const PASSWORD: &str = "una-password-lunga-1";
const SOLO: &[(&str, &str)] = &[("NOCT_DEPLOYMENT_MODE", "solo")];

async fn register(env: &Env, name: &str) -> (StatusCode, serde_json::Value) {
    env.post(
        "/auth/register",
        None,
        json!({"username": name, "email": format!("{name}@example.com"), "password": PASSWORD}),
    )
    .await
}

async fn blogs(pool: &PgPool) -> Vec<(Uuid, String, Uuid, String, String)> {
    sqlx::query_as("SELECT id, slug, owner_id, kind::text, display_mode::text FROM blogs ORDER BY created_at")
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn marker(pool: &PgPool) -> Option<Uuid> {
    sqlx::query_scalar("SELECT primary_blog_id FROM platform_config WHERE id = 1").fetch_one(pool).await.unwrap()
}

async fn audit_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'blog.primary_created'")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn solo_first_registration_creates_the_primary_blog() {
    let env = setup(&[], SOLO).await;
    let (s, u) = register(&env, "anna").await;
    assert_eq!(s, StatusCode::CREATED, "{u}");
    let uid: Uuid = u["id"].as_str().unwrap().parse().unwrap();
    let list = blogs(&env.pool).await;
    assert_eq!(list.len(), 1);
    let (id, slug, owner, kind, mode) = &list[0];
    assert_eq!((slug.as_str(), *owner, kind.as_str(), mode.as_str()), ("anna", uid, "blog", "feed"));
    assert_eq!(marker(&env.pool).await, Some(*id));
    let (s, cfg) = env.get("/config", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(cfg["primary_blog_slug"], "anna");
    assert_eq!(audit_count(&env.pool).await, 1);
    // seconda registrazione: 409 e nessun secondo blog
    let (s, _) = register(&env, "bruno").await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(blogs(&env.pool).await.len(), 1);
    env.teardown().await;
}

#[tokio::test]
async fn ensure_is_idempotent() {
    let env = setup(&[], SOLO).await;
    register(&env, "anna").await;
    for _ in 0..2 {
        env.registry.run_startup(&env.enabled, &env.state).await.unwrap();
    }
    assert_eq!(blogs(&env.pool).await.len(), 1);
    assert_eq!(audit_count(&env.pool).await, 1);
    env.teardown().await;
}

#[tokio::test]
async fn startup_creates_it_for_super_admin_from_env() {
    let env = setup(
        &[],
        &[
            ("NOCT_DEPLOYMENT_MODE", "solo"),
            ("NOCT_SUPER_ADMIN_USERNAME", "owner"),
            ("NOCT_SUPER_ADMIN_EMAIL", "owner@example.com"),
            ("NOCT_SUPER_ADMIN_PASSWORD", "una-password-lunga-9"),
            ("NOCT_SITE_NAME", "Il Mio Diario"),
        ],
    )
    .await;
    let list = blogs(&env.pool).await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].1, "il-mio-diario");
    let title: String = sqlx::query_scalar("SELECT title FROM blogs").fetch_one(&env.pool).await.unwrap();
    assert_eq!(title, "Il Mio Diario");
    env.registry.run_startup(&env.enabled, &env.state).await.unwrap();
    assert_eq!(blogs(&env.pool).await.len(), 1);
    env.teardown().await;
}

#[tokio::test]
async fn startup_upgrades_an_existing_install_and_avoids_slug_collisions() {
    let env = setup(&[], SOLO).await;
    // utente senza blog (installazione precedente); lo slug "anna" è occupato da un blog altrui
    let anna = env.user("anna", "super_admin").await;
    let other = env.user("altro", "utente").await;
    env.blog("anna", other, "public").await;
    env.registry.run_startup(&env.enabled, &env.state).await.unwrap();
    let list = blogs(&env.pool).await;
    assert_eq!(list.len(), 2);
    let primary = list.iter().find(|b| b.2 == anna).expect("blog di anna");
    assert_eq!(primary.1, "anna-2");
    assert_eq!(marker(&env.pool).await, Some(primary.0));
    env.teardown().await;
}

#[tokio::test]
async fn slug_that_collides_with_a_site_route_gets_a_suffix() {
    let env = setup(&[], SOLO).await;
    let (s, _) = register(&env, "login").await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(blogs(&env.pool).await[0].1, "login-2");
    env.teardown().await;
}

#[tokio::test]
async fn purged_primary_blog_is_recreated_at_next_startup() {
    let env = setup(&[], SOLO).await;
    register(&env, "anna").await;
    sqlx::query("DELETE FROM blogs").execute(&env.pool).await.unwrap();
    let (_, cfg) = env.get("/config", None).await;
    assert!(cfg["primary_blog_slug"].is_null(), "{cfg}");
    env.registry.run_startup(&env.enabled, &env.state).await.unwrap();
    assert_eq!(blogs(&env.pool).await.len(), 1);
    assert_eq!(audit_count(&env.pool).await, 2);
    let (_, cfg) = env.get("/config", None).await;
    assert_eq!(cfg["primary_blog_slug"], "anna");
    env.teardown().await;
}

#[tokio::test]
async fn platform_creates_no_blog_and_config_slug_is_null() {
    let env = setup(&[], &[]).await;
    let (s, _) = register(&env, "anna").await;
    assert_eq!(s, StatusCode::CREATED);
    env.registry.run_startup(&env.enabled, &env.state).await.unwrap();
    assert!(blogs(&env.pool).await.is_empty());
    assert_eq!(marker(&env.pool).await, None);
    let (_, cfg) = env.get("/config", None).await;
    assert!(cfg.get("primary_blog_slug").is_some_and(|v| v.is_null()), "{cfg}");
    env.teardown().await;
}

/// Solo: il blog primario non conta verso `max_blogs_per_user`. Il proprietario ne crea altri
/// cinque (il limite predefinito) e il sesto viene rifiutato.
#[tokio::test]
async fn solo_primary_blog_does_not_count_toward_the_space_limit() {
    let env = setup(&[], SOLO).await;
    let (s, u) = register(&env, "anna").await;
    assert_eq!(s, StatusCode::CREATED, "{u}");
    let uid: Uuid = u["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(blogs(&env.pool).await.len(), 1);
    for i in 0..5 {
        let (s, b) = env.post("/blogs", Some(uid), json!({"slug": format!("extra-{i}"), "title": "T"})).await;
        assert_eq!(s, StatusCode::CREATED, "spazio {i}: {b}");
    }
    let (s, b) = env.post("/blogs", Some(uid), json!({"slug": "extra-sei", "title": "T"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert!(detail(&b).contains("Limite massimo di 5"), "{b}");
    assert_eq!(blogs(&env.pool).await.len(), 6);
    env.teardown().await;
}

/// Platform: il conteggio resta quello di sempre, cioè tutti i blog dell'utente.
#[tokio::test]
async fn platform_mode_counts_every_blog_toward_the_space_limit() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    for i in 0..5 {
        let (s, b) = env.post("/blogs", Some(anna), json!({"slug": format!("blog-{i}"), "title": "T"})).await;
        assert_eq!(s, StatusCode::CREATED, "spazio {i}: {b}");
    }
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "blog-sei", "title": "T"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert!(detail(&b).contains("Limite massimo di 5"), "{b}");
    env.teardown().await;
}

/// Un marker lasciato da un passaggio solo -> platform non esclude nessun blog in platform mode.
#[tokio::test]
async fn platform_mode_ignores_a_leftover_primary_marker() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "blog-0", "title": "T"})).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    let first: Uuid =
        sqlx::query_scalar("SELECT id FROM blogs WHERE slug = 'blog-0'").fetch_one(&env.pool).await.unwrap();
    sqlx::query("UPDATE platform_config SET primary_blog_id = $1 WHERE id = 1")
        .bind(first)
        .execute(&env.pool)
        .await
        .unwrap();
    for i in 1..5 {
        let (s, b) = env.post("/blogs", Some(anna), json!({"slug": format!("blog-{i}"), "title": "T"})).await;
        assert_eq!(s, StatusCode::CREATED, "spazio {i}: {b}");
    }
    let (s, b) = env.post("/blogs", Some(anna), json!({"slug": "blog-sei", "title": "T"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    env.teardown().await;
}

async fn make_coauthor(env: &Env, blog: &str, user: Uuid) {
    sqlx::query(
        "INSERT INTO blog_memberships (blog_id, user_id, role)
         SELECT id, $2, 'co_autore' FROM blogs WHERE slug = $1",
    )
    .bind(blog)
    .bind(user)
    .execute(&env.pool)
    .await
    .unwrap();
}

async fn owner_of(pool: &PgPool, slug: &str) -> Uuid {
    sqlx::query_scalar("SELECT owner_id FROM blogs WHERE slug = $1").bind(slug).fetch_one(pool).await.unwrap()
}

/// Solo: il blog primario non può essere trasferito, anche a un coautore. Gli altri blog sì.
#[tokio::test]
async fn solo_primary_blog_cannot_be_transferred() {
    let env = setup(&[], SOLO).await;
    let (s, u) = register(&env, "anna").await;
    assert_eq!(s, StatusCode::CREATED, "{u}");
    let anna: Uuid = u["id"].as_str().unwrap().parse().unwrap();
    let bruno = env.user("bruno", "utente").await;
    make_coauthor(&env, "anna", bruno).await;
    let (s, b) = env.post("/blogs/anna/transfer", Some(anna), json!({"username": "bruno"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert!(detail(&b).contains("Il blog primario non può essere trasferito."), "{b}");
    assert_eq!(owner_of(&env.pool, "anna").await, anna);
    env.teardown().await;
}

#[tokio::test]
async fn solo_other_blog_can_still_be_transferred() {
    let env = setup(&[], SOLO).await;
    let (s, u) = register(&env, "anna").await;
    assert_eq!(s, StatusCode::CREATED, "{u}");
    let anna: Uuid = u["id"].as_str().unwrap().parse().unwrap();
    let bruno = env.user("bruno", "utente").await;
    env.blog("extra", anna, "public").await;
    make_coauthor(&env, "extra", bruno).await;
    let (s, b) = env.post("/blogs/extra/transfer", Some(anna), json!({"username": "bruno"})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(owner_of(&env.pool, "extra").await, bruno);
    assert_eq!(owner_of(&env.pool, "anna").await, anna);
    env.teardown().await;
}

/// Platform: nessun blog primario, il trasferimento resta permesso.
#[tokio::test]
async fn platform_blog_transfer_is_unaffected() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    env.blog("diario", anna, "public").await;
    make_coauthor(&env, "diario", bruno).await;
    let (s, b) = env.post("/blogs/diario/transfer", Some(anna), json!({"username": "bruno"})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(owner_of(&env.pool, "diario").await, bruno);
    env.teardown().await;
}
