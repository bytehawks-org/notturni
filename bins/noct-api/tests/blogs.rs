//! Blog: creazione e regole di dominio, visibilità, collaboratori e inviti, ciclo di vita,
//! configurazione di presentazione. Router completo + Postgres vero.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use uuid::Uuid;

async fn audit_actions(env: &Env, blog: Uuid) -> Vec<String> {
    sqlx::query_scalar("SELECT action FROM audit_log WHERE blog_id = $1 ORDER BY occurred_at")
        .bind(blog)
        .fetch_all(&env.pool)
        .await
        .unwrap()
}

fn new_blog(slug: &str) -> Value {
    json!({"slug": slug, "title": format!("Blog {slug}")})
}

// ---- creazione --------------------------------------------------------------------------------

#[tokio::test]
async fn create_blog_applies_domain_rules() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;

    assert_eq!(env.post("/blogs", None, new_blog("diario")).await.0, StatusCode::UNAUTHORIZED);

    let (s, b) = env.post("/blogs", Some(anna), new_blog("diario")).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    assert_eq!(b["slug"], "diario");
    assert_eq!(b["visibility"], "public");
    assert_eq!(b["default_locale"], "it");
    assert_eq!(b["owner_id"], anna.to_string(), "il proprietario vede il proprio id");
    assert_eq!(b["favicon_url"], Value::Null);
    assert_eq!(b["extra_locales"], json!([]));
    assert_eq!(b["mentions_enabled"], true);
    assert_eq!(b["is_paused"], false);

    for (slug, needle) in [
        ("abc", "almeno 4"),
        ("Diario", "minuscole"),
        ("-diario", "minuscole"),
        ("dia_rio", "minuscole"),
        ("admin", "riservato"),
        ("api", "almeno 4"),
    ] {
        let (s, b) = env.post("/blogs", Some(anna), new_blog(slug)).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{slug}: {b}");
        assert!(detail(&b).contains(needle), "{slug}: {b}");
    }
    let (s, b) = env.post("/blogs", Some(anna), new_blog("diario")).await;
    assert_eq!((s, detail(&b)), (StatusCode::CONFLICT, "Slug già in uso."));

    for body in [
        json!({"slug": "altro", "title": "x", "default_locale": "ita"}),
        json!({"slug": "altro", "title": "x", "subtitle": "s".repeat(65)}),
        json!({"slug": "altro", "title": "x", "description": "d".repeat(257)}),
        json!({"slug": "altro", "title": "  "}),
        json!({"slug": "altro", "title": "x", "visibility": "segreto"}),
    ] {
        assert_eq!(env.post("/blogs", Some(anna), body.clone()).await.0, StatusCode::BAD_REQUEST, "{body}");
    }
    let (s, b) = env
        .post(
            "/blogs",
            Some(anna),
            json!({"slug": "altro", "title": "Altro", "visibility": "private", "default_locale": "en",
                   "subtitle": "Sotto", "default_author_display_name": "Anonima"}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    assert_eq!((b["visibility"].as_str(), b["default_locale"].as_str()), (Some("private"), Some("en")));
    assert_eq!(b["default_author_display_name"], "Anonima");

    let blog: Uuid =
        sqlx::query_scalar("SELECT id FROM blogs WHERE slug = 'diario'").fetch_one(&env.pool).await.unwrap();
    assert_eq!(audit_actions(&env, blog).await, vec!["blog.created"]);
    env.teardown().await;
}

#[tokio::test]
async fn blog_limit_per_user_is_enforced_even_under_concurrency() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    // 8 richieste contemporanee: il limite predefinito è 5
    let results =
        futures::future::join_all((0..8).map(|i| env.post("/blogs", Some(anna), new_blog(&format!("blog-{i}"))))).await;
    let created = results.iter().filter(|(s, _)| *s == StatusCode::CREATED).count();
    assert_eq!(created, 5, "{results:?}");
    let rejected: Vec<_> = results.iter().filter(|(s, _)| *s != StatusCode::CREATED).collect();
    assert!(rejected.iter().all(|(s, b)| *s == StatusCode::BAD_REQUEST && detail(b).contains("Limite massimo di 5")));
    // il limite è di piattaforma: un Super Admin può cambiarlo
    sqlx::query("INSERT INTO platform_config (id, max_blogs_per_user, reserved_blog_names) VALUES (1, 6, '{riservato}') ON CONFLICT (id) DO UPDATE SET max_blogs_per_user = 6, reserved_blog_names = '{riservato}'")
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(env.post("/blogs", Some(anna), new_blog("sesto-blog")).await.0, StatusCode::CREATED);
    let (s, b) = env.post("/blogs", Some(anna), new_blog("riservato")).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert!(detail(&b).contains("riservato"));
    env.teardown().await;
}

// ---- visibilità -------------------------------------------------------------------------------------

#[tokio::test]
async fn blog_detail_follows_visibility_and_state() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    let carla = env.user("carla", "utente").await;
    let public = env.blog("pubblico", anna, "public").await;
    let members = env.blog("soci", anna, "members").await;
    let private = env.blog("diario", anna, "private").await;
    env.member(private, bruno, "co_autore").await;

    // pubblico: tutti, ma l'id del proprietario è visibile solo a lui
    let (s, b) = env.get("/blogs/pubblico", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["owner_id"], Value::Null);
    assert_eq!(env.get("/blogs/pubblico", Some(bruno)).await.1["owner_id"], Value::Null);
    assert_eq!(env.get("/blogs/pubblico", Some(anna)).await.1["owner_id"], anna.to_string());

    // members: solo autenticati; private: proprietario e membri; altrimenti 404 (non 403)
    assert_eq!(env.get("/blogs/soci", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/soci", Some(carla)).await.0, StatusCode::OK);
    assert_eq!(env.get("/blogs/diario", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/diario", Some(carla)).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/diario", Some(bruno)).await.0, StatusCode::OK);
    assert_eq!(env.get("/blogs/non-esiste", None).await.0, StatusCode::NOT_FOUND);

    // pubblico in pausa o sospeso: il dettaglio resta leggibile (spiega lo stato)
    sqlx::query("UPDATE blogs SET is_paused = true WHERE id = $1").bind(public).execute(&env.pool).await.unwrap();
    let (s, b) = env.get("/blogs/pubblico", None).await;
    assert_eq!((s, b["is_paused"].as_bool()), (StatusCode::OK, Some(true)));
    sqlx::query("UPDATE blogs SET is_paused = false, is_suspended = true WHERE id = $1")
        .bind(public)
        .execute(&env.pool)
        .await
        .unwrap();
    let (s, b) = env.get("/blogs/pubblico", None).await;
    assert_eq!((s, b["is_suspended"].as_bool()), (StatusCode::OK, Some(true)));
    // ma un blog "soci" in pausa non lo è: la pausa non apre ciò che la visibilità chiude
    sqlx::query("UPDATE blogs SET is_paused = true WHERE id = $1").bind(members).execute(&env.pool).await.unwrap();
    assert_eq!(env.get("/blogs/soci", Some(carla)).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/soci", Some(anna)).await.0, StatusCode::OK);

    // cancellato: sparisce per tutti tranne il proprietario
    sqlx::query("UPDATE blogs SET is_suspended = false, deleted_at = now() WHERE id = $1")
        .bind(public)
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(env.get("/blogs/pubblico", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/pubblico", Some(bruno)).await.0, StatusCode::NOT_FOUND);
    let (s, b) = env.get("/blogs/pubblico", Some(anna)).await;
    assert_eq!(s, StatusCode::OK);
    assert!(b["deleted_at"].is_string());
    env.teardown().await;
}

// ---- modifica ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn update_blog_is_owner_only_and_tristate() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    let id = env.blog("diario", anna, "public").await;
    env.member(id, bruno, "co_autore").await;
    sqlx::query("UPDATE blogs SET subtitle = 'Sotto', description = 'Desc', default_author_display_name = 'Alias' WHERE id = $1")
        .bind(id)
        .execute(&env.pool)
        .await
        .unwrap();

    assert_eq!(env.patch("/blogs/diario", None, json!({"title": "x"})).await.0, StatusCode::UNAUTHORIZED);
    let (s, b) = env.patch("/blogs/diario", Some(bruno), json!({"title": "x"})).await;
    assert_eq!((s, detail(&b)), (StatusCode::FORBIDDEN, "Solo il proprietario può modificare il blog."));
    assert_eq!(env.patch("/blogs/nessuno", Some(anna), json!({})).await.0, StatusCode::NOT_FOUND);

    // assente = invariato; "" = azzera
    let (s, b) = env
        .patch(
            "/blogs/diario",
            Some(anna),
            json!({"title": "Nuovo titolo", "subtitle": "", "visibility": "members", "mentions_enabled": false,
                   "search_indexing_enabled": false, "ai_crawling_enabled": false, "is_paused": true,
                   "default_author_display_name": "", "extra_locales": ["en", "it", "en", "de"]}),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["title"], "Nuovo titolo");
    assert_eq!(b["subtitle"], Value::Null);
    assert_eq!(b["description"], "Desc", "non inviato: invariato");
    assert_eq!(b["default_author_display_name"], Value::Null);
    assert_eq!(b["visibility"], "members");
    assert_eq!((b["mentions_enabled"].as_bool(), b["is_paused"].as_bool()), (Some(false), Some(true)));
    assert_eq!(
        (b["search_indexing_enabled"].as_bool(), b["ai_crawling_enabled"].as_bool()),
        (Some(false), Some(false))
    );
    assert_eq!(b["extra_locales"], json!(["en", "de"]), "duplicati e lingua principale scartati");

    for body in [
        json!({"visibility": "boh"}),
        json!({"subtitle": "s".repeat(65)}),
        json!({"description": "d".repeat(257)}),
        json!({"extra_locales": ["xx1"]}),
        json!({"title": ""}),
    ] {
        assert_eq!(env.patch("/blogs/diario", Some(anna), body.clone()).await.0, StatusCode::BAD_REQUEST, "{body}");
    }
    // corpo vuoto: nessuna modifica, risposta 200
    assert_eq!(env.patch("/blogs/diario", Some(anna), json!({})).await.0, StatusCode::OK);
    env.teardown().await;
}

#[tokio::test]
async fn my_blogs_and_memberships_are_listed() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    let b1 = env.blog("primo", anna, "public").await;
    env.blog("secondo", anna, "private").await;
    let b3 = env.blog("terzo", bruno, "public").await;
    env.member(b3, anna, "mediatore").await;
    env.member(b1, bruno, "co_autore").await;

    let (s, mine) = env.get("/blogs/mine", Some(anna)).await;
    assert_eq!(s, StatusCode::OK);
    let slugs: Vec<_> = mine.as_array().unwrap().iter().map(|b| b["slug"].as_str().unwrap()).collect();
    assert_eq!(slugs, ["primo", "secondo"]);
    assert!(mine.as_array().unwrap().iter().all(|b| b["owner_id"] == anna.to_string()));
    assert_eq!(env.get("/blogs/mine", None).await.0, StatusCode::UNAUTHORIZED);

    let (s, member_of) = env.get("/blogs/member-of", Some(anna)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(member_of.as_array().unwrap().len(), 1);
    assert_eq!(member_of[0]["blog"]["slug"], "terzo");
    assert_eq!(member_of[0]["role"], "mediatore");
    assert_eq!(member_of[0]["blog"]["owner_id"], Value::Null, "non è il proprietario");
    assert_eq!(member_of[0]["author_display_name"], Value::Null);

    // alias di firma del collaboratore
    let (s, m) = env.patch("/blogs/terzo/my-membership", Some(anna), json!({"author_display_name": "Firma"})).await;
    assert_eq!(s, StatusCode::OK, "{m}");
    assert_eq!((m["role"].as_str(), m["author_display_name"].as_str()), (Some("mediatore"), Some("Firma")));
    let (_, m) = env.patch("/blogs/terzo/my-membership", Some(anna), json!({})).await;
    assert_eq!(m["author_display_name"], "Firma", "assente = invariato");
    let (_, m) = env.patch("/blogs/terzo/my-membership", Some(anna), json!({"author_display_name": ""})).await;
    assert_eq!(m["author_display_name"], Value::Null);
    let (s, b) = env.patch("/blogs/primo/my-membership", Some(anna), json!({"author_display_name": "x"})).await;
    assert_eq!((s, detail(&b)), (StatusCode::NOT_FOUND, "Non sei un collaboratore di questo blog."));
    env.teardown().await;
}

// ---- collaboratori e inviti ----------------------------------------------------------------------------

#[tokio::test]
async fn invitation_flow_creates_the_membership_and_is_audited() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    let carla = env.user("carla", "utente").await;
    let blog = env.blog("diario", anna, "public").await;

    // solo il proprietario gestisce inviti e collaboratori
    assert_eq!(env.get("/blogs/diario/invitations", Some(bruno)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(env.get("/blogs/diario/members", Some(bruno)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(
        env.post("/blogs/diario/invitations", Some(bruno), json!({"username": "carla", "role": "co_autore"})).await.0,
        StatusCode::FORBIDDEN
    );

    let (s, b) =
        env.post("/blogs/diario/invitations", Some(anna), json!({"username": "bruno", "role": "autore"})).await;
    assert_eq!((s, detail(&b)), (StatusCode::BAD_REQUEST, "Si può invitare solo come co_autore o mediatore."));
    assert_eq!(
        env.post("/blogs/diario/invitations", Some(anna), json!({"username": "nessuno", "role": "co_autore"})).await.0,
        StatusCode::NOT_FOUND
    );
    let (s, b) =
        env.post("/blogs/diario/invitations", Some(anna), json!({"username": "anna", "role": "co_autore"})).await;
    assert_eq!((s, detail(&b)), (StatusCode::BAD_REQUEST, "Sei già il proprietario del blog."));

    let (s, inv) =
        env.post("/blogs/diario/invitations", Some(anna), json!({"username": "bruno", "role": "co_autore"})).await;
    assert_eq!(s, StatusCode::CREATED, "{inv}");
    assert_eq!((inv["status"].as_str(), inv["role"].as_str()), (Some("pending"), Some("co_autore")));
    assert_eq!(
        (inv["blog_slug"].as_str(), inv["invited_username"].as_str(), inv["invited_by_username"].as_str()),
        (Some("diario"), Some("bruno"), Some("anna"))
    );
    let inv_id = inv["id"].as_str().unwrap().to_string();
    let (s, b) =
        env.post("/blogs/diario/invitations", Some(anna), json!({"username": "bruno", "role": "mediatore"})).await;
    assert_eq!((s, detail(&b)), (StatusCode::CONFLICT, "C'è già un invito in attesa per questo utente."));

    // l'invitato lo vede; un altro utente no e non può rispondere
    let (_, received) = env.get("/blogs/received-invitations", Some(bruno)).await;
    assert_eq!(received.as_array().unwrap().len(), 1);
    assert!(env.get("/blogs/received-invitations", Some(carla)).await.1.as_array().unwrap().is_empty());
    assert_eq!(
        env.post(&format!("/blogs/received-invitations/{inv_id}/accept"), Some(carla), json!({})).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        env.post("/blogs/received-invitations/non-un-uuid/accept", Some(bruno), json!({})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let (s, acc) = env.post(&format!("/blogs/received-invitations/{inv_id}/accept"), Some(bruno), json!({})).await;
    assert_eq!(s, StatusCode::OK, "{acc}");
    assert_eq!(acc["status"], "accepted");
    assert!(acc["responded_at"].is_string());
    let (s, b) = env.post(&format!("/blogs/received-invitations/{inv_id}/accept"), Some(bruno), json!({})).await;
    assert_eq!((s, detail(&b)), (StatusCode::CONFLICT, "Invito già gestito."));
    assert!(env.get("/blogs/received-invitations", Some(bruno)).await.1.as_array().unwrap().is_empty());

    // ora è un collaboratore: scrive, ed è elencato
    let (_, members) = env.get("/blogs/diario/members", Some(anna)).await;
    assert_eq!(members.as_array().unwrap().len(), 1);
    assert_eq!((members[0]["username"].as_str(), members[0]["role"].as_str()), (Some("bruno"), Some("co_autore")));
    let (s, b) =
        env.post("/blogs/diario/invitations", Some(anna), json!({"username": "bruno", "role": "mediatore"})).await;
    assert_eq!((s, detail(&b)), (StatusCode::CONFLICT, "L'utente è già un collaboratore."));

    // ruolo: solo co_autore/mediatore; membro inesistente 404
    let (s, m) = env.patch(&format!("/blogs/diario/members/{bruno}"), Some(anna), json!({"role": "mediatore"})).await;
    assert_eq!((s, m["role"].as_str()), (StatusCode::OK, Some("mediatore")));
    assert_eq!(
        env.patch(&format!("/blogs/diario/members/{bruno}"), Some(anna), json!({"role": "revisore"})).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        env.patch(&format!("/blogs/diario/members/{carla}"), Some(anna), json!({"role": "mediatore"})).await.0,
        StatusCode::NOT_FOUND
    );

    // rimozione idempotente
    assert_eq!(env.delete(&format!("/blogs/diario/members/{bruno}"), Some(anna)).await, StatusCode::NO_CONTENT);
    assert_eq!(env.delete(&format!("/blogs/diario/members/{bruno}"), Some(anna)).await, StatusCode::NO_CONTENT);
    assert!(env.get("/blogs/diario/members", Some(anna)).await.1.as_array().unwrap().is_empty());

    // rifiuto e nuovo invito riusano la stessa riga
    let (_, inv2) =
        env.post("/blogs/diario/invitations", Some(anna), json!({"username": "bruno", "role": "mediatore"})).await;
    assert_eq!(inv2["id"], inv["id"], "stessa riga riportata a pending");
    assert_eq!(inv2["role"], "mediatore");
    let (s, dec) = env.post(&format!("/blogs/received-invitations/{inv_id}/decline"), Some(bruno), json!({})).await;
    assert_eq!((s, dec["status"].as_str()), (StatusCode::OK, Some("declined")));
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM blog_invitations WHERE blog_id = $1")
        .bind(blog)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(rows, 1, "una sola riga per (blog, invitato)");

    // revoca: solo se in attesa
    env.post("/blogs/diario/invitations", Some(anna), json!({"username": "carla", "role": "co_autore"})).await;
    let carla_inv = env.get("/blogs/received-invitations", Some(carla)).await.1[0]["id"].as_str().unwrap().to_string();
    assert_eq!(env.delete(&format!("/blogs/diario/invitations/{carla_inv}"), Some(anna)).await, StatusCode::NO_CONTENT);
    assert_eq!(
        env.delete(&format!("/blogs/diario/invitations/{carla_inv}"), Some(anna)).await,
        StatusCode::NO_CONTENT,
        "idempotente"
    );
    assert_eq!(
        env.delete(&format!("/blogs/diario/invitations/{}", Uuid::new_v4()), Some(anna)).await,
        StatusCode::NOT_FOUND
    );
    let (s, b) = env.post(&format!("/blogs/received-invitations/{carla_inv}/accept"), Some(carla), json!({})).await;
    assert_eq!((s, detail(&b)), (StatusCode::CONFLICT, "Invito già gestito."), "un invito revocato non si accetta");
    let (_, all) = env.get("/blogs/diario/invitations", Some(anna)).await;
    assert_eq!(all.as_array().unwrap().len(), 2);

    assert_eq!(
        audit_actions(&env, blog).await,
        [
            "blog.invitation_created",
            "blog.invitation_accepted",
            "blog.member_role_changed",
            "blog.member_removed",
            "blog.invitation_created",
            "blog.invitation_declined",
            "blog.invitation_created",
            "blog.invitation_revoked",
        ]
    );
    env.teardown().await;
}

#[tokio::test]
async fn concurrent_accepts_create_one_membership() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    env.blog("diario", anna, "public").await;
    let (_, inv) =
        env.post("/blogs/diario/invitations", Some(anna), json!({"username": "bruno", "role": "co_autore"})).await;
    let path = format!("/blogs/received-invitations/{}/accept", inv["id"].as_str().unwrap());
    let results = futures::future::join_all((0..4).map(|_| env.post(&path, Some(bruno), json!({})))).await;
    assert_eq!(results.iter().filter(|(s, _)| *s == StatusCode::OK).count(), 1, "{results:?}");
    assert!(results.iter().filter(|(s, _)| *s != StatusCode::OK).all(|(s, _)| *s == StatusCode::CONFLICT));
    env.teardown().await;
}

// ---- ciclo di vita ------------------------------------------------------------------------------------------

#[tokio::test]
async fn delete_restore_and_transfer() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    let carla = env.user("carla", "utente").await;
    let blog = env.blog("diario", anna, "public").await;
    env.member(blog, bruno, "co_autore").await;
    env.member(blog, carla, "mediatore").await;

    // cancellazione: serve lo slug; solo il proprietario
    let (s, b) = env.delete_json("/blogs/diario", Some(anna), json!({"confirm_slug": "sbagliato"})).await;
    assert_eq!((s, detail(&b)), (StatusCode::BAD_REQUEST, "Lo slug digitato non corrisponde."));
    let (s, _) = env.delete_json("/blogs/diario", Some(bruno), json!({"confirm_slug": "diario"})).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, b) = env.delete_json("/blogs/diario", Some(anna), json!({"confirm_slug": " diario "})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert!(b["deleted_at"].is_string());
    let first = b["deleted_at"].clone();
    // idempotente: la data non cambia e non si duplica l'audit
    let (_, b) = env.delete_json("/blogs/diario", Some(anna), json!({"confirm_slug": "diario"})).await;
    assert_eq!(b["deleted_at"], first);
    assert_eq!(env.get("/blogs/diario", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/mine", Some(anna)).await.1[0]["deleted_at"], first, "resta in elenco al proprietario");

    assert_eq!(env.post("/blogs/diario/restore", Some(bruno), json!({})).await.0, StatusCode::FORBIDDEN);
    let (s, b) = env.post("/blogs/diario/restore", Some(anna), json!({})).await;
    assert_eq!((s, b["deleted_at"].clone()), (StatusCode::OK, Value::Null));
    assert_eq!(env.post("/blogs/diario/restore", Some(anna), json!({})).await.0, StatusCode::OK, "no-op");
    assert_eq!(env.get("/blogs/diario", None).await.0, StatusCode::OK);

    // trasferimento: solo a un coautore attivo
    let (s, b) = env.post("/blogs/diario/transfer", Some(anna), json!({"username": "carla"})).await;
    assert_eq!((s, detail(&b)), (StatusCode::BAD_REQUEST, "Il nuovo proprietario deve essere un coautore del blog."));
    assert_eq!(
        env.post("/blogs/diario/transfer", Some(anna), json!({"username": "nessuno"})).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        env.post("/blogs/diario/transfer", Some(anna), json!({"username": "anna"})).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        env.post("/blogs/diario/transfer", Some(bruno), json!({"username": "bruno"})).await.0,
        StatusCode::FORBIDDEN
    );
    for i in 0..5 {
        env.blog(&format!("di-bruno-{i}"), bruno, "public").await;
    }
    let (s, b) = env.post("/blogs/diario/transfer", Some(anna), json!({"username": "bruno"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert!(detail(&b).starts_with("Il nuovo proprietario: Limite massimo"));
    sqlx::query("DELETE FROM blogs WHERE slug = 'di-bruno-0'").execute(&env.pool).await.unwrap();

    let (s, b) = env.post("/blogs/diario/transfer", Some(anna), json!({"username": "bruno"})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["owner_id"], Value::Null, "anna non è più la proprietaria");
    let (owner, old_owner_role): (Uuid, String) = (
        sqlx::query_scalar("SELECT owner_id FROM blogs WHERE id = $1").bind(blog).fetch_one(&env.pool).await.unwrap(),
        sqlx::query_scalar("SELECT role::text FROM blog_memberships WHERE blog_id = $1 AND user_id = $2")
            .bind(blog)
            .bind(anna)
            .fetch_one(&env.pool)
            .await
            .unwrap(),
    );
    assert_eq!((owner, old_owner_role.as_str()), (bruno, "co_autore"), "chi cede resta coautore");
    let has_membership: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM blog_memberships WHERE blog_id = $1 AND user_id = $2)")
            .bind(blog)
            .bind(bruno)
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert!(!has_membership, "il nuovo proprietario non ha più una membership");
    assert_eq!(env.patch("/blogs/diario", Some(anna), json!({"title": "x"})).await.0, StatusCode::FORBIDDEN);
    assert_eq!(env.patch("/blogs/diario", Some(bruno), json!({"title": "Del nuovo"})).await.0, StatusCode::OK);

    assert_eq!(audit_actions(&env, blog).await, ["blog.deleted", "blog.restored", "blog.ownership_transferred"]);
    env.teardown().await;
}

// ---- configurazione di presentazione ------------------------------------------------------------------------

#[tokio::test]
async fn blog_config_defaults_validation_and_visibility() {
    let env = setup(&[], &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    env.blog("diario", anna, "public").await;
    env.blog("riservato", anna, "private").await;

    // default della piattaforma finché il proprietario non salva nulla
    let (s, cfg) = env.get("/blogs/diario/config", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(cfg["typography"]["heading_font"], "Lora");
    assert_eq!(cfg["layout"], "standard");
    assert_eq!(cfg["palette"].as_object().unwrap().len(), 5);

    let put = |who: Option<Uuid>, body: Value| env.put("/blogs/diario/config", who, body);
    assert_eq!(put(None, json!({})).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(put(Some(bruno), json!({})).await.0, StatusCode::FORBIDDEN);
    for (body, needle) in [
        (json!({"palette": {"a": "#ff0000"}}), "troppo acceso"),
        (json!({"typography": {"heading_font": "Inter"}}), "serif"),
        (json!({"footer": {"column3": "x"}}), "column3"),
    ] {
        let (s, b) = put(Some(anna), body.clone()).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body}: {b}");
        assert!(detail(&b).contains(needle), "{b}");
    }
    let saved = json!({"palette": {"primary": "#3e6259"}, "layout": "magazine", "chiave_libera": {"x": [1, 2]}});
    let (s, b) = put(Some(anna), saved.clone()).await;
    assert_eq!((s, &b), (StatusCode::OK, &saved));
    assert_eq!(env.get("/blogs/diario/config", None).await.1, saved, "conservato com'è, chiavi sconosciute comprese");
    // sostituzione, non merge
    let (_, b) = put(Some(anna), json!({"layout": "standard"})).await;
    assert_eq!(b, json!({"layout": "standard"}));
    assert_eq!(env.get("/blogs/diario/config", None).await.1, json!({"layout": "standard"}));

    // segue la visibilità del blog
    assert_eq!(env.get("/blogs/riservato/config", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/riservato/config", Some(anna)).await.0, StatusCode::OK);
    env.teardown().await;
}
