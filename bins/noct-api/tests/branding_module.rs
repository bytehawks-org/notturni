//! Identità e SEO del portale (icona, immagine di condivisione, meta tag per lingua, verifiche, blocco
//! dell'indicizzazione) e sitemap globale e per Spazio.

mod common;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use common::*;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde_json::{Value, json};
use std::io::Cursor;
use uuid::Uuid;

fn png(w: u32, h: u32) -> Vec<u8> {
    let img = RgbaImage::from_fn(w, h, |x, y| Rgba([(x % 251) as u8, (y % 241) as u8, 90, 255]));
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}

async fn upload(env: &Env, path: &str, who: Option<Uuid>, ct: &str, data: &[u8]) -> (StatusCode, Value) {
    let mut body =
        format!("--XB\r\nContent-Disposition: form-data; name=\"file\"; filename=\"x\"\r\nContent-Type: {ct}\r\n\r\n")
            .into_bytes();
    body.extend_from_slice(data);
    body.extend_from_slice(b"\r\n--XB--\r\n");
    let mut req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1{path}"))
        .header(header::CONTENT_TYPE, "multipart/form-data; boundary=XB");
    if let Some(u) = who {
        req = req.header(header::AUTHORIZATION, format!("Bearer {}", env.access_token(u)));
    }
    let (s, _, j) = env.send(req.body(Body::from(body)).unwrap()).await;
    (s, j)
}

fn on_disk(env: &Env, url: &str) -> std::path::PathBuf {
    env.storage_dir.join(url.strip_prefix("http://localhost:8000/storage/").expect("URL del backend locale"))
}

#[tokio::test]
async fn branding_and_seo_settings_are_validated_localized_and_served_publicly() {
    let env = setup(&["branding"], &[]).await;
    let root = env.user("root", "super_admin").await;
    let anna = env.user("anna", "utente").await;

    // predefiniti
    let (s, site) = env.get("/seo/site", None).await;
    assert_eq!(
        (s, site["site_name"].clone(), site["icons"].clone(), site["noindex"].clone()),
        (StatusCode::OK, json!("Notturni"), json!([]), json!(false))
    );
    assert_eq!(env.get("/admin/seo", None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.get("/admin/seo", Some(anna)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(env.put("/admin/seo", Some(anna), json!({})).await.0, StatusCode::FORBIDDEN);

    // validazione
    for bad in [
        json!({"google_verification": "<meta name=\"google-site-verification\" content=\"abc\">"}),
        json!({"twitter_site": "non valido!"}),
        json!({"theme_color": "verde"}),
        json!({"extra_meta": [{"name": "robots", "content": "noindex"}]}),
        json!({"extra_meta": [{"name": "ok", "content": "<x>"}]}),
        json!({"locales": {"ita": {"title": "x"}}}),
        json!({"locales": {"it": {"description": "a\nb"}}}),
        json!({"site_name": "a<b>"}),
    ] {
        assert_eq!(env.put("/admin/seo", Some(root), bad.clone()).await.0, StatusCode::BAD_REQUEST, "{bad}");
    }

    let settings = json!({
        "site_name": "Notturni Esempio",
        "google_verification": "AbCdEf1234567890_-xyz",
        "bing_verification": "0123456789ABCDEF0123456789ABCDEF",
        "twitter_site": "notturni",
        "theme_color": "#3D6B5C",
        "extra_meta": [{"name": "p:domain_verify", "content": "abc123"}],
        "locales": {
            "it": {"title": "Scrivere con calma", "description": "Una piattaforma per la parola.", "keywords": "blog, newsletter"},
            "en": {"title": "Write calmly", "description": "A platform for words."}
        }
    });
    let (s, out) = env.put("/admin/seo", Some(root), settings.clone()).await;
    assert_eq!(s, StatusCode::OK, "{out}");
    assert_eq!(out["branding"]["twitter_site"], "@notturni");
    assert_eq!(out["branding"]["theme_color"], "#3d6b5c");
    assert_eq!(out["locales"]["en"]["title"], "Write calmly");

    // lettura pubblica nella lingua richiesta, con ripiego su quella predefinita
    let (_, en) = env.get("/seo/site?locale=en", None).await;
    assert_eq!(
        (en["locale"].clone(), en["title"].clone(), en["site_name"].clone()),
        (json!("en"), json!("Write calmly"), json!("Notturni Esempio"))
    );
    assert_eq!(en["extra_meta"], json!([{"name": "p:domain_verify", "content": "abc123"}]));
    assert_eq!(env.get("/seo/site?locale=fr", None).await.1["locale"], "it", "lingua senza testi → quella predefinita");
    assert_eq!(env.get("/seo/site?locale=de", None).await.1["description"], "Una piattaforma per la parola.");

    // il PUT sostituisce l'insieme: senza `locales` le lingue spariscono; il blocco dell'indicizzazione si legge
    let (s, _) = env.put("/admin/seo", Some(root), json!({"noindex": true})).await;
    assert_eq!(s, StatusCode::OK);
    let (_, site) = env.get("/seo/site?locale=it", None).await;
    assert_eq!(
        (site["noindex"].clone(), site["title"].clone(), site["google_verification"].clone()),
        (json!(true), Value::Null, Value::Null)
    );

    let audited: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action LIKE 'branding.%'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(audited, 2);
    env.teardown().await;
}

#[tokio::test]
async fn icon_is_derived_in_several_sizes_and_the_social_image_is_checked() {
    let env = setup(&["branding"], &[]).await;
    let root = env.user("root", "super_admin").await;
    let anna = env.user("anna", "utente").await;

    assert_eq!(
        upload(&env, "/admin/branding/icon", None, "image/png", &png(600, 600)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        upload(&env, "/admin/branding/icon", Some(anna), "image/png", &png(600, 600)).await.0,
        StatusCode::FORBIDDEN
    );

    // un'immagine non adatta dice perché
    for (data, ct, needle) in [
        (png(100, 100), "image/png", "almeno"),
        (png(600, 300), "image/png", "quadrata"),
        (b"GIF89a....".to_vec(), "image/gif", "Formato immagine non supportato"),
        (b"non un'immagine".to_vec(), "image/png", "Formato immagine non supportato"),
    ] {
        let (s, b) = upload(&env, "/admin/branding/icon", Some(root), ct, &data).await;
        assert_eq!((s, detail(&b).contains(needle)), (StatusCode::BAD_REQUEST, true), "{needle}: {b}");
    }

    let (s, out) = upload(&env, "/admin/branding/icon", Some(root), "image/png", &png(600, 600)).await;
    assert_eq!(s, StatusCode::CREATED, "{out}");
    let icons = out["branding"]["icons"].as_object().unwrap();
    assert_eq!(icons.keys().cloned().collect::<Vec<_>>(), ["16", "180", "192", "32", "48", "512"]);
    for (side, url) in icons {
        let img = image::open(on_disk(&env, url.as_str().unwrap())).expect("file su storage");
        assert_eq!((img.width(), img.height()), (side.parse().unwrap(), side.parse().unwrap()), "lato {side}");
    }
    let (_, site) = env.get("/seo/site", None).await;
    assert_eq!(site["icons"].as_array().unwrap().len(), 6);
    assert_eq!(site["icons"][0]["sizes"], "16x16");
    assert!(site["apple_touch_icon"].as_str().unwrap().ends_with("-180.png"));
    let first_files: Vec<_> = icons.values().map(|u| on_disk(&env, u.as_str().unwrap())).collect();

    // una nuova icona sostituisce la precedente: i vecchi file spariscono; una piccola produce meno lati
    let (s, out) = upload(&env, "/admin/branding/icon", Some(root), "image/png", &png(200, 205)).await;
    assert_eq!(s, StatusCode::CREATED, "{out}");
    assert_eq!(out["branding"]["icons"].as_object().unwrap().len(), 5, "16, 32, 48, 180, 192");
    assert!(first_files.iter().all(|f| !f.exists()), "vecchi file cancellati");
    let current: Vec<_> =
        out["branding"]["icons"].as_object().unwrap().values().map(|u| on_disk(&env, u.as_str().unwrap())).collect();
    assert!(current.iter().all(|f| f.exists()));

    let (s, out) = env.delete_json("/admin/branding/icon", Some(root), json!({})).await;
    assert_eq!((s, out["branding"]["icons"].clone()), (StatusCode::OK, json!({})));
    assert!(current.iter().all(|f| !f.exists()), "file rimossi");
    assert_eq!(env.get("/seo/site", None).await.1["apple_touch_icon"], Value::Null);

    // immagine per le condivisioni: larghezza minima, conservata com'è
    let (s, b) = upload(&env, "/admin/branding/social", Some(root), "image/png", &png(400, 210)).await;
    assert_eq!((s, detail(&b).contains("600")), (StatusCode::BAD_REQUEST, true), "{b}");
    let (s, out) = upload(&env, "/admin/branding/social", Some(root), "image/png", &png(1200, 630)).await;
    assert_eq!(s, StatusCode::CREATED, "{out}");
    let social = out["branding"]["social_image_url"].as_str().unwrap().to_string();
    assert!(on_disk(&env, &social).exists());
    assert_eq!(env.get("/seo/site", None).await.1["social_image_url"], json!(social));
    env.delete_json("/admin/branding/social", Some(root), json!({})).await;
    assert!(!on_disk(&env, &social).exists());
    assert_eq!(env.get("/seo/site", None).await.1["social_image_url"], Value::Null);
    env.teardown().await;
}

fn paths(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|e| e["path"].as_str().unwrap().to_string()).collect()
}

#[tokio::test]
async fn sitemaps_list_only_indexable_content_with_translations_as_alternates() {
    let env = setup(&["branding", "pages", "publications"], &[]).await;
    let root = env.user("root", "super_admin").await;
    let anna = env.user("anna", "utente").await;
    env.blog("diario", anna, "public").await;
    env.blog("riservato", anna, "private").await;
    env.blog("chiuso", anna, "public").await;
    sqlx::query("UPDATE blogs SET search_indexing_enabled = false WHERE slug = 'chiuso'")
        .execute(&env.pool)
        .await
        .unwrap();

    let mk = |blog: &'static str, slug: &'static str, publish: bool, extra: Value| {
        let env = &env;
        async move {
            let mut body = json!({"slug": slug, "title": slug, "content": "testo"});
            body.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            let (s, p) = env.post(&format!("/blogs/{blog}/posts"), Some(anna), body).await;
            assert_eq!(s, StatusCode::CREATED, "{p}");
            if publish {
                assert_eq!(
                    env.post(&format!("/posts/{}/publish", p["id"].as_str().unwrap()), Some(anna), json!({})).await.0,
                    StatusCode::OK
                );
            }
            p
        }
    };
    let ciao = mk("diario", "ciao", true, json!({})).await;
    mk("diario", "bozza", false, json!({})).await;
    mk("diario", "escluso", true, json!({})).await;
    sqlx::query("UPDATE posts SET search_indexing_enabled = false WHERE slug = 'escluso'")
        .execute(&env.pool)
        .await
        .unwrap();
    mk("riservato", "privato", true, json!({})).await;
    mk("chiuso", "nascosto", true, json!({})).await;
    let (s, tr) = env
        .post(
            &format!("/posts/{}/translations", ciao["id"].as_str().unwrap()),
            Some(anna),
            json!({"slug": "hello", "locale": "en", "title": "Hello", "content": "text"}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{tr}");
    assert_eq!(
        env.post(&format!("/posts/{}/publish", tr["id"].as_str().unwrap()), Some(anna), json!({})).await.0,
        StatusCode::OK
    );
    let (s, _) = env
        .post(
            "/pages",
            Some(root),
            json!({"slug": "chi-siamo", "locale": "it", "title": "Chi siamo", "content": "x", "is_published": true}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED);
    sqlx::query("INSERT INTO blog_pages_settings (blog_id, static_pages_enabled) SELECT id, true FROM blogs WHERE slug = 'diario' ON CONFLICT DO NOTHING")
        .execute(&env.pool)
        .await
        .unwrap();
    let (s, bp) = env
        .post(
            "/blogs/diario/pages",
            Some(anna),
            json!({"slug": "info", "locale": "it", "title": "Info", "content": "x", "is_published": true}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{bp}");

    // globale: home, pagine del sito, Spazi, post indicizzabili (mai bozze, privati, esclusi)
    let (s, g) = env.get("/seo/sitemap", None).await;
    assert_eq!(s, StatusCode::OK, "{g}");
    assert_eq!(paths(&g["entries"]), ["/", "/p/chi-siamo", "/diario", "/diario/ciao", "/diario/hello"]);
    assert_eq!((g["total"].clone(), g["noindex"].clone()), (json!(5), json!(false)));
    let ciao_entry = &g["entries"][3];
    assert_eq!(
        ciao_entry["alternates"],
        json!([{"locale": "en", "path": "/diario/hello"}, {"locale": "it", "path": "/diario/ciao"}])
    );
    assert_eq!(g["entries"][2]["alternates"], json!([]));
    // ogni traduzione dichiara tutte le lingue, non solo la prima del gruppo
    assert_eq!(g["entries"][4]["alternates"], ciao_entry["alternates"]);

    // paginazione stabile
    let (_, p1) = env.get("/seo/sitemap?offset=0&limit=2", None).await;
    let (_, p2) = env.get("/seo/sitemap?offset=2&limit=2", None).await;
    assert_eq!(
        (p1["total"].clone(), paths(&p1["entries"])),
        (json!(5), vec!["/".to_string(), "/p/chi-siamo".to_string()])
    );
    assert_eq!(paths(&p2["entries"]), ["/diario", "/diario/ciao"]);
    assert_eq!(env.get("/seo/sitemap?limit=abc", None).await.0, StatusCode::BAD_REQUEST);

    // per Spazio: home, post, pagine statiche; Spazi privati o esclusi non esistono
    let (s, sp) = env.get("/blogs/diario/sitemap", None).await;
    assert_eq!(s, StatusCode::OK, "{sp}");
    let listed = paths(&sp["entries"]);
    assert_eq!(listed[0], "/diario");
    assert!(
        listed.contains(&"/diario/ciao".to_string())
            && listed.contains(&"/diario/hello".to_string())
            && listed.contains(&"/diario/pagina/info".to_string()),
        "{listed:?}"
    );
    assert!(!listed.iter().any(|p| p.contains("bozza") || p.contains("escluso")), "{listed:?}");
    assert_eq!(env.get("/blogs/riservato/sitemap", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/chiuso/sitemap", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/inesistente/sitemap", None).await.0, StatusCode::NOT_FOUND);

    // il portale chiuso agli indici ha una sitemap vuota
    assert_eq!(env.put("/admin/seo", Some(root), json!({"noindex": true})).await.0, StatusCode::OK);
    let (_, g) = env.get("/seo/sitemap", None).await;
    assert_eq!((g["total"].clone(), g["noindex"].clone(), g["entries"].clone()), (json!(0), json!(true), json!([])));
    assert_eq!(env.get("/blogs/diario/sitemap", None).await.0, StatusCode::NOT_FOUND, "nemmeno quella dello Spazio");
    env.teardown().await;
}
