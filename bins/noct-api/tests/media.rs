//! Immagini dei blog: upload (anche con contenuto travestito), libreria, copertina, favicon,
//! quota e moderazione fail-open. Storage locale su directory temporanea.

mod common;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use common::*;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use uuid::Uuid;

const PNG_HEAD: &[u8] = b"\x89PNG\r\n\x1a\n";

fn png(extra: usize) -> Vec<u8> {
    let mut v = PNG_HEAD.to_vec();
    v.extend(std::iter::repeat_n(7u8, extra));
    v
}

fn multipart(field: &str, filename: &str, content_type: &str, data: &[u8]) -> (String, Vec<u8>) {
    let mut body = format!(
        "--XBOUNDARYX\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(data);
    body.extend_from_slice(b"\r\n--XBOUNDARYX--\r\n");
    ("multipart/form-data; boundary=XBOUNDARYX".to_string(), body)
}

async fn upload(env: &Env, path: &str, who: Option<Uuid>, ct: &str, data: &[u8]) -> (StatusCode, Value) {
    let (content_type, body) = multipart("file", "foto", ct, data);
    let mut req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1{path}"))
        .header(header::CONTENT_TYPE, content_type);
    if let Some(u) = who {
        req = req.header(header::AUTHORIZATION, format!("Bearer {}", env.access_token(u)));
    }
    let (s, _, j) = env.send(req.body(Body::from(body)).unwrap()).await;
    (s, j)
}

/// File sul disco a partire dall'URL pubblico.
fn on_disk(env: &Env, url: &str) -> std::path::PathBuf {
    env.storage_dir.join(url.strip_prefix("http://localhost:8000/storage/").expect("URL del backend locale"))
}

struct World {
    env: Env,
    anna: Uuid,
    bruno: Uuid,
    carla: Uuid,
    blog: Uuid,
}

async fn world() -> World {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    let carla = env.user("carla", "utente").await;
    let blog = env.blog("diario", anna, "public").await;
    env.member(blog, bruno, "co_autore").await;
    env.member(blog, carla, "mediatore").await;
    World { env, anna, bruno, carla, blog }
}

#[tokio::test]
async fn media_upload_validates_what_is_really_inside() {
    let w = world().await;
    let env = &w.env;
    let ok = png(100);
    assert_eq!(upload(env, "/blogs/diario/media", None, "image/png", &ok).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(upload(env, "/blogs/diario/media", Some(w.carla), "image/png", &ok).await.0, StatusCode::FORBIDDEN);
    assert_eq!(upload(env, "/blogs/inesistente/media", Some(w.anna), "image/png", &ok).await.0, StatusCode::NOT_FOUND);

    // tipo dichiarato non ammesso, contenuto che non è un'immagine, tipo dichiarato diverso dal reale
    for (ct, data) in [
        ("image/svg+xml", b"<svg onload=alert(1)/>".to_vec()),
        ("image/png", b"<html><script>alert(1)</script></html>".to_vec()),
        ("image/jpeg", ok.clone()),
        ("text/plain", ok.clone()),
    ] {
        let (s, b) = upload(env, "/blogs/diario/media", Some(w.anna), ct, &data).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{ct}: {b}");
        assert!(detail(&b).contains("Formato immagine non supportato"), "{b}");
    }
    let (s, b) = upload(env, "/blogs/diario/media", Some(w.anna), "image/png", &png(10 * 1024 * 1024)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert!(detail(&b).contains("10 MiB"));

    // multipart senza il campo `file`, o corpo che non è multipart
    let (ct, body) = multipart("altro", "x", "image/png", &ok);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/blogs/diario/media")
        .header(header::CONTENT_TYPE, ct)
        .header(header::AUTHORIZATION, format!("Bearer {}", env.access_token(w.anna)))
        .body(Body::from(body))
        .unwrap();
    assert_eq!(env.send(req).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(env.post("/blogs/diario/media", Some(w.anna), json!({})).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    let (s, m) = upload(env, "/blogs/diario/media", Some(w.bruno), "image/png", &ok).await;
    assert_eq!(s, StatusCode::CREATED, "{m}");
    assert_eq!(m["is_sensitive"], false, "moderazione non configurata: fail open");
    let url = m["url"].as_str().unwrap();
    assert!(
        url.contains(&format!("/notturni-content/notturni/userdata/{}/{}/media/", w.anna, w.blog)),
        "{url}: il prefisso usa il proprietario"
    );
    assert!(url.ends_with(".png"));
    assert_eq!(std::fs::read(on_disk(env, url)).unwrap(), ok);
    let (size, uploader): (i64, Uuid) = sqlx::query_as("SELECT size_bytes, uploader_id FROM media_files WHERE id = $1")
        .bind(Uuid::parse_str(m["media_id"].as_str().unwrap()).unwrap())
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!((size as usize, uploader), (ok.len(), w.bruno));
    w.env.teardown().await;
}

#[tokio::test]
async fn library_lists_updates_and_protects_used_images() {
    let w = world().await;
    let env = &w.env;
    let (_, up1) = upload(env, "/blogs/diario/media", Some(w.anna), "image/png", &png(10)).await;
    let (_, up2) = upload(env, "/blogs/diario/media", Some(w.bruno), "image/png", &png(20)).await;
    let (url1, url2) = (up1["url"].as_str().unwrap().to_string(), up2["url"].as_str().unwrap().to_string());
    let (id1, id2) = (up1["media_id"].as_str().unwrap().to_string(), up2["media_id"].as_str().unwrap().to_string());

    // elenco: proprietario e collaboratori (anche il mediatore); gli altri no
    let stranger = env.user("estraneo", "utente").await;
    assert_eq!(env.get("/blogs/diario/media", None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.get("/blogs/diario/media", Some(stranger)).await.0, StatusCode::FORBIDDEN);
    let (s, lib) = env.get("/blogs/diario/media", Some(w.carla)).await;
    assert_eq!(s, StatusCode::OK, "{lib}");
    assert_eq!(lib["items"].as_array().unwrap().len(), 2);
    assert_eq!(lib["total_bytes"], (PNG_HEAD.len() * 2 + 30) as i64);
    let item2 = lib["items"].as_array().unwrap().iter().find(|i| i["id"] == id2.as_str()).unwrap();
    assert_eq!(item2["uploader_username"], "bruno");
    assert_eq!(item2["used_in"], json!([]));
    assert_eq!(item2["used_as_blog_cover"], false);

    // alt, didascalia, avviso: bruno scrive, carla (mediatrice) no
    let path = format!("/blogs/diario/media/{id1}");
    assert_eq!(env.patch(&path, Some(w.carla), json!({"alt_text": "x"})).await.0, StatusCode::FORBIDDEN);
    let (s, m) = env
        .patch(
            &path,
            Some(w.bruno),
            json!({"alt_text": "  Un gatto  ", "caption": "Didascalia", "categories": ["nudity", "nudity"]}),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{m}");
    assert_eq!((m["alt_text"].as_str(), m["caption"].as_str()), (Some("Un gatto"), Some("Didascalia")));
    assert_eq!(m["categories"], json!(["nudity"]));
    let (_, m) = env.patch(&path, Some(w.bruno), json!({"caption": "  "})).await;
    assert_eq!(m["caption"], Value::Null, "vuota = nessuna didascalia");
    assert_eq!(env.patch(&path, Some(w.bruno), json!({"categories": ["boh"]})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        env.patch(&format!("/blogs/diario/media/{}", Uuid::new_v4()), Some(w.bruno), json!({})).await.0,
        StatusCode::NOT_FOUND
    );

    // un'immagine citata nel testo o usata come copertina di un post non si cancella
    let (s, post) = env
        .post(
            "/blogs/diario/posts",
            Some(w.anna),
            json!({"slug": "p", "title": "P", "content": format!("![x]({url1})"), "cover_image_url": url2}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{post}");
    let (_, lib) = env.get("/blogs/diario/media", Some(w.anna)).await;
    for item in lib["items"].as_array().unwrap() {
        assert_eq!(item["used_in"][0]["post_slug"], "p", "{item}");
        assert_eq!(item["used_in"][0]["permalink"], "/diario/p");
    }
    for id in [&id1, &id2] {
        let (s, b) =
            env.call(Method::DELETE, &format!("/blogs/diario/media/{id}"), Some(w.anna), None).await.into_pair();
        assert_eq!(s, StatusCode::CONFLICT, "{b}");
        assert!(detail(&b).contains("ancora usata"));
    }
    // libera la prima: il testo non la cita più; la seconda resta come copertina del post
    env.patch(
        &format!("/posts/{}", post["id"].as_str().unwrap()),
        Some(w.anna),
        json!({"content": "niente immagini", "cover_image_url": ""}),
    )
    .await;
    assert_eq!(env.delete(&format!("/blogs/diario/media/{id1}"), Some(w.anna)).await, StatusCode::NO_CONTENT);
    assert!(!on_disk(env, &url1).exists(), "l'oggetto su storage è stato cancellato");
    assert_eq!(env.delete(&format!("/blogs/diario/media/{id1}"), Some(w.anna)).await, StatusCode::NOT_FOUND);
    assert_eq!(env.delete(&format!("/blogs/diario/media/{id2}"), Some(w.carla)).await, StatusCode::FORBIDDEN);
    w.env.teardown().await;
}

#[tokio::test]
async fn sync_imports_images_cited_in_posts() {
    let w = world().await;
    let env = &w.env;
    for (slug, md) in [
        ("a", "![uno](https://i.org/1.png) ![due](https://i.org/2.png \"sensitive:nudity\")"),
        ("b", "![uno bis](https://i.org/1.png \"sensitive\")"),
    ] {
        env.post("/blogs/diario/posts", Some(w.anna), json!({"slug": slug, "title": slug, "content": md})).await;
    }
    assert_eq!(env.post("/blogs/diario/media/sync", Some(w.carla), json!({})).await.0, StatusCode::FORBIDDEN);
    let (s, lib) = env.post("/blogs/diario/media/sync", Some(w.anna), json!({})).await;
    assert_eq!(s, StatusCode::OK, "{lib}");
    let items = lib["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    let one = items.iter().find(|i| i["url"] == "https://i.org/1.png").unwrap();
    assert_eq!(one["is_sensitive"], true, "una citazione sensibile basta");
    assert_eq!((one["size_bytes"].as_i64(), one["uploader_username"].clone()), (Some(0), Value::Null));
    assert_eq!(one["used_in"].as_array().unwrap().len(), 2);
    // idempotente
    let (_, again) = env.post("/blogs/diario/media/sync", Some(w.anna), json!({})).await;
    assert_eq!(again["items"].as_array().unwrap().len(), 2);
    w.env.teardown().await;
}

#[tokio::test]
async fn cover_and_favicon_are_owner_only_and_replace_cleanly() {
    let w = world().await;
    let env = &w.env;
    let p = png(30);

    assert_eq!(upload(env, "/blogs/diario/cover-image", Some(w.bruno), "image/png", &p).await.0, StatusCode::FORBIDDEN);
    assert_eq!(upload(env, "/blogs/diario/cover-image", None, "image/png", &p).await.0, StatusCode::UNAUTHORIZED);
    let (s, b) = env.patch("/blogs/diario/cover-image", Some(w.anna), json!({"categories": []})).await;
    assert_eq!((s, detail(&b)), (StatusCode::BAD_REQUEST, "Il blog non ha una cover da annotare."));

    let (s, blog) = upload(env, "/blogs/diario/cover-image", Some(w.anna), "image/png", &p).await;
    assert_eq!(s, StatusCode::CREATED, "{blog}");
    let cover = blog["cover_image_url"].as_str().unwrap().to_string();
    assert_eq!(
        (blog["cover_image_is_sensitive"].as_bool(), blog["owner_id"].clone()),
        (Some(false), json!(w.anna.to_string()))
    );
    assert!(on_disk(env, &cover).exists());
    let (_, lib) = env.get("/blogs/diario/media", Some(w.anna)).await;
    assert_eq!(lib["items"][0]["used_as_blog_cover"], true, "la copertina entra nella libreria");

    let (s, blog) = env
        .patch("/blogs/diario/cover-image", Some(w.anna), json!({"categories": ["explicit"], "alt_text": "Mare"}))
        .await;
    assert_eq!(s, StatusCode::OK, "{blog}");
    assert_eq!(
        (
            blog["cover_image_is_sensitive"].clone(),
            blog["cover_image_categories"].clone(),
            blog["cover_image_alt_text"].clone()
        ),
        (json!(true), json!(["explicit"]), json!("Mare"))
    );
    let (_, blog) = env.patch("/blogs/diario/cover-image", Some(w.anna), json!({"categories": []})).await;
    assert_eq!(
        (blog["cover_image_is_sensitive"].clone(), blog["cover_image_alt_text"].clone()),
        (json!(false), json!("Mare")),
        "alt assente: invariato"
    );
    let (_, blog) =
        env.patch("/blogs/diario/cover-image", Some(w.anna), json!({"categories": [], "alt_text": null})).await;
    assert_eq!(blog["cover_image_alt_text"], "");
    assert_eq!(
        env.patch("/blogs/diario/cover-image", Some(w.anna), json!({"categories": ["boh"]})).await.0,
        StatusCode::BAD_REQUEST
    );

    // l'immagine che è la copertina non si cancella dalla libreria
    let media_id = lib["items"][0]["id"].as_str().unwrap();
    assert_eq!(env.delete(&format!("/blogs/diario/media/{media_id}"), Some(w.anna)).await, StatusCode::CONFLICT);
    let (s, blog) = env.call(Method::DELETE, "/blogs/diario/cover-image", Some(w.anna), None).await.into_pair();
    assert_eq!((s, blog["cover_image_url"].clone()), (StatusCode::OK, Value::Null));
    assert!(on_disk(env, &cover).exists(), "l'oggetto della copertina resta");
    assert_eq!(env.delete(&format!("/blogs/diario/media/{media_id}"), Some(w.anna)).await, StatusCode::NO_CONTENT);

    // favicon
    assert_eq!(upload(env, "/blogs/diario/favicon", Some(w.bruno), "image/png", &p).await.0, StatusCode::FORBIDDEN);
    let gif = b"GIF89a....".to_vec();
    assert_eq!(
        upload(env, "/blogs/diario/favicon", Some(w.anna), "image/gif", &gif).await.0,
        StatusCode::BAD_REQUEST,
        "niente GIF"
    );
    let (s, b) = upload(env, "/blogs/diario/favicon", Some(w.anna), "image/png", &png(600 * 1024)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert!(detail(&b).contains("512 KiB"));
    let (s, blog) = upload(env, "/blogs/diario/favicon", Some(w.anna), "image/png", &p).await;
    assert_eq!(s, StatusCode::CREATED, "{blog}");
    let first = blog["favicon_url"].as_str().unwrap().to_string();
    assert!(first.contains(&format!("/avatars/favicons/{}/", w.blog)), "{first}");
    assert!(on_disk(env, &first).exists());
    let (_, blog) = upload(env, "/blogs/diario/favicon", Some(w.anna), "image/png", &png(40)).await;
    let second = blog["favicon_url"].as_str().unwrap().to_string();
    assert_ne!(first, second);
    assert!(!on_disk(env, &first).exists(), "la favicon precedente è stata cancellata");
    assert!(on_disk(env, &second).exists());
    assert_eq!(env.get("/blogs/diario", None).await.1["favicon_url"], second.as_str());
    let (_, blog) = env.call(Method::DELETE, "/blogs/diario/favicon", Some(w.anna), None).await.into_pair();
    assert_eq!(blog["favicon_url"], Value::Null);
    assert!(!on_disk(env, &second).exists());
    // idempotente
    assert_eq!(env.call(Method::DELETE, "/blogs/diario/favicon", Some(w.anna), None).await.0, StatusCode::OK);
    w.env.teardown().await;
}

#[tokio::test]
async fn storage_quota_is_enforced_per_blog() {
    let w = world().await;
    let env = &w.env;
    assert_eq!(
        upload(env, "/blogs/diario/media", Some(w.anna), "image/png", &png(900 * 1024)).await.0,
        StatusCode::CREATED,
        "nessun limite di default"
    );
    sqlx::query("INSERT INTO platform_config (id, max_blog_storage_mb) VALUES (1, 1) ON CONFLICT (id) DO UPDATE SET max_blog_storage_mb = 1")
        .execute(&env.pool).await.unwrap();
    let (s, b) = upload(env, "/blogs/diario/media", Some(w.anna), "image/png", &png(200 * 1024)).await;
    assert_eq!((s, detail(&b)), (StatusCode::PAYLOAD_TOO_LARGE, "Spazio massimo per questo blog superato (1 MB)."));
    let (s, _) = upload(env, "/blogs/diario/cover-image", Some(w.anna), "image/png", &png(200 * 1024)).await;
    assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE, "vale anche per la copertina");
    assert_eq!(
        upload(env, "/blogs/diario/media", Some(w.anna), "image/png", &png(50 * 1024)).await.0,
        StatusCode::CREATED
    );
    // un altro blog ha il proprio spazio
    env.blog("altro", w.anna, "public").await;
    assert_eq!(
        upload(env, "/blogs/altro/media", Some(w.anna), "image/png", &png(900 * 1024)).await.0,
        StatusCode::CREATED
    );
    w.env.teardown().await;
}

/// Servizio di moderazione finto: risponde sempre `is_sensitive: true`.
async fn fake_moderation() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { return };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 64 * 1024];
                let mut total = 0;
                // legge fino alla fine del multipart (l'ultimo boundary termina con "--\r\n")
                loop {
                    let n = sock.read(&mut buf[total..]).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    total += n;
                    if buf[..total].ends_with(b"--\r\n") {
                        break;
                    }
                }
                let body = r#"{"is_sensitive": true}"#;
                let _ = sock
                    .write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes())
                    .await;
            });
        }
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn moderation_flags_uploads_and_never_blocks_them() {
    let url = fake_moderation().await;
    let env = setup_with(&[], &[], Some(&move |cfg| cfg.moderation_service_url = Some(url.clone()))).await;
    let anna = env.user("anna", "utente").await;
    env.blog("diario", anna, "public").await;
    let (s, m) = upload(&env, "/blogs/diario/media", Some(anna), "image/png", &png(50)).await;
    assert_eq!((s, m["is_sensitive"].as_bool()), (StatusCode::CREATED, Some(true)), "{m}");
    let (_, blog) = upload(&env, "/blogs/diario/cover-image", Some(anna), "image/png", &png(50)).await;
    assert_eq!(blog["cover_image_is_sensitive"], true);
    env.teardown().await;

    // servizio irraggiungibile: l'upload riesce comunque, non sensibile
    let env = setup_with(&[], &[], Some(&|cfg| cfg.moderation_service_url = Some("http://127.0.0.1:1".into()))).await;
    let anna = env.user("anna", "utente").await;
    env.blog("diario", anna, "public").await;
    let (s, m) = upload(&env, "/blogs/diario/media", Some(anna), "image/png", &png(50)).await;
    assert_eq!((s, m["is_sensitive"].as_bool()), (StatusCode::CREATED, Some(false)), "{m}");
    env.teardown().await;
}
