//! Scoperta: feed, tendenze, ricerca, directory dei blog, follow, panoramica, letture, menzioni,
//! viste «tutti i miei blog» e interessi.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use uuid::Uuid;

const MODULES: &[&str] = &["discovery", "interests", "taxonomy", "publications", "comments", "notes"];

async fn world() -> (Env, Uuid) {
    let env = setup(MODULES, &[]).await;
    let anna = env.user("anna", "utente").await;
    (env, anna)
}

async fn post_in(env: &Env, who: Uuid, blog: &str, slug: &str, extra: Value) -> Value {
    let mut body = json!({"slug": slug, "title": format!("Titolo {slug}"), "content": "uno due tre"});
    body.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    let (s, b) = env.post(&format!("/blogs/{blog}/posts"), Some(who), body).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    b
}

async fn published(env: &Env, who: Uuid, blog: &str, slug: &str, extra: Value) -> Value {
    let p = post_in(env, who, blog, slug, extra).await;
    let (s, b) = env.post(&format!("/posts/{}/publish", id(&p)), Some(who), json!({})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    p
}

fn id(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

fn slugs(list: &Value) -> Vec<String> {
    list.as_array().unwrap().iter().map(|p| p["slug"].as_str().unwrap().to_string()).collect()
}

// ---- feed, tendenze, ricerca -----------------------------------------------------------------------------------------

#[tokio::test]
async fn feed_shows_only_published_posts_of_listable_blogs_with_filters() {
    let (env, anna) = world().await;
    let bruno = env.user("bruno", "utente").await;
    env.blog("diario", anna, "public").await;
    env.blog("segreto", bruno, "private").await;
    env.blog("sospeso", bruno, "public").await;

    let cat = env.post("/blogs/diario/categories", Some(anna), json!({"name": "Viaggi", "slug": "viaggi"})).await.1;
    published(&env, anna, "diario", "a", json!({"content": "Rust #poesia", "category_id": id(&cat)})).await;
    published(&env, anna, "diario", "b", json!({"locale": "en", "content": "english text"})).await;
    post_in(&env, anna, "diario", "bozza", json!({})).await;
    published(&env, bruno, "segreto", "privato", json!({})).await;
    published(&env, bruno, "sospeso", "fermo", json!({})).await;
    sqlx::query("UPDATE blogs SET is_suspended = true WHERE slug = 'sospeso'").execute(&env.pool).await.unwrap();

    let (s, feed) = env.get("/feed/posts", None).await;
    assert_eq!((s, slugs(&feed)), (StatusCode::OK, vec!["b".to_string(), "a".to_string()]), "più recente prima");
    assert_eq!(feed[0]["blog_slug"], "diario");
    assert_eq!(slugs(&env.get("/feed/posts?locale=en", None).await.1), ["b"]);
    assert_eq!(slugs(&env.get("/feed/posts?tag=poesia", None).await.1), ["a"]);
    assert_eq!(slugs(&env.get("/feed/posts?category=viaggi", None).await.1), ["a"]);
    assert_eq!(env.get("/feed/posts?category=inesistente", None).await.1, json!([]));
    assert_eq!(slugs(&env.get("/feed/posts?limit=1", None).await.1), ["b"]);
    assert_eq!(slugs(&env.get("/feed/posts?limit=1&offset=1", None).await.1), ["a"]);
    assert_eq!(env.get("/feed/posts?limit=0", None).await.1.as_array().unwrap().len(), 1, "limite portato a 1");
    assert_eq!(env.get("/feed/posts?limit=abc", None).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    // lingue e tendenze
    let (_, locales) = env.get("/feed/locales", None).await;
    assert_eq!(locales, json!([{"locale": "en", "count": 1}, {"locale": "it", "count": 1}]));
    let (_, trending) = env.get("/feed/trending", None).await;
    assert_eq!(trending, json!([{"tag": "poesia", "post_count": 1}]));
    sqlx::query("UPDATE posts SET published_at = now() - interval '10 days'").execute(&env.pool).await.unwrap();
    assert_eq!(env.get("/feed/trending", None).await.1, json!([]));
    assert_eq!(env.get("/feed/trending?days=30", None).await.1.as_array().unwrap().len(), 1);
    env.teardown().await;
}

#[tokio::test]
async fn following_feed_needs_login_and_does_not_unmask_aliases() {
    let (env, anna) = world().await;
    let bruno = env.user("bruno", "utente").await;
    let carla = env.user("carla", "utente").await;
    env.blog("diario", anna, "public").await;
    env.blog("altro", bruno, "public").await;
    env.blog("alias", carla, "public").await;
    published(&env, anna, "diario", "annuncio", json!({})).await;
    published(&env, bruno, "altro", "bruno-1", json!({})).await;
    published(&env, carla, "alias", "carla-1", json!({})).await;
    // un post di carla firmato con un alias del blog
    sqlx::query("UPDATE posts SET author_display_name = 'Nome Segreto' WHERE slug = 'carla-1'")
        .execute(&env.pool)
        .await
        .unwrap();

    assert_eq!(env.get("/feed/posts?following=true", None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.get("/feed/posts?following=true", Some(anna)).await.1, json!([]), "non segue nessuno");
    assert_eq!(env.post("/blogs/altro/follow", Some(anna), json!({})).await.0, StatusCode::NO_CONTENT);
    assert_eq!(env.post("/users/carla/follow", Some(anna), json!({})).await.0, StatusCode::NO_CONTENT);
    let (_, feed) = env.get("/feed/posts?following=true", Some(anna)).await;
    assert_eq!(slugs(&feed), ["bruno-1"], "il post di carla è firmato con un alias: non rivela di chi è");
    sqlx::query("UPDATE posts SET author_display_name = 'carla' WHERE slug = 'carla-1'")
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(slugs(&env.get("/feed/posts?following=true", Some(anna)).await.1).len(), 2);
    env.teardown().await;
}

#[tokio::test]
async fn global_search_treats_wildcards_literally_and_hides_unlisted_blogs() {
    let (env, anna) = world().await;
    env.blog("diario", anna, "public").await;
    env.blog("segreto", anna, "private").await;
    published(&env, anna, "diario", "uno", json!({"title": "Il Mare", "content": "onde e sale"})).await;
    published(&env, anna, "diario", "due", json!({"title": "Altro", "content": "sconto del 50% oggi"})).await;
    published(&env, anna, "segreto", "tre", json!({"title": "Il mare segreto"})).await;

    assert_eq!(slugs(&env.get("/search/posts?q=mare", None).await.1), ["uno"], "case-insensitive e solo blog pubblici");
    assert_eq!(slugs(&env.get("/search/posts?q=SALE", None).await.1), ["uno"], "anche nel contenuto");
    assert_eq!(slugs(&env.get("/search/posts?q=50%25", None).await.1), ["due"]);
    assert_eq!(slugs(&env.get("/search/posts?q=%25", None).await.1), ["due"], "il % è un carattere come gli altri");
    assert_eq!(env.get("/search/posts?q=_", None).await.1, json!([]), "_ non è un jolly");
    assert_eq!(env.get("/search/posts?q=%20%20", None).await.1, json!([]));
    assert_eq!(env.get("/search/posts", None).await.0, StatusCode::UNPROCESSABLE_ENTITY, "q obbligatoria");
    env.teardown().await;
}

async fn order(env: &Env, q: &str) -> Vec<String> {
    slugs(&env.get(&format!("/blogs{q}"), None).await.1)
}

// ---- directory e follow ----------------------------------------------------------------------------------------------

#[tokio::test]
async fn blog_directory_lists_indexable_public_blogs_with_counts() {
    let (env, anna) = world().await;
    let bruno = env.user("bruno", "utente").await;
    let carla = env.user("carla", "utente").await;
    env.blog("mare", anna, "public").await;
    env.blog("monti", bruno, "public").await;
    env.blog("nascosto", carla, "public").await;
    env.blog("privato", carla, "private").await;
    sqlx::query("UPDATE blogs SET search_indexing_enabled = false WHERE slug = 'nascosto'")
        .execute(&env.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE blogs SET default_locale = 'en', subtitle = 'Vette' WHERE slug = 'monti'")
        .execute(&env.pool)
        .await
        .unwrap();
    published(&env, anna, "mare", "p1", json!({})).await;
    published(&env, anna, "mare", "p2", json!({})).await;
    post_in(&env, anna, "mare", "bozza", json!({})).await;
    env.post("/blogs/monti/follow", Some(anna), json!({})).await;
    env.post("/blogs/monti/follow", Some(carla), json!({})).await;

    let (s, dir) = env.get("/blogs", None).await;
    assert_eq!(s, StatusCode::OK);
    let list = dir.as_array().unwrap();
    let names: Vec<_> = list.iter().map(|b| b["slug"].as_str().unwrap()).collect();
    assert_eq!(names, ["mare", "monti"], "per attività: prima chi ha pubblicato");
    assert_eq!((list[0]["post_count"].clone(), list[0]["follower_count"].clone()), (json!(2), json!(0)), "{dir}");
    assert_eq!((list[1]["post_count"].clone(), list[1]["follower_count"].clone()), (json!(0), json!(2)));
    assert!(list[0]["last_published_at"].is_string() && list[1]["last_published_at"].is_null());
    assert!(list[0]["owner_id"].is_null(), "il proprietario non si espone");
    assert_eq!(order(&env, "?sort=followers").await, ["monti", "mare"]);
    assert_eq!(order(&env, "?sort=new").await, ["monti", "mare"]);
    assert_eq!(order(&env, "?q=VETTE").await, ["monti"], "cerca anche nel sottotitolo");
    assert_eq!(order(&env, "?locale=en").await, ["monti"]);
    assert_eq!(order(&env, "?limit=1&offset=1").await, ["monti"]);
    // l'elenco condiviso `POST /blogs` continua a funzionare sullo stesso percorso
    assert_eq!(env.post("/blogs", Some(anna), json!({"slug": "nuovo", "title": "N"})).await.0, StatusCode::CREATED);
    env.teardown().await;
}

#[tokio::test]
async fn follows_are_idempotent_and_stats_add_up() {
    let (env, anna) = world().await;
    let bruno = env.user("bruno", "utente").await;
    let carla = env.user("carla", "utente").await;
    env.blog("diario", anna, "public").await;
    env.blog("alias", anna, "public").await;
    sqlx::query("UPDATE blogs SET default_author_display_name = 'Penna' WHERE slug = 'alias'")
        .execute(&env.pool)
        .await
        .unwrap();

    assert_eq!(env.post("/blogs/diario/follow", None, json!({})).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.post("/blogs/nope/follow", Some(bruno), json!({})).await.0, StatusCode::NOT_FOUND);
    for _ in 0..2 {
        assert_eq!(env.post("/blogs/diario/follow", Some(bruno), json!({})).await.0, StatusCode::NO_CONTENT);
    }
    assert_eq!(env.post("/blogs/alias/follow", Some(carla), json!({})).await.0, StatusCode::NO_CONTENT);
    assert_eq!(env.post("/users/anna/follow", Some(bruno), json!({})).await.0, StatusCode::NO_CONTENT);
    assert_eq!(env.post("/users/anna/follow", Some(bruno), json!({})).await.0, StatusCode::NO_CONTENT);
    assert_eq!(env.post("/users/ANNA/follow", Some(anna), json!({})).await.0, StatusCode::BAD_REQUEST, "non se stessi");
    assert_eq!(env.post("/users/nessuno/follow", Some(anna), json!({})).await.0, StatusCode::NOT_FOUND);

    let (_, followers) = env.get("/blogs/diario/followers", None).await;
    assert_eq!(followers, json!([{"username": "bruno"}]));
    assert_eq!(env.get("/users/anna/followers", None).await.1, json!([{"username": "bruno"}]));
    assert_eq!(env.get("/users/bruno/following", None).await.1, json!([{"username": "anna"}]));
    assert_eq!(env.get("/users/me/follow-stats", None).await.0, StatusCode::UNAUTHORIZED);
    let (_, stats) = env.get("/users/me/follow-stats", Some(anna)).await;
    assert_eq!(stats["user_followers"], 1);
    assert_eq!(stats["total_followers"], 3, "{stats}");
    let alias = stats["blogs"].as_array().unwrap().iter().find(|b| b["blog_slug"] == "alias").unwrap();
    assert_eq!((alias["alias"].as_str(), alias["followers"].clone()), (Some("Penna"), json!(1)));

    // un blog che diventa privato non è più visibile, ma chi lo seguiva può comunque smettere
    sqlx::query("UPDATE blogs SET visibility = 'private' WHERE slug = 'diario'").execute(&env.pool).await.unwrap();
    assert_eq!(env.post("/blogs/diario/follow", Some(carla), json!({})).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.delete("/blogs/diario/follow", Some(bruno)).await, StatusCode::NO_CONTENT);
    assert_eq!(env.delete("/users/anna/follow", Some(bruno)).await, StatusCode::NO_CONTENT);
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM blog_follows WHERE blog_id = (SELECT id FROM blogs WHERE slug = 'diario')",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert_eq!(n, 0);
    env.teardown().await;
}

// ---- panoramica, letture, menzioni -----------------------------------------------------------------------------------

#[tokio::test]
async fn reads_are_counted_once_per_visitor_and_feed_the_overview() {
    let (env, anna) = world().await;
    let bruno = env.user("bruno", "utente").await;
    let estraneo = env.user("estraneo", "utente").await;
    let blog = env.blog("diario", anna, "public").await;
    env.member(blog, bruno, "revisore").await;
    let p =
        published(&env, anna, "diario", "uno", json!({"content": "con un'immagine ![x](https://x.org/a.png)"})).await;
    post_in(&env, anna, "diario", "bozza", json!({})).await;
    let future = post_in(&env, anna, "diario", "futuro", json!({})).await;
    env.post(&format!("/posts/{}/publish", id(&future)), Some(anna), json!({"published_at": "2999-01-01T00:00:00Z"}))
        .await;
    let read = format!("/posts/{}/read", id(&p));

    assert_eq!(env.post(&read, None, json!({})).await.0, StatusCode::NO_CONTENT);
    assert_eq!(env.post(&read, None, json!({})).await.0, StatusCode::NO_CONTENT, "accettata ma non contata");
    assert_eq!(env.post(&format!("/posts/{}/read", Uuid::new_v4()), None, json!({})).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        env.post(&format!("/posts/{}/read", id(&future)), None, json!({})).await.0,
        StatusCode::NOT_FOUND,
        "non pubblicato"
    );
    let reads: i64 = sqlx::query_scalar("SELECT coalesce(sum(reads), 0)::bigint FROM post_reads_daily")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(reads, 1);

    // commenti e frammenti per i conteggi
    let bruno_comment = env.post(&format!("/posts/{}/comments", id(&p)), Some(bruno), json!({"content": "ciao"})).await;
    assert_eq!(bruno_comment.0, StatusCode::CREATED);
    sqlx::query("INSERT INTO comments (post_id, author_display_name, content, status) SELECT id, 'x', 'y', 'pending' FROM posts WHERE slug = 'uno'")
        .execute(&env.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO media_files (blog_id, url, content_type, size_bytes) VALUES ($1, 'https://x.org/a.png', 'image/png', 2048)")
        .bind(blog)
        .execute(&env.pool)
        .await
        .unwrap();
    env.post("/blogs/diario/follow", Some(estraneo), json!({})).await;

    let path = "/blogs/diario/overview";
    assert_eq!(env.get(path, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.get(path, Some(estraneo)).await.0, StatusCode::FORBIDDEN);
    let (s, o) = env.get(path, Some(bruno)).await;
    assert_eq!(s, StatusCode::OK, "ogni membro la vede: {o}");
    assert_eq!(
        (
            o["posts_total"].clone(),
            o["posts_published"].clone(),
            o["posts_scheduled"].clone(),
            o["posts_draft"].clone()
        ),
        (json!(3), json!(1), json!(1), json!(1))
    );
    assert_eq!((o["followers"].clone(), o["members"].clone(), o["media"].clone()), (json!(1), json!(1), json!(1)));
    assert_eq!((o["pending_comments"].clone(), o["approved_comments"].clone()), (json!(1), json!(1)));
    assert_eq!((o["storage_bytes"].clone(), o["storage_limit_mb"].clone()), (json!(2048), Value::Null));
    assert_eq!(o["reads_total_30d"], 1);
    let days = o["reads_30d"].as_array().unwrap();
    assert_eq!(days.len(), 30);
    assert_eq!(days[29]["reads"], 1, "oggi è l'ultimo giorno");
    assert!(days[0]["day"].as_str().unwrap().len() == 10);
    assert!(o["last_published_at"].is_string());

    // blog privato: nessuna lettura contata
    sqlx::query("UPDATE blogs SET visibility = 'private'").execute(&env.pool).await.unwrap();
    assert_eq!(env.post(&read, None, json!({})).await.0, StatusCode::NOT_FOUND);
    env.teardown().await;
}

#[tokio::test]
async fn overview_works_without_the_comments_module() {
    let env = setup(&["discovery"], &[]).await;
    let anna = env.user("anna", "utente").await;
    env.blog("diario", anna, "public").await;
    let (s, o) = env.get("/blogs/diario/overview", Some(anna)).await;
    assert_eq!(
        (s, o["pending_comments"].clone(), o["approved_comments"].clone()),
        (StatusCode::OK, json!(0), json!(0))
    );
    env.teardown().await;
}

#[tokio::test]
async fn mentionable_users_are_members_followers_and_owner() {
    let (env, anna) = world().await;
    let bruno = env.user("bruno", "utente").await;
    let carla = env.user("carla", "utente").await;
    let dario = env.user("dario", "utente").await;
    env.user("estraneo", "utente").await;
    let blog = env.blog("diario", anna, "public").await;
    env.member(blog, bruno, "co_autore").await;
    env.post("/blogs/diario/follow", Some(carla), json!({})).await;
    sqlx::query("UPDATE users SET display_name = 'Caro Dario' WHERE username = 'carla'")
        .execute(&env.pool)
        .await
        .unwrap();
    let url = "/blogs/diario/mentionable-users";

    assert_eq!(env.get(url, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.get(url, Some(dario)).await.0, StatusCode::FORBIDDEN);
    let (_, all) = env.get(url, Some(anna)).await;
    let names: Vec<_> = all.as_array().unwrap().iter().map(|u| u["username"].as_str().unwrap()).collect();
    assert_eq!(names, ["anna", "bruno", "carla"]);
    assert_eq!(
        env.get(&format!("{url}?q=%40br"), Some(bruno)).await.1.as_array().unwrap().len(),
        1,
        "@ iniziale ignorata"
    );
    let (_, by_display) = env.get(&format!("{url}?q=dario"), Some(anna)).await;
    assert_eq!(by_display[0]["username"], "carla", "il nome pubblico si cerca per sottostringa");
    assert_eq!(env.get(&format!("{url}?limit=1"), Some(anna)).await.1.as_array().unwrap().len(), 1);
    assert_eq!(env.get(&format!("{url}?q=_"), Some(anna)).await.1, json!([]), "_ è letterale");

    sqlx::query("UPDATE blogs SET mentions_enabled = false").execute(&env.pool).await.unwrap();
    assert_eq!(env.get(url, Some(anna)).await.1, json!([]));
    env.teardown().await;
}

// ---- viste su tutti i miei blog e interessi ---------------------------------------------------------------------------

#[tokio::test]
async fn author_views_span_owned_and_joined_blogs_but_not_deleted_ones() {
    let (env, anna) = world().await;
    let bruno = env.user("bruno", "utente").await;
    env.blog("mio", anna, "public").await;
    let altrui = env.blog("altrui", bruno, "public").await;
    env.blog("estraneo", bruno, "public").await;
    env.blog("cancellato", anna, "public").await;
    env.member(altrui, anna, "co_autore").await;

    let link = "Vedi [esempio](https://example.com/a) e ![i](https://x.org/m.png)";
    let p1 = post_in(&env, anna, "mio", "uno", json!({"content": link})).await;
    post_in(&env, anna, "altrui", "due", json!({"content": "[esempio](https://example.com/a)"})).await;
    post_in(&env, bruno, "estraneo", "tre", json!({"content": "[altro](https://other.org)"})).await;
    post_in(&env, anna, "cancellato", "quattro", json!({})).await;
    sqlx::query("UPDATE blogs SET deleted_at = now() WHERE slug = 'cancellato'").execute(&env.pool).await.unwrap();
    let pubr = env.post("/blogs/mio/publications", Some(anna), json!({"name": "guida", "title": "Guida"})).await.1;
    env.patch(&format!("/posts/{}", id(&p1)), Some(anna), json!({"publication_id": id(&pubr)})).await;
    sqlx::query("INSERT INTO media_files (blog_id, url, content_type, size_bytes) SELECT id, 'https://x.org/m.png', 'image/png', 10 FROM blogs WHERE slug = 'mio'")
        .execute(&env.pool)
        .await
        .unwrap();

    for path in ["/users/me/posts", "/users/me/media", "/users/me/links", "/users/me/publications"] {
        assert_eq!(env.get(path, None).await.0, StatusCode::UNAUTHORIZED, "{path}");
    }
    let (_, posts) = env.get("/users/me/posts", Some(anna)).await;
    assert_eq!(
        slugs(&posts),
        ["due", "uno"],
        "i miei e quelli dei blog in cui collaboro, non di blog altrui né cancellati"
    );
    let (_, links) = env.get("/users/me/links", Some(anna)).await;
    let links = links.as_array().unwrap();
    assert_eq!(links.len(), 1, "raggruppati per URL");
    assert_eq!(links[0]["url"], "https://example.com/a");
    assert_eq!(links[0]["citations"].as_array().unwrap().len(), 2);
    assert_eq!(links[0]["citations"][0]["blog_slug"], "altrui");
    let (_, media) = env.get("/users/me/media", Some(anna)).await;
    assert_eq!(
        (media[0]["blog_slug"].as_str(), media[0]["used_in"][0]["post_slug"].as_str()),
        (Some("mio"), Some("uno")),
        "{media}"
    );
    let (_, pubs) = env.get("/users/me/publications", Some(anna)).await;
    assert_eq!(
        (pubs[0]["blog_slug"].as_str(), pubs[0]["chapters_total"].clone(), pubs[0]["chapters_published"].clone()),
        (Some("mio"), json!(1), json!(0))
    );
    assert_eq!(
        env.get("/users/me/posts", Some(bruno)).await.1.as_array().unwrap().len(),
        2,
        "bruno: il post di anna sul suo blog e il suo estraneo"
    );
    env.teardown().await;
}

#[tokio::test]
async fn interests_are_public_and_seeded_with_the_curated_list() {
    let (env, _) = world().await;
    let (s, list) = env.get("/interests", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 16);
    assert_eq!(list[0]["key"], "music");
    sqlx::query("UPDATE interests_settings SET items = $1")
        .bind(json!([{"key": "poesia", "translations": {"it": "Poesia", "en": "Poetry"}}]))
        .execute(&env.pool)
        .await
        .unwrap();
    let (_, list) = env.get("/interests", None).await;
    assert_eq!(list[0]["translations"]["en"], "Poetry");
    env.teardown().await;

    // seme dall'ambiente; un valore malformato ricade sull'elenco predefinito
    let seed = r#"[{"key": "Poesia", "translations": {"it": "Poesia"}}]"#;
    let env = setup(&["interests"], &[("NOCT_DEFAULT_INTERESTS", seed)]).await;
    assert_eq!(env.get("/interests", None).await.1, json!([{"key": "poesia", "translations": {"it": "Poesia"}}]));
    env.teardown().await;
    let env = setup(&["interests"], &[("NOCT_DEFAULT_INTERESTS", "{non json")]).await;
    assert_eq!(env.get("/interests", None).await.1.as_array().unwrap().len(), 16);
    env.teardown().await;
}
