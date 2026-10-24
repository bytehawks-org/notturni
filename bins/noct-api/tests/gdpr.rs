//! Diritti GDPR: esportazione, cancellazione per anonimizzazione e coda delle richieste.

mod common;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use common::*;
use serde_json::{Value, json};
use uuid::Uuid;

const MODULES: &[&str] =
    &["gdpr", "discovery", "fragments", "comments", "newsletter", "custom_domains", "auth_mfa", "auth_sso", "reports"];
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000";

async fn upload_avatar(env: &Env, who: Uuid) -> String {
    let mut body =
        b"--XBOUNDARYX\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a\"\r\nContent-Type: image/png\r\n\r\n".to_vec();
    body.extend_from_slice(PNG);
    body.extend_from_slice(b"\r\n--XBOUNDARYX--\r\n");
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/users/me/avatar")
        .header(header::CONTENT_TYPE, "multipart/form-data; boundary=XBOUNDARYX")
        .header(header::AUTHORIZATION, format!("Bearer {}", env.access_token(who)))
        .body(Body::from(body))
        .unwrap();
    let (s, _, j) = env.send(req).await;
    assert_eq!(s, StatusCode::OK, "{j}");
    j["avatar_url"].as_str().unwrap().to_string()
}

fn on_disk(env: &Env, url: &str) -> std::path::PathBuf {
    env.storage_dir.join(url.strip_prefix("http://localhost:8000/storage/").expect("URL del backend locale"))
}

async fn count(env: &Env, sql: &str, who: Uuid) -> i64 {
    sqlx::query_scalar(sql).bind(who).fetch_one(&env.pool).await.unwrap()
}

/// Un utente con dati in quasi ogni modulo attivo, e un altro che interagisce con lui.
struct World {
    env: Env,
    anna: Uuid,
    bruno: Uuid,
    avatar: String,
    post: String,
}

async fn world() -> World {
    let env = setup(MODULES, &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    let blog_anna = env.blog("diario", anna, "public").await;
    let blog_bruno = env.blog("altrui", bruno, "public").await;
    env.member(blog_bruno, anna, "autore").await;
    // contenuti condivisi: un post e un commento di anna
    let (_, p) = env
        .post("/blogs/diario/posts", Some(anna), json!({"slug": "uno", "title": "Uno", "content": "testo di anna"}))
        .await;
    let post = p["id"].as_str().unwrap().to_string();
    env.post(&format!("/posts/{post}/publish"), Some(anna), json!({})).await;
    let (s, c) = env.post(&format!("/posts/{post}/comments"), Some(anna), json!({"content": "il mio commento"})).await;
    assert_eq!(s, StatusCode::CREATED, "{c}");
    // profilo
    env.patch(
        "/users/me",
        Some(anna),
        json!({"bio": "scrivo di notte", "first_name": "Anna", "country": "it", "post_author_name_style": "full_name"}),
    )
    .await;
    env.post("/users/me/social-links", Some(anna), json!({"label": "sito", "url": "https://example.com/anna"})).await;
    let avatar = upload_avatar(&env, anna).await;
    // follow in entrambe le direzioni e di un blog
    env.post("/users/bruno/follow", Some(anna), json!({})).await;
    env.post("/users/anna/follow", Some(bruno), json!({})).await;
    env.post("/blogs/altrui/follow", Some(anna), json!({})).await;
    // dati inseriti direttamente
    sqlx::query("INSERT INTO user_sessions (user_id, refresh_token_hash, expires_at) VALUES ($1, 'hash', now() + interval '1 day')").bind(anna).execute(&env.pool).await.unwrap();
    sqlx::query("INSERT INTO api_tokens (name, owner_type, user_id, token_prefix, token_hash) VALUES ('cli', 'user', $1, 'noct_ab', 'tokenhash')").bind(anna).execute(&env.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO post_fragments (user_id, post_id, text, text_hash) SELECT $1, id, 'testo', 'h' FROM posts",
    )
    .bind(anna)
    .execute(&env.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO newsletter_subscribers (list_key, email, user_id, status) VALUES ('platform', 'anna@example.com', $1, 'confirmed')").bind(anna).execute(&env.pool).await.unwrap();
    sqlx::query("INSERT INTO mfa_settings (user_id, method, totp_secret) VALUES ($1, 'totp', 'SEGRETO')")
        .bind(anna)
        .execute(&env.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO mfa_email_codes (user_id, expires_at) VALUES ($1, now() + interval '5 minutes')")
        .bind(anna)
        .execute(&env.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET mfa_enabled = true WHERE id = $1").bind(anna).execute(&env.pool).await.unwrap();
    sqlx::query("INSERT INTO sso_identities (user_id, provider, provider_user_id, email) VALUES ($1, 'google', 'g-1', 'anna@gmail.example')").bind(anna).execute(&env.pool).await.unwrap();
    sqlx::query("INSERT INTO custom_domains (user_id, domain, verification_token, status, verified_at) VALUES ($1, 'anna.example.org', 'segretissimo', 'verified', now())").bind(anna).execute(&env.pool).await.unwrap();
    sqlx::query("UPDATE users SET verified_domain = 'anna.example.org' WHERE id = $1")
        .bind(anna)
        .execute(&env.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO blog_blocked_authors (blog_id, user_id, label) SELECT id, $1, 'anna' FROM blogs WHERE slug = 'altrui'").bind(anna).execute(&env.pool).await.unwrap();
    env.post("/blogs/altrui/report", Some(anna), json!({"reason": "spam"})).await;
    let _ = blog_anna;
    World { env, anna, bruno, avatar, post }
}

// ---- esportazione ---------------------------------------------------------------------------------------------------

#[tokio::test]
async fn export_collects_every_active_module_without_secrets() {
    let w = world().await;
    let env = &w.env;
    assert_eq!(env.get("/users/me/export-data", None).await.0, StatusCode::UNAUTHORIZED);
    let (s, data) = env.get("/users/me/export-data", Some(w.anna)).await;
    assert_eq!(s, StatusCode::OK, "{data}");

    // account: core + profilo + dominio
    let a = &data["account"];
    assert_eq!(
        (a["username"].as_str(), a["email"].as_str(), a["platform_role"].as_str()),
        (Some("anna"), Some("anna@example.com"), Some("utente"))
    );
    assert_eq!(
        (a["bio"].as_str(), a["country"].as_str(), a["mfa_enabled"].as_bool()),
        (Some("scrivo di notte"), Some("IT"), Some(true))
    );
    assert_eq!(a["custom_domain_claim"]["domain"], "anna.example.org");
    // sezioni dei moduli
    assert_eq!(data["social_links"].as_array().unwrap().len(), 1);
    assert_eq!(data["blogs_owned"][0]["slug"], "diario");
    assert_eq!(data["memberships"][0], json!({"blog_slug": "altrui", "role": "autore"}));
    assert_eq!(data["posts_authored"][0]["content"], "testo di anna");
    assert_eq!(data["comments_authored"][0]["content"], "il mio commento");
    assert_eq!(data["fragments"].as_array().unwrap().len(), 1);
    assert_eq!(data["follows"]["following_user_ids"].as_array().unwrap().len(), 1);
    assert_eq!(data["follows"]["follower_user_ids"].as_array().unwrap().len(), 1);
    assert_eq!(data["follows"]["blogs_followed_ids"].as_array().unwrap().len(), 1);
    assert_eq!(data["api_tokens"][0]["token_prefix"], "noct_ab");
    assert_eq!(data["newsletter_subscriptions"][0]["email"], "anna@example.com");
    assert_eq!(data["sso_identities"][0]["provider"], "google");
    assert_eq!(data["reports_made"][0]["reason"], "spam");
    assert!(
        data["audit_log"].as_array().unwrap().iter().any(|e| e["action"] == "user.data_exported")
            || data["audit_log"].is_array()
    );

    // nessun segreto: né hash dei token, né segreto TOTP, né token di verifica del dominio
    let text = data.to_string();
    for secret in ["tokenhash", "SEGRETO", "segretissimo", "hashed_password"] {
        assert!(!text.contains(secret), "{secret} nell'esportazione");
    }
    // la richiesta è registrata (self-service, già completata) e l'esportazione è nell'audit
    let (kind, status, note): (String, String, Option<String>) =
        sqlx::query_as("SELECT type::text, status::text, note FROM gdpr_requests").fetch_one(&env.pool).await.unwrap();
    assert_eq!((kind.as_str(), status.as_str(), note.as_deref()), ("export", "completed", Some("self-service")));
    assert_eq!(
        count(env, "SELECT count(*) FROM audit_log WHERE action = 'user.data_exported' AND actor_id = $1", w.anna)
            .await,
        1
    );
    // gli altri non vedono i dati di anna
    let (_, other) = env.get("/users/me/export-data", Some(w.bruno)).await;
    assert_eq!(other["account"]["username"], "bruno");
    assert!(!other.to_string().contains("testo di anna"));
    w.env.teardown().await;
}

#[tokio::test]
async fn export_leaves_out_the_sections_of_inactive_modules() {
    let env = setup(&["gdpr"], &[]).await;
    let anna = env.user("anna", "utente").await;
    let (s, data) = env.get("/users/me/export-data", Some(anna)).await;
    assert_eq!(s, StatusCode::OK, "{data}");
    for absent in
        ["fragments", "follows", "newsletter_subscriptions", "comments_authored", "sso_identities", "reports_made"]
    {
        assert!(data.get(absent).is_none(), "{absent}");
    }
    for present in ["account", "blogs_owned", "posts_authored", "api_tokens", "audit_log", "social_links"] {
        assert!(data.get(present).is_some(), "{present}");
    }
    env.teardown().await;
}

// ---- cancellazione ----------------------------------------------------------------------------------------------------

#[tokio::test]
async fn deleting_the_account_erases_personal_data_and_anonymizes_shared_content() {
    let w = world().await;
    let env = &w.env;
    let token = env.access_token(w.anna);

    // la conferma deve essere lo username esatto
    for wrong in ["", "Anna", "bruno"] {
        let (s, b) = env.delete_json("/users/me", Some(w.anna), json!({"confirm_username": wrong})).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{wrong}: {b}");
    }
    assert_eq!(
        env.delete_json("/users/me", None, json!({"confirm_username": "anna"})).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        count(env, "SELECT count(*) FROM users WHERE id = $1 AND is_active", w.anna).await,
        1,
        "nulla è cambiato"
    );

    assert!(on_disk(env, &w.avatar).exists());
    let (s, b) = env.delete_json("/users/me", Some(w.anna), json!({"confirm_username": "anna"})).await;
    assert_eq!((s, b), (StatusCode::NO_CONTENT, Value::Null));

    // la riga resta ma è anonima e inattiva; le credenziali sono invalidate
    type Anonymized =
        (String, String, Option<String>, String, bool, bool, Option<String>, Option<String>, Option<String>);
    let (username, email, name, role, active, mfa, bio, verified, avatar): Anonymized =
        sqlx::query_as("SELECT username, email, display_name, platform_role::text, is_active, mfa_enabled, bio, verified_domain, avatar_object_key FROM users WHERE id = $1")
            .bind(w.anna)
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert!(username.starts_with("utente-eliminato-") && username.len() <= 32, "{username}");
    assert!(email.starts_with("deleted-") && email.ends_with("@notturni.invalid"), "{email}");
    assert_eq!(
        (name.as_deref(), role.as_str(), active, mfa, bio, verified, avatar),
        (Some("Utente eliminato"), "utente", false, false, None, None, None)
    );
    let pw: Option<String> = sqlx::query_scalar("SELECT hashed_password FROM users WHERE id = $1")
        .bind(w.anna)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert!(pw.is_none());
    let (s, _, _) = env.call(Method::GET, "/users/me", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let req = Request::builder()
        .uri("/api/v1/users/me")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(env.send(req).await.0, StatusCode::UNAUTHORIZED, "il vecchio access token non vale più");

    // i dati personali sono spariti in ogni modulo
    for (table, col) in [
        ("user_sessions", "user_id"),
        ("api_tokens", "user_id"),
        ("social_links", "user_id"),
        ("post_fragments", "user_id"),
        ("newsletter_subscribers", "user_id"),
        ("mfa_settings", "user_id"),
        ("mfa_email_codes", "user_id"),
        ("sso_identities", "user_id"),
        ("custom_domains", "user_id"),
        ("blog_memberships", "user_id"),
        ("user_follows", "follower_id"),
        ("user_follows", "followed_user_id"),
        ("blog_follows", "follower_id"),
    ] {
        let n = count(env, &format!("SELECT count(*) FROM {table} WHERE {col} = $1"), w.anna).await;
        assert_eq!(n, 0, "{table}.{col}");
    }
    assert!(!on_disk(env, &w.avatar).exists(), "l'avatar è stato cancellato dallo storage");
    let label: String = sqlx::query_scalar("SELECT label FROM blog_blocked_authors WHERE user_id = $1")
        .bind(w.anna)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(label, "Utente eliminato");

    // i contenuti condivisi restano, con l'autore anonimizzato
    let (s, post) = env.get("/blogs/diario/posts/uno", None).await;
    assert_eq!((s, post["author_display_name"].as_str()), (StatusCode::OK, Some("Utente eliminato")), "{post}");
    let (_, comments) = env.get(&format!("/posts/{}/comments", w.post), None).await;
    assert_eq!(
        (comments[0]["content"].as_str(), comments[0]["author_display_name"].as_str()),
        (Some("il mio commento"), Some("Utente eliminato")),
        "{comments}"
    );
    // il vecchio username e il dominio non puntano più a nessuno
    assert_eq!(env.get("/users/anna", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/users/anna.example.org", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        env.get(&format!("/users/{username}"), None).await.0,
        StatusCode::NOT_FOUND,
        "un account disattivato non ha profilo"
    );
    let (_, dir) = env.get("/users", None).await;
    assert!(dir.as_array().unwrap().iter().all(|u| u["username"] != json!(username)));
    assert_eq!(
        count(env, "SELECT count(*) FROM blogs WHERE owner_id = $1", w.anna).await,
        1,
        "il blog di proprietà resta"
    );
    assert_eq!(
        count(env, "SELECT count(*) FROM content_reports WHERE reporter_id = $1", w.anna).await,
        1,
        "le segnalazioni restano ai moderatori"
    );

    // traccia: richiesta con lo username di allora, completata; evento di audit
    let (user, kind, status, note): (String, String, String, Option<String>) =
        sqlx::query_as("SELECT username, type::text, status::text, note FROM gdpr_requests WHERE type = 'deletion'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert_eq!(
        (user.as_str(), kind.as_str(), status.as_str(), note.as_deref()),
        ("anna", "deletion", "completed", Some("self-service"))
    );
    assert_eq!(
        count(env, "SELECT count(*) FROM audit_log WHERE action = 'user.account_deleted' AND actor_id = $1", w.anna)
            .await,
        1
    );
    // gli altri non sono toccati
    assert_eq!(count(env, "SELECT count(*) FROM users WHERE id = $1 AND is_active", w.bruno).await, 1);
    // il nome si può riusare
    assert_eq!(count(env, "SELECT count(*) FROM users WHERE username = 'anna'", w.anna).await, 0);
    w.env.teardown().await;
}

#[tokio::test]
async fn a_super_admin_cannot_delete_their_own_account() {
    let env = setup(&["gdpr"], &[]).await;
    let root = env.user("root", "super_admin").await;
    let (s, b) = env.delete_json("/users/me", Some(root), json!({"confirm_username": "root"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(count(&env, "SELECT count(*) FROM users WHERE id = $1 AND is_active", root).await, 1);
    env.teardown().await;
}

// ---- coda delle richieste -----------------------------------------------------------------------------------------------

#[tokio::test]
async fn admin_queue_needs_a_second_admin_for_deletions() {
    let env = setup(&["gdpr", "fragments"], &[]).await;
    let anna = env.user("anna", "utente").await;
    let root = env.user("root", "super_admin").await;
    let adm = env.user("adm", "amministratore").await;
    let mod_ = env.user("mod", "moderatore").await;

    // permessi
    for who in [Some(anna), Some(mod_), None] {
        assert_eq!(
            env.get("/admin/gdpr", who).await.0,
            if who.is_some() { StatusCode::FORBIDDEN } else { StatusCode::UNAUTHORIZED }
        );
    }
    // creazione: nota obbligatoria, utente esistente e attivo
    let create = |who, username: &str, kind: &str, note: &str| {
        env.post("/admin/gdpr", Some(who), json!({"username": username, "type": kind, "note": note}))
    };
    assert_eq!(create(root, "anna", "deletion", "  ").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(create(root, "nessuno", "deletion", "PEC del 3/5").await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        env.post("/admin/gdpr", Some(root), json!({"username": "anna", "type": "altro", "note": "x"})).await.0,
        StatusCode::BAD_REQUEST
    );
    let (s, req) = create(root, "anna", "deletion", "PEC del 3/5").await;
    assert_eq!(s, StatusCode::CREATED, "{req}");
    assert_eq!(
        (req["status"].as_str(), req["created_by_username"].as_str(), req["username"].as_str()),
        (Some("open"), Some("root"), Some("anna"))
    );
    let id = req["id"].as_str().unwrap().to_string();
    let deadline = chrono::DateTime::parse_from_rfc3339(req["deadline_at"].as_str().unwrap()).unwrap();
    assert!((deadline.to_utc() - chrono::Utc::now()).num_days() >= 29, "scadenza a 30 giorni");

    // la cancellazione non parte senza approvazione, né la approva chi l'ha inserita
    let exec = format!("/admin/gdpr/{id}/execute");
    assert_eq!(env.post(&exec, Some(root), json!({})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(env.post(&format!("/admin/gdpr/{id}/approve"), Some(root), json!({})).await.0, StatusCode::FORBIDDEN);
    let (s, ok) = env.post(&format!("/admin/gdpr/{id}/approve"), Some(adm), json!({})).await;
    assert_eq!(
        (s, ok["status"].as_str(), ok["approved_by_username"].as_str()),
        (StatusCode::OK, Some("approved"), Some("adm")),
        "{ok}"
    );
    assert_eq!(
        env.post(&format!("/admin/gdpr/{id}/approve"), Some(adm), json!({})).await.0,
        StatusCode::BAD_REQUEST,
        "non più aperta"
    );
    assert_eq!(
        env.post(&format!("/admin/gdpr/{id}/reject"), Some(adm), json!({"note": " "})).await.0,
        StatusCode::BAD_REQUEST
    );

    // elenco: aperte e approvate prima; filtro per stato
    let (_, e) =
        env.post("/admin/gdpr", Some(adm), json!({"username": "anna", "type": "export", "note": "email"})).await;
    let (_, all) = env.get("/admin/gdpr", Some(root)).await;
    assert_eq!(all.as_array().unwrap().len(), 2);
    assert_eq!(all[0]["status"], "open", "{all}");
    assert_eq!(env.get("/admin/gdpr?status=approved", Some(root)).await.1.as_array().unwrap().len(), 1);
    assert_eq!(env.get("/admin/gdpr?status=boh", Some(root)).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    // esecuzione dell'export: restituisce i dati e chiude la richiesta, una sola volta
    let eid = e["id"].as_str().unwrap();
    let (s, data) = env.post(&format!("/admin/gdpr/{eid}/execute"), Some(root), json!({})).await;
    assert_eq!((s, data["account"]["username"].as_str()), (StatusCode::OK, Some("anna")), "{data}");
    assert_eq!(env.post(&format!("/admin/gdpr/{eid}/execute"), Some(root), json!({})).await.0, StatusCode::BAD_REQUEST);

    // esecuzione della cancellazione approvata
    let (s, done) = env.post(&exec, Some(root), json!({})).await;
    assert_eq!((s, done), (StatusCode::OK, json!({"status": "completed"})));
    assert_eq!(count(&env, "SELECT count(*) FROM users WHERE id = $1 AND NOT is_active", anna).await, 1);
    assert_eq!(env.post(&exec, Some(root), json!({})).await.0, StatusCode::BAD_REQUEST, "già chiusa");
    // l'account non c'è più: niente nuove richieste a suo nome
    assert_eq!(create(root, "anna", "export", "x").await.0, StatusCode::NOT_FOUND);
    let actions: Vec<String> =
        sqlx::query_scalar("SELECT action FROM audit_log WHERE action LIKE 'gdpr.%' ORDER BY occurred_at, id")
            .fetch_all(&env.pool)
            .await
            .unwrap();
    for expected in ["gdpr.request_created", "gdpr.request_approved", "gdpr.export_executed", "gdpr.deletion_executed"]
    {
        assert!(actions.iter().any(|a| a == expected), "{expected} in {actions:?}");
    }
    let note: String = sqlx::query_scalar("SELECT note FROM gdpr_requests WHERE id = $1::uuid")
        .bind(&id)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(note, "PEC del 3/5");
    env.teardown().await;
}

#[tokio::test]
async fn rejecting_closes_the_request_with_a_reason_and_super_admins_cannot_be_deleted_by_a_request() {
    let env = setup(&["gdpr"], &[]).await;
    let anna = env.user("anna", "utente").await;
    let root = env.user("root", "super_admin").await;
    let adm = env.user("adm", "amministratore").await;
    let (_, req) = env
        .post("/admin/gdpr", Some(adm), json!({"username": "anna", "type": "deletion", "note": "richiesta telefonica"}))
        .await;
    let id = req["id"].as_str().unwrap();
    let (s, r) =
        env.post(&format!("/admin/gdpr/{id}/reject"), Some(root), json!({"note": "identità non verificata"})).await;
    assert_eq!((s, r["status"].as_str()), (StatusCode::OK, Some("rejected")), "{r}");
    assert!(
        r["note"].as_str().unwrap().contains("richiesta telefonica")
            && r["note"].as_str().unwrap().contains("[rifiutata] identità non verificata"),
        "{r}"
    );
    assert!(r["completed_at"].is_string());
    assert_eq!(
        env.post(&format!("/admin/gdpr/{id}/reject"), Some(root), json!({"note": "ancora"})).await.0,
        StatusCode::BAD_REQUEST,
        "già chiusa"
    );
    assert_eq!(env.post(&format!("/admin/gdpr/{id}/execute"), Some(root), json!({})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(count(&env, "SELECT count(*) FROM users WHERE id = $1 AND is_active", anna).await, 1);

    // un Super Admin non si cancella con una richiesta
    let (_, req) =
        env.post("/admin/gdpr", Some(adm), json!({"username": "root", "type": "deletion", "note": "test"})).await;
    let id = req["id"].as_str().unwrap();
    env.post(&format!("/admin/gdpr/{id}/approve"), Some(root), json!({})).await;
    let (s, b) = env.post(&format!("/admin/gdpr/{id}/execute"), Some(root), json!({})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(count(&env, "SELECT count(*) FROM users WHERE id = $1 AND is_active", root).await, 1);
    assert_eq!(
        env.post(&format!("/admin/gdpr/{}/approve", Uuid::new_v4()), Some(root), json!({})).await.0,
        StatusCode::NOT_FOUND
    );
    env.teardown().await;
}
