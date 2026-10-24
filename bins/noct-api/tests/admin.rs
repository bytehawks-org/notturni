//! Amministrazione di piattaforma: panoramica, utenti, blog, post, audit, impostazioni e le rotte
//! di amministrazione dei moduli (segnalazioni, commenti).

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use uuid::Uuid;

const MODULES: &[&str] = &["comments", "reports", "interests", "auth_sso", "auth_mfa", "gdpr"];

struct Cast {
    env: Env,
    root: Uuid,
    admin: Uuid,
    moderator: Uuid,
    anna: Uuid,
}

async fn cast() -> Cast {
    let env = setup(MODULES, &[]).await;
    let root = env.user("root", "super_admin").await;
    let admin = env.user("capo", "amministratore").await;
    let moderator = env.user("mod", "moderatore").await;
    let anna = env.user("anna", "utente").await;
    Cast { env, root, admin, moderator, anna }
}

async fn actions(env: &Env, action: &str) -> Vec<Value> {
    sqlx::query_scalar("SELECT payload FROM audit_log WHERE action = $1 ORDER BY occurred_at")
        .bind(action)
        .fetch_all(&env.pool)
        .await
        .unwrap()
}

async fn published(env: &Env, who: Uuid, blog: &str, slug: &str) -> String {
    let (s, p) = env
        .post(&format!("/blogs/{blog}/posts"), Some(who), json!({"slug": slug, "title": slug, "content": "testo"}))
        .await;
    assert_eq!(s, StatusCode::CREATED, "{p}");
    let id = p["id"].as_str().unwrap().to_string();
    env.post(&format!("/posts/{id}/publish"), Some(who), json!({})).await;
    id
}

#[tokio::test]
async fn admin_routes_follow_the_platform_role() {
    let c = cast().await;
    let e = &c.env;
    for path in ["/admin/users", "/admin/blogs", "/admin/posts", "/admin/audit-log"] {
        assert_eq!(e.get(path, None).await.0, StatusCode::UNAUTHORIZED, "{path}");
        assert_eq!(e.get(path, Some(c.anna)).await.0, StatusCode::FORBIDDEN, "{path}");
        assert_eq!(e.get(path, Some(c.moderator)).await.0, StatusCode::FORBIDDEN, "{path}");
        assert_eq!(e.get(path, Some(c.admin)).await.0, StatusCode::OK, "{path}");
    }
    // panoramica e commenti trasversali: anche il moderatore
    for path in ["/admin/overview", "/admin/comments"] {
        assert_eq!(e.get(path, Some(c.moderator)).await.0, StatusCode::OK, "{path}");
        assert_eq!(e.get(path, Some(c.anna)).await.0, StatusCode::FORBIDDEN, "{path}");
    }
    // le impostazioni solo al Super Admin
    assert_eq!(e.get("/admin/config", Some(c.admin)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(e.get("/admin/config", Some(c.root)).await.0, StatusCode::OK);
    c.env.teardown().await;
}

#[tokio::test]
async fn overview_counts_content_queues_and_services() {
    let c = cast().await;
    let e = &c.env;
    e.blog("diario", c.anna, "public").await;
    let post = published(e, c.anna, "diario", "uno").await;
    // un commento anonimo in attesa non c'è senza captcha: lo inserisco direttamente
    sqlx::query("INSERT INTO comments (post_id, author_display_name, content, status) VALUES ($1::uuid, 'x', 'ciao', 'pending')")
        .bind(&post)
        .execute(&e.pool)
        .await
        .unwrap();
    let (s, r) = e.post(&format!("/posts/{post}/report"), Some(c.admin), json!({"reason": "spam"})).await;
    assert_eq!(s, StatusCode::CREATED, "{r}");

    let (s, o) = e.get("/admin/overview", Some(c.moderator)).await;
    assert_eq!(s, StatusCode::OK, "{o}");
    assert_eq!(o["users_total"], 4);
    assert_eq!(o["users_new_7d"], 4);
    assert_eq!(o["blogs_total"], 1);
    assert_eq!(o["posts_published"], 1);
    assert_eq!(o["queue_pending_comments"], 1);
    assert_eq!(o["queue_open_reports"], 1);
    assert_eq!(o["queue_hidden_posts"], 0);
    let services = o["services"].as_array().unwrap();
    assert!(services.iter().any(|s| s["name"] == "postgres" && s["status"] == "ok"), "{services:?}");
    assert!(services.iter().all(|s| s["status"] != "down"), "{services:?}");
    assert!(services.iter().any(|s| s["name"] == "moderation" && s["status"] == "unconfigured"));
    c.env.teardown().await;
}

#[tokio::test]
async fn overview_without_optional_modules_has_zero_queues() {
    let env = setup(&[], &[]).await;
    let root = env.user("root", "super_admin").await;
    let (s, o) = env.get("/admin/overview", Some(root)).await;
    assert_eq!(s, StatusCode::OK, "{o}");
    assert_eq!(o["queue_pending_comments"], 0);
    assert_eq!(o["queue_open_reports"], 0);
    // le impostazioni dei moduli spenti non compaiono
    let (_, cfg) = env.get("/admin/config", Some(root)).await;
    for key in ["interests", "sso_providers", "anonymous_comments_allowed", "verification_gold_identifiers"] {
        assert!(cfg.get(key).is_none() || key == "verification_gold_identifiers", "{key}: {cfg}");
    }
    env.teardown().await;
}

#[tokio::test]
async fn users_list_filters_and_counts() {
    let c = cast().await;
    let e = &c.env;
    e.blog("uno-blog", c.anna, "public").await;
    e.blog("due-blog", c.anna, "public").await;
    sqlx::query("INSERT INTO user_sessions (user_id, refresh_token_hash, expires_at, last_used_at) VALUES ($1, 'h', now() + interval '1 day', now())")
        .bind(c.anna)
        .execute(&e.pool)
        .await
        .unwrap();
    let (s, all) = e.get("/admin/users", Some(c.admin)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(all.as_array().unwrap().len(), 4);
    let (_, found) = e.get("/admin/users?q=ANN", Some(c.admin)).await;
    let found = found.as_array().unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["username"], "anna");
    assert_eq!(found[0]["blogs_count"], 2);
    assert!(found[0]["last_seen_at"].is_string());
    assert!(found[0].get("hashed_password").is_none());
    // `%` è letterale
    let (_, none) = e.get("/admin/users?q=%25", Some(c.admin)).await;
    assert!(none.as_array().unwrap().is_empty());
    c.env.teardown().await;
}

#[tokio::test]
async fn role_and_activation_changes_need_a_note_and_respect_privileges() {
    let c = cast().await;
    let e = &c.env;
    let url = |id: Uuid| format!("/admin/users/{id}");
    // senza nota
    let (s, j) = e.patch(&url(c.anna), Some(c.admin), json!({"platform_role": "moderatore"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{j}");
    // ruolo non valido
    let (s, _) = e.patch(&url(c.anna), Some(c.admin), json!({"platform_role": "dio", "note": "prova"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // un Amministratore promuove a moderatore ma non ad amministratore
    let (s, j) = e
        .patch(&url(c.anna), Some(c.admin), json!({"platform_role": "moderatore", "note": "aiuta con i commenti"}))
        .await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert_eq!(j["platform_role"], "moderatore");
    let (s, _) = e
        .patch(&url(c.anna), Some(c.admin), json!({"platform_role": "amministratore", "note": "non consentito"}))
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    // né toglie un ruolo di amministrazione
    let (s, _) =
        e.patch(&url(c.root), Some(c.admin), json!({"platform_role": "utente", "note": "non consentito"})).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    // il Super Admin sì
    let (s, j) =
        e.patch(&url(c.anna), Some(c.root), json!({"platform_role": "amministratore", "note": "fiducia"})).await;
    assert_eq!(s, StatusCode::OK, "{j}");
    // una patch senza cambiamenti non chiede la nota
    let (s, _) = e.patch(&url(c.anna), Some(c.root), json!({"platform_role": "amministratore"})).await;
    assert_eq!(s, StatusCode::OK);
    let log = actions(e, "user.role_change").await;
    assert_eq!(log.len(), 2);
    assert_eq!(log[0]["from"], "utente");
    assert_eq!(log[0]["note"], "aiuta con i commenti");
    // l'ultimo Super Admin non si retrocede
    let (s, j) = e.patch(&url(c.root), Some(c.root), json!({"platform_role": "utente", "note": "mi dimetto"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{j}");
    c.env.teardown().await;
}

#[tokio::test]
async fn deactivating_revokes_sessions_and_cannot_target_yourself_or_a_peer() {
    let c = cast().await;
    let e = &c.env;
    sqlx::query("INSERT INTO user_sessions (user_id, refresh_token_hash, expires_at) VALUES ($1, 'h', now() + interval '1 day')")
        .bind(c.anna)
        .execute(&e.pool)
        .await
        .unwrap();
    let url = |id: Uuid| format!("/admin/users/{id}");
    let (s, _) = e.patch(&url(c.admin), Some(c.admin), json!({"is_active": false, "note": "prova su me"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = e.patch(&url(c.root), Some(c.admin), json!({"is_active": false, "note": "non consentito"})).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = e.patch(&url(c.anna), Some(c.admin), json!({"is_active": false})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, j) = e.patch(&url(c.anna), Some(c.admin), json!({"is_active": false, "note": "spam ripetuto"})).await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert_eq!(j["is_active"], false);
    let live: i64 = sqlx::query_scalar("SELECT count(*) FROM user_sessions WHERE user_id = $1 AND revoked_at IS NULL")
        .bind(c.anna)
        .fetch_one(&e.pool)
        .await
        .unwrap();
    assert_eq!(live, 0);
    // il token già emesso non vale più
    assert_eq!(e.get("/users/me", Some(c.anna)).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(actions(e, "user.deactivated").await.len(), 1);
    let (s, _) = e.patch(&url(c.anna), Some(c.admin), json!({"is_active": true, "note": "chiarito"})).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(actions(e, "user.activated").await.len(), 1);
    assert_eq!(e.patch(&url(Uuid::new_v4()), Some(c.admin), json!({})).await.0, StatusCode::NOT_FOUND);
    c.env.teardown().await;
}

#[tokio::test]
async fn forced_password_reset_issues_a_code_and_is_audited() {
    let c = cast().await;
    let e = &c.env;
    let url = |id: Uuid| format!("/admin/users/{id}/reset-password");
    let (s, j) = e.post(&url(c.anna), Some(c.admin), json!({})).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{j}");
    let codes: i64 =
        sqlx::query_scalar("SELECT count(*) FROM password_reset_codes WHERE user_id = $1 AND consumed_at IS NULL")
            .bind(c.anna)
            .fetch_one(&e.pool)
            .await
            .unwrap();
    assert_eq!(codes, 1);
    // un secondo reset invalida il primo
    e.post(&url(c.anna), Some(c.admin), json!({})).await;
    let codes: i64 =
        sqlx::query_scalar("SELECT count(*) FROM password_reset_codes WHERE user_id = $1 AND consumed_at IS NULL")
            .bind(c.anna)
            .fetch_one(&e.pool)
            .await
            .unwrap();
    assert_eq!(codes, 1);
    assert_eq!(actions(e, "user.password_reset_triggered").await.len(), 2);
    assert_eq!(e.post(&url(Uuid::new_v4()), Some(c.admin), json!({})).await.0, StatusCode::NOT_FOUND);
    // un Amministratore non reimposta la password di un altro amministratore
    assert_eq!(e.post(&url(c.root), Some(c.admin), json!({})).await.0, StatusCode::FORBIDDEN);
    assert_eq!(e.post(&url(c.admin), Some(c.root), json!({})).await.0, StatusCode::ACCEPTED);
    sqlx::query("UPDATE users SET is_active = FALSE WHERE id = $1").bind(c.anna).execute(&e.pool).await.unwrap();
    assert_eq!(e.post(&url(c.anna), Some(c.admin), json!({})).await.0, StatusCode::BAD_REQUEST);
    c.env.teardown().await;
}

#[tokio::test]
async fn blogs_are_listed_filtered_and_suspended_with_a_note() {
    let c = cast().await;
    let e = &c.env;
    let bad = e.blog("rumoroso", c.anna, "public").await;
    let calm = e.blog("tranquillo", c.anna, "private").await;
    let post = published(e, c.anna, "rumoroso", "uno").await;
    e.post(&format!("/posts/{post}/report"), Some(c.admin), json!({"reason": "spam"})).await;

    let (s, all) = e.get("/admin/blogs", Some(c.admin)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(all.as_array().unwrap().len(), 2);
    let (_, v) = e.get("/admin/blogs?visibility=private", Some(c.admin)).await;
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(e.get("/admin/blogs?visibility=boh", Some(c.admin)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(e.get("/admin/blogs?state=boh", Some(c.admin)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, reported) = e.get("/admin/blogs?state=reported", Some(c.admin)).await;
    let reported = reported.as_array().unwrap();
    assert_eq!(reported.len(), 1);
    assert_eq!(reported[0]["slug"], "rumoroso");
    assert_eq!(reported[0]["reports_open"], 1);
    assert_eq!(reported[0]["posts_count"], 1);
    assert_eq!(reported[0]["owner_username"], "anna");
    let (_, q) = e.get("/admin/blogs?q=tranq", Some(c.admin)).await;
    assert_eq!(q[0]["id"], json!(calm));

    let url = format!("/admin/blogs/{bad}");
    let (s, _) = e.patch(&url, Some(c.admin), json!({"is_suspended": true})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, j) = e.patch(&url, Some(c.admin), json!({"is_suspended": true, "note": "contenuti ripetitivi"})).await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert_eq!(j["is_suspended"], true);
    let (_, susp) = e.get("/admin/blogs?state=suspended", Some(c.admin)).await;
    assert_eq!(susp.as_array().unwrap().len(), 1);
    let (_, active) = e.get("/admin/blogs?state=active", Some(c.admin)).await;
    assert_eq!(active.as_array().unwrap().len(), 1);
    // il dettaglio resta leggibile (spiega lo stato), i contenuti no
    assert_eq!(e.get("/blogs/rumoroso/posts/uno", None).await.0, StatusCode::NOT_FOUND);
    let log = actions(e, "blog.suspended").await;
    assert_eq!(log[0]["note"], "contenuti ripetitivi");
    // stesso stato = niente nota, niente audit
    let (s, _) = e.patch(&url, Some(c.admin), json!({"is_suspended": true})).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(actions(e, "blog.suspended").await.len(), 1);
    e.patch(&url, Some(c.admin), json!({"is_suspended": false, "note": "chiarito"})).await;
    assert_eq!(actions(e, "blog.unsuspended").await.len(), 1);
    assert_eq!(
        e.patch(&format!("/admin/blogs/{}", Uuid::new_v4()), Some(c.admin), json!({"is_suspended": true})).await.0,
        StatusCode::NOT_FOUND
    );
    c.env.teardown().await;
}

#[tokio::test]
async fn posts_are_listed_and_hidden_with_a_note() {
    let c = cast().await;
    let e = &c.env;
    e.blog("diario", c.anna, "public").await;
    let post = published(e, c.anna, "diario", "notte").await;
    published(e, c.anna, "diario", "alba").await;
    e.post(&format!("/posts/{post}/report"), Some(c.root), json!({"reason": "abuse"})).await;

    let (_, all) = e.get("/admin/posts", Some(c.admin)).await;
    assert_eq!(all.as_array().unwrap().len(), 2);
    let (_, found) = e.get("/admin/posts?q=NOTT", Some(c.admin)).await;
    let found = found.as_array().unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["reports_open"], 1);
    assert_eq!(found[0]["author_username"], "anna");
    assert_eq!(found[0]["blog_slug"], "diario");
    assert_eq!(found[0]["status"], "published");

    let url = format!("/admin/posts/{post}");
    assert_eq!(e.patch(&url, Some(c.admin), json!({"is_hidden": true})).await.0, StatusCode::BAD_REQUEST);
    let (s, j) = e.patch(&url, Some(c.admin), json!({"is_hidden": true, "note": "segnalato più volte"})).await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert_eq!(j["is_hidden"], true);
    assert_eq!(e.get("/blogs/diario/posts/notte", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(actions(e, "post.hidden").await[0]["note"], "segnalato più volte");
    let (_, o) = e.get("/admin/overview", Some(c.admin)).await;
    assert_eq!(o["queue_hidden_posts"], 1);
    e.patch(&url, Some(c.admin), json!({"is_hidden": false, "note": "rivisto"})).await;
    assert_eq!(actions(e, "post.unhidden").await.len(), 1);
    c.env.teardown().await;
}

#[tokio::test]
async fn audit_log_filters_and_pages() {
    let c = cast().await;
    let e = &c.env;
    e.blog("diario", c.anna, "public").await;
    for i in 0..3 {
        e.patch(
            &format!("/admin/users/{}", c.anna),
            Some(c.admin),
            json!({"is_active": i % 2 == 1, "note": "giro di prova"}),
        )
        .await;
    }
    sqlx::query("INSERT INTO audit_log (actor_type, action) VALUES ('system', 'system.job')")
        .execute(&e.pool)
        .await
        .unwrap();
    let (s, all) = e.get("/admin/audit-log", Some(c.admin)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(all.as_array().unwrap().len(), 4);
    assert_eq!(all[0]["channel"], "system");
    let (_, by_action) = e.get("/admin/audit-log?action=user.deactivated", Some(c.admin)).await;
    assert_eq!(by_action.as_array().unwrap().len(), 2);
    assert!(by_action[0]["actor_label"].as_str().unwrap().starts_with("capo <"));
    let (_, web) = e.get("/admin/audit-log?channel=web", Some(c.admin)).await;
    assert_eq!(web.as_array().unwrap().len(), 3);
    let (_, sys) = e.get("/admin/audit-log?channel=system", Some(c.admin)).await;
    assert_eq!(sys.as_array().unwrap().len(), 1);
    assert_eq!(e.get("/admin/audit-log?channel=boh", Some(c.admin)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, page) = e.get("/admin/audit-log?limit=2&offset=1", Some(c.admin)).await;
    assert_eq!(page.as_array().unwrap().len(), 2);
    let (_, by_actor) = e.get(&format!("/admin/audit-log?actor_id={}", c.admin), Some(c.admin)).await;
    assert_eq!(by_actor.as_array().unwrap().len(), 3);
    let (_, by_target) = e.get(&format!("/admin/audit-log?target_id={}", c.anna), Some(c.admin)).await;
    assert_eq!(by_target.as_array().unwrap().len(), 3);
    let (_, future) = e.get("/admin/audit-log?since=2999-01-01T00:00:00Z", Some(c.admin)).await;
    assert!(future.as_array().unwrap().is_empty());
    assert_eq!(e.get("/admin/audit-log?since=ieri", Some(c.admin)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(e.get("/admin/audit-log?actor_id=x", Some(c.admin)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    c.env.teardown().await;
}

#[tokio::test]
async fn config_core_fields_are_validated_and_audited_with_old_values() {
    let c = cast().await;
    let e = &c.env;
    let (s, cfg) = e.get("/admin/config", Some(c.root)).await;
    assert_eq!(s, StatusCode::OK, "{cfg}");
    assert_eq!(cfg["registration_mode"], "open");
    assert_eq!(cfg["max_blogs_per_user"], 5);
    assert!(cfg["reserved_builtin"].as_array().unwrap().iter().any(|n| n == "admin"));
    assert_eq!(cfg["infrastructure"]["storage_backend"], "localstorage");
    assert!(cfg["infrastructure"].get("jwt_secret").is_none());

    for (body, why) in [
        (json!({"default_locale": "xx"}), "lingua"),
        (json!({"registration_mode": "boh"}), "modalità"),
        (json!({"moderation_threshold": 0}), "soglia"),
        (json!({"moderation_threshold": 1.5}), "soglia"),
        (json!({"max_blogs_per_user": 0}), "blog"),
        (json!({"max_blogs_per_user": 101}), "blog"),
        (json!({"audit_retention_days": 3}), "audit"),
        (json!({"audit_retention_days": 4000}), "audit"),
        (json!({"reserved_blog_names": ["Va Bene Si"]}), "nomi"),
        (json!({"max_blog_storage_mb": -1}), "spazio"),
        (json!({"footer_column1_markdown": "x".repeat(5001)}), "footer"),
    ] {
        let (s, j) = e.patch("/admin/config", Some(c.root), body).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{why}: {j}");
    }
    // un tipo sbagliato è rifiutato dalla deserializzazione del corpo
    let (s, _) = e.patch("/admin/config", Some(c.root), json!({"max_blogs_per_user": "tanti"})).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(actions(e, "platform.config_updated").await.is_empty());

    let (s, j) = e
        .patch(
            "/admin/config",
            Some(c.root),
            json!({
                "registration_mode": "invite", "max_blogs_per_user": 3, "audit_retention_days": 30,
                "reserved_blog_names": ["  Zeta ", "alfa", "alfa"], "footer_column1_markdown": "**ciao**",
                "max_blog_storage_mb": 500, "moderation_threshold": 0.6, "default_locale": "en",
            }),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert_eq!(j["registration_mode"], "invite");
    assert_eq!(j["reserved_blog_names"], json!(["alfa", "zeta"]));
    assert_eq!(j["footer_column1_markdown"], "**ciao**");
    assert_eq!(j["max_blog_storage_mb"], 500);
    assert!(j["updated_at"].is_string());
    let log = actions(e, "platform.config_updated").await;
    assert_eq!(log.len(), 1);
    assert_eq!(log[0]["changes"]["registration_mode"], json!({"from": "open", "to": "invite"}));
    assert_eq!(log[0]["changes"]["max_blogs_per_user"], json!({"from": 5, "to": 3}));

    // 0 azzera il limite di spazio, "" azzera il footer; lo stesso valore non produce audit
    let (_, j) =
        e.patch("/admin/config", Some(c.root), json!({"max_blog_storage_mb": 0, "footer_column1_markdown": ""})).await;
    assert!(j["max_blog_storage_mb"].is_null());
    assert!(j["footer_column1_markdown"].is_null());
    e.patch("/admin/config", Some(c.root), json!({"registration_mode": "invite"})).await;
    assert_eq!(actions(e, "platform.config_updated").await.len(), 2);

    // la modalità di registrazione cambia davvero il comportamento
    let (s, j) = e
        .post(
            "/auth/register",
            None,
            json!({"username": "nuova", "email": "n@example.com", "password": "Una-password-lunga-1"}),
        )
        .await;
    assert_ne!(s, StatusCode::CREATED, "{j}");
    c.env.teardown().await;
}

#[tokio::test]
async fn config_includes_the_settings_of_active_modules() {
    let c = cast().await;
    let e = &c.env;
    let (_, cfg) = e.get("/admin/config", Some(c.root)).await;
    assert_eq!(cfg["anonymous_comments_allowed"], true);
    assert_eq!(cfg["sso_providers"], json!([]));
    assert_eq!(cfg["sso_configured"], json!([]));
    assert_eq!(cfg["interests"].as_array().unwrap().len(), 16);
    assert_eq!(cfg["verification_blue_domains"], json!([]));

    // interessi: validazione, normalizzazione e pulizia delle scelte già salvate
    for bad in [
        json!([{"key": "Con Spazi", "translations": {"it": "x"}}]),
        json!([{"key": "a", "translations": {"it": "x"}}, {"key": "a", "translations": {"it": "y"}}]),
        json!([{"key": "senza", "translations": {}}]),
    ] {
        let (s, j) = e.patch("/admin/config", Some(c.root), json!({"interests": bad})).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{j}");
    }
    let (s, j) = e
        .patch(
            "/admin/config",
            Some(c.root),
            json!({"interests": [
                {"key": "Musica", "translations": {"IT": " Musica ", "en": "Music"}},
                {"key": "arte", "translations": {"it": "Arte"}},
            ]}),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert_eq!(j["interests"][0], json!({"key": "musica", "translations": {"it": "Musica", "en": "Music"}}));
    let (s, _) = e.patch("/users/me", Some(c.anna), json!({"interests": ["musica", "arte"]})).await;
    assert_eq!(s, StatusCode::OK);
    e.patch("/admin/config", Some(c.root), json!({"interests": [{"key": "arte", "translations": {"it": "Arte"}}]}))
        .await;
    let (_, me) = e.get("/users/me", Some(c.anna)).await;
    assert_eq!(me["interests"], json!(["arte"]));
    let (_, public) = e.get("/interests", None).await;
    assert_eq!(public.as_array().unwrap().len(), 1);

    // provider SSO
    let (s, _) = e.patch("/admin/config", Some(c.root), json!({"sso_providers": ["facebook"]})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (_, j) = e.patch("/admin/config", Some(c.root), json!({"sso_providers": ["github", "google", "github"]})).await;
    assert_eq!(j["sso_providers"], json!(["github", "google"]));

    // commenti anonimi
    let (_, j) = e.patch("/admin/config", Some(c.root), json!({"anonymous_comments_allowed": false})).await;
    assert_eq!(j["anonymous_comments_allowed"], false);

    // sigilli di verifica
    let (s, _) = e.patch("/admin/config", Some(c.root), json!({"verification_blue_domains": ["non un dominio"]})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (_, j) = e
        .patch(
            "/admin/config",
            Some(c.root),
            json!({"verification_gold_identifiers": [" Anna@Example.com "], "verification_blue_domains": ["Example.org"]}),
        )
        .await;
    assert_eq!(j["verification_gold_identifiers"], json!(["anna@example.com"]));
    assert_eq!(j["verification_blue_domains"], json!(["example.org"]));
    let (_, profile) = e.get("/users/anna", None).await;
    assert_eq!(profile["verification_tier"], "gold");
    let log = actions(e, "platform.config_updated").await;
    assert!(log.iter().any(|l| l["changes"].get("sso_providers").is_some()));
    c.env.teardown().await;
}

#[tokio::test]
async fn admins_without_mfa_are_locked_out_when_the_platform_requires_it() {
    let c = cast().await;
    let e = &c.env;
    let (s, _) = e.patch("/admin/config", Some(c.root), json!({"mfa_required_for_admins": true})).await;
    assert_eq!(s, StatusCode::OK);
    let (s, j) = e.get("/admin/users", Some(c.admin)).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{j}");
    assert!(detail(&j).contains("due fattori"));
    // anche il Super Admin senza MFA: si rientra dal database (o si attiva l'MFA prima)
    assert_eq!(e.get("/admin/config", Some(c.root)).await.0, StatusCode::FORBIDDEN);
    // il moderatore non è un amministratore: non è toccato
    assert_eq!(e.get("/admin/comments", Some(c.moderator)).await.0, StatusCode::OK);
    sqlx::query("UPDATE users SET mfa_enabled = TRUE WHERE id = $1").bind(c.admin).execute(&e.pool).await.unwrap();
    assert_eq!(e.get("/admin/users", Some(c.admin)).await.0, StatusCode::OK);
    c.env.teardown().await;
}

#[tokio::test]
async fn blog_reports_panel_and_actions_close_open_reports() {
    let c = cast().await;
    let e = &c.env;
    let bad = e.blog("rumoroso", c.anna, "public").await;
    let post = published(e, c.anna, "rumoroso", "uno").await;
    let other = published(e, c.anna, "rumoroso", "due").await;
    let (s, _) =
        e.post(&format!("/posts/{post}/report"), Some(c.admin), json!({"reason": "spam", "note": "pubblicità"})).await;
    assert_eq!(s, StatusCode::CREATED);
    e.post("/blogs/rumoroso/report", Some(c.root), json!({"reason": "abuse"})).await;

    let url = format!("/admin/blogs/{bad}/reports");
    assert_eq!(e.get(&url, Some(c.moderator)).await.0, StatusCode::FORBIDDEN);
    let (s, p) = e.get(&url, Some(c.admin)).await;
    assert_eq!(s, StatusCode::OK, "{p}");
    assert_eq!(p["blog"]["reports_open"], 2);
    assert_eq!(p["owner_email_domain"], "example.com");
    assert_eq!(p["owner_mfa_enabled"], false);
    let reports = p["reports"].as_array().unwrap();
    assert_eq!(reports.len(), 2);
    assert!(
        reports
            .iter()
            .any(|r| r["post_slug"] == "uno" && r["reporter_username"] == "capo" && r["note"] == "pubblicità")
    );
    assert_eq!(
        e.get(&format!("/admin/blogs/{}/reports", Uuid::new_v4()), Some(c.admin)).await.0,
        StatusCode::NOT_FOUND
    );

    let act = format!("/admin/blogs/{bad}/action");
    let (s, _) = e.post(&act, Some(c.admin), json!({"action": "suspend", "note": ""})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = e.post(&act, Some(c.admin), json!({"action": "boh", "note": "prova"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    // nasconde solo i post segnalati
    let (s, j) =
        e.post(&act, Some(c.admin), json!({"action": "hide_reported_posts", "note": "contenuto ingannevole"})).await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert_eq!(j["reports_open"], 0);
    let hidden: Vec<(String, bool)> =
        sqlx::query_as("SELECT slug, is_hidden FROM posts ORDER BY slug").fetch_all(&e.pool).await.unwrap();
    assert_eq!(hidden, vec![("due".to_string(), false), ("uno".to_string(), true)]);
    let st: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT status::text FROM content_reports").fetch_all(&e.pool).await.unwrap();
    assert_eq!(st, vec!["actioned"]);
    assert_eq!(actions(e, "blog.reported_posts_hidden").await[0]["hidden_posts"], json!(["uno"]));

    // sospensione, riattivazione e archiviazione
    e.post(&format!("/posts/{other}/report"), Some(c.root), json!({"reason": "other"})).await;
    let (_, j) = e.post(&act, Some(c.admin), json!({"action": "suspend", "note": "in attesa di verifiche"})).await;
    assert_eq!(j["is_suspended"], true);
    e.post(&act, Some(c.admin), json!({"action": "restore", "note": "verificato"})).await;
    e.post("/blogs/rumoroso/report", Some(c.admin), json!({"reason": "other"})).await;
    let (_, j) = e.post(&act, Some(c.admin), json!({"action": "dismiss", "note": "infondata"})).await;
    assert_eq!(j["reports_open"], 0);
    let st: Vec<String> = sqlx::query_scalar("SELECT status::text FROM content_reports WHERE note IS NULL AND reason = 'other' ORDER BY created_at DESC LIMIT 1").fetch_all(&e.pool).await.unwrap();
    assert_eq!(st, vec!["dismissed"]);
    assert_eq!(actions(e, "blog.reports_dismissed").await.len(), 1);

    // disattiva il proprietario: sessioni revocate, blog sospeso; un amministratore non tocca un amministratore
    sqlx::query("INSERT INTO user_sessions (user_id, refresh_token_hash, expires_at) VALUES ($1, 'h', now() + interval '1 day')").bind(c.anna).execute(&e.pool).await.unwrap();
    let (s, j) =
        e.post(&act, Some(c.admin), json!({"action": "deactivate_owner", "note": "account usato per spam"})).await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert_eq!(j["is_suspended"], true);
    let active: bool =
        sqlx::query_scalar("SELECT is_active FROM users WHERE id = $1").bind(c.anna).fetch_one(&e.pool).await.unwrap();
    assert!(!active);
    assert_eq!(e.get("/users/me", Some(c.anna)).await.0, StatusCode::UNAUTHORIZED);
    let boss_blog = e.blog("del-capo", c.root, "public").await;
    let (s, _) = e
        .post(
            &format!("/admin/blogs/{boss_blog}/action"),
            Some(c.admin),
            json!({"action": "deactivate_owner", "note": "non consentito"}),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let own = e.blog("mio", c.root, "public").await;
    let (s, _) = e
        .post(
            &format!("/admin/blogs/{own}/action"),
            Some(c.root),
            json!({"action": "deactivate_owner", "note": "non consentito"}),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    c.env.teardown().await;
}

#[tokio::test]
async fn platform_comment_queue_spans_all_blogs() {
    let c = cast().await;
    let e = &c.env;
    e.blog("uno-blog", c.anna, "public").await;
    let other = e.user("bruno", "utente").await;
    e.blog("due-blog", other, "public").await;
    let p1 = published(e, c.anna, "uno-blog", "a").await;
    let p2 = published(e, other, "due-blog", "b").await;
    for (post, text, status) in
        [(&p1, "da moderare uno", "pending"), (&p2, "da moderare due", "pending"), (&p2, "già approvato", "approved")]
    {
        sqlx::query("INSERT INTO comments (post_id, author_display_name, content, status) VALUES ($1::uuid, 'ospite', $2, $3::comment_status)")
            .bind(post)
            .bind(text)
            .bind(status)
            .execute(&e.pool)
            .await
            .unwrap();
    }
    let (s, pending) = e.get("/admin/comments", Some(c.moderator)).await;
    assert_eq!(s, StatusCode::OK, "{pending}");
    let pending = pending.as_array().unwrap();
    assert_eq!(pending.len(), 2);
    assert!(
        pending
            .iter()
            .any(|c| c["blog_slug"] == "uno-blog" && c["post_slug"] == "a" && c["content"] == "da moderare uno")
    );
    assert!(pending.iter().all(|c| c["blog_id"].is_string() && c["blog_title"].is_string()));
    let (_, approved) = e.get("/admin/comments?status=approved", Some(c.admin)).await;
    assert_eq!(approved.as_array().unwrap().len(), 1);
    assert_eq!(e.get("/admin/comments?status=boh", Some(c.admin)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, q) = e.get("/admin/comments?q=DUE", Some(c.admin)).await;
    assert_eq!(q.as_array().unwrap().len(), 1);
    sqlx::query("UPDATE comments SET reported_to_platform = TRUE, report_note = 'offensivo', reported_at = now() WHERE content = 'già approvato'")
        .execute(&e.pool)
        .await
        .unwrap();
    let (_, rep) = e.get("/admin/comments?reported=true", Some(c.moderator)).await;
    let rep = rep.as_array().unwrap();
    assert_eq!(rep.len(), 1);
    assert_eq!(rep[0]["report_note"], "offensivo");
    assert_eq!(rep[0]["reported_to_platform"], true);
    c.env.teardown().await;
}
