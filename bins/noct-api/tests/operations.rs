//! Operazioni: job periodici, manutenzione dell'audit, pulizia dei blog, backup e attivazione a
//! caldo dei moduli.

mod common;

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use common::*;
use noct_core::Schedule;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

// ---- job periodici ---------------------------------------------------------------------------------------------------

fn schedule(name: &str, every_secs: u64) -> Schedule {
    Schedule { name: name.into(), subject: format!("{name}.run"), every_secs }
}

async fn queued(env: &Env, subject: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM noct_outbox WHERE subject = $1")
        .bind(subject)
        .fetch_one(&env.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_periodic_job_is_queued_once_per_interval_even_with_concurrent_replicas() {
    let env = setup(&[], &[]).await;
    let list = vec![schedule("prova.uno", 3600)];
    // dieci "repliche" nello stesso istante: una sola accoda
    let runs = futures::future::join_all((0..10).map(|_| noct_core::schedule::run_due(&env.pool, &list))).await;
    let fired: usize = runs.into_iter().map(|r| r.unwrap().len()).sum();
    assert_eq!(fired, 1);
    assert_eq!(queued(&env, "prova.uno.run").await, 1);
    // finché l'intervallo non scade, niente
    assert!(noct_core::schedule::run_due(&env.pool, &list).await.unwrap().is_empty());
    // scaduto l'intervallo, di nuovo una volta
    sqlx::query("UPDATE noct_schedules SET last_run_at = now() - interval '2 hours' WHERE name = 'prova.uno'")
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(noct_core::schedule::run_due(&env.pool, &list).await.unwrap(), vec!["prova.uno".to_string()]);
    assert_eq!(queued(&env, "prova.uno.run").await, 2);
    // job diversi non si ostacolano
    let both = vec![schedule("prova.uno", 3600), schedule("prova.due", 60)];
    assert_eq!(noct_core::schedule::run_due(&env.pool, &both).await.unwrap(), vec!["prova.due".to_string()]);
    env.teardown().await;
}

#[tokio::test]
async fn the_registry_collects_the_schedules_of_active_modules_only() {
    let env = setup(&["newsletter"], &[]).await;
    let names: Vec<String> = env.registry.schedules(&env.enabled).into_iter().map(|s| s.name).collect();
    for expected in ["newsletter.dispatch_due", "audit.maintenance", "blogs.purge_deleted"] {
        assert!(names.iter().any(|n| n == expected), "{expected} in {names:?}");
    }
    assert!(!names.iter().any(|n| n.starts_with("backup.")), "il modulo backup è spento: {names:?}");
    let all = setup(&["newsletter", "backup"], &[]).await;
    assert!(all.registry.schedules(&all.enabled).iter().any(|s| s.name == "backup.run" && s.every_secs == 86_400));
    all.teardown().await;
    env.teardown().await;
}

#[tokio::test]
async fn the_running_scheduler_queues_jobs_and_the_consumer_runs_them() {
    let env = setup(&["newsletter"], &[]).await;
    // una campagna pianificata nel passato: il job periodico la fa partire senza che nessuno chiami la rotta
    sqlx::query("INSERT INTO newsletter_campaigns (kind, subject, body_markdown, status, scheduled_at) VALUES ('manual', 's', 'b', 'scheduled', now() - interval '1 minute')")
        .execute(&env.pool)
        .await
        .unwrap();
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();
    for _ in 0..100 {
        let status: String =
            sqlx::query_scalar("SELECT status::text FROM newsletter_campaigns").fetch_one(&env.pool).await.unwrap();
        if status != "scheduled" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let status: String =
        sqlx::query_scalar("SELECT status::text FROM newsletter_campaigns").fetch_one(&env.pool).await.unwrap();
    assert_ne!(status, "scheduled", "il job periodico non è partito");
    background.stop().await;
    env.teardown().await;
}

// ---- manutenzione dell'audit -----------------------------------------------------------------------------------------

async fn old_event(env: &Env, days_ago: i64, action: &str) {
    sqlx::query(
        "INSERT INTO audit_log (occurred_at, actor_type, action, payload) VALUES ($1, 'system', $2, '{\"n\": 1}')",
    )
    .bind(Utc::now() - Duration::days(days_ago))
    .bind(action)
    .execute(&env.pool)
    .await
    .unwrap();
}

async fn count_audit(env: &Env) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit_log").fetch_one(&env.pool).await.unwrap()
}

#[tokio::test]
async fn audit_maintenance_archives_closed_weeks_then_prunes_only_what_is_archived() {
    let env = setup(&[], &[]).await;
    old_event(&env, 200, "vecchio.uno").await;
    old_event(&env, 188, "vecchio.due").await;
    old_event(&env, 20, "recente.uno").await;
    old_event(&env, 0, "oggi").await;

    let report = noct_audit::maintenance::run_once(&env.state, true).await.unwrap();
    assert!(report.archived_weeks.len() >= 28, "{:?}", report.archived_weeks);
    // oltre la conservazione (105 giorni) e già archiviati: spariti; il resto resta
    assert_eq!(report.deleted_rows, 2);
    assert_eq!(count_audit(&env).await, 2);
    // l'archivio è su storage, con checksum registrato
    let (label, key, rows, digest): (String, String, i32, String) = sqlx::query_as(
        "SELECT week_label, object_key, row_count, sha256 FROM audit_archive_runs WHERE row_count > 0 ORDER BY period_start LIMIT 1",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert_eq!(rows, 1);
    assert_eq!(digest.len(), 64);
    assert!(key.ends_with(&format!("audit-archive/{label}.jsonl.gz")), "{key}");
    let object = env.state.services.storage.get(&env.state.config.storage.bucket_audit, &key).await.unwrap().unwrap();
    assert_eq!(&object.data[..2], &[0x1f, 0x8b], "gzip");

    // idempotente
    let again = noct_audit::maintenance::run_once(&env.state, true).await.unwrap();
    assert!(again.archived_weeks.is_empty());
    assert_eq!(again.deleted_rows, 0);

    // ripristino di una settimana: reinserisce l'evento cancellato, una volta sola
    assert_eq!(noct_audit::maintenance::restore_week(&env.state, &label).await.unwrap(), 1);
    assert_eq!(noct_audit::maintenance::restore_week(&env.state, &label).await.unwrap(), 0);
    let restored: serde_json::Value =
        sqlx::query_scalar("SELECT payload FROM audit_log WHERE action LIKE 'vecchio.%' LIMIT 1")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert_eq!(restored, json!({"n": 1}));
    assert!(noct_audit::maintenance::restore_week(&env.state, "2000w01").await.is_err());
    assert!(noct_audit::maintenance::restore_week(&env.state, "../x").await.is_err());
    env.teardown().await;
}

#[tokio::test]
async fn audit_pruning_waits_for_the_archive_and_respects_a_short_retention() {
    let env = setup(&[], &[]).await;
    old_event(&env, 400, "antico").await;
    // archiviazione attiva ma mai eseguita: niente si cancella
    assert_eq!(noct_audit::maintenance::prune(&env.state, Utc::now(), true).await.unwrap(), 0);
    assert_eq!(count_audit(&env).await, 1);
    // archiviazione spenta: vale la sola conservazione
    assert_eq!(noct_audit::maintenance::prune(&env.state, Utc::now(), false).await.unwrap(), 1);
    // una conservazione sotto la settimana è ignorata
    old_event(&env, 30, "recente").await;
    sqlx::query("UPDATE platform_config SET audit_retention_days = 7").execute(&env.pool).await.ok();
    assert_eq!(count_audit(&env).await, 1);
    env.teardown().await;
}

// ---- cancellazione definitiva dei blog -----------------------------------------------------------------------------------

#[tokio::test]
async fn deleted_blogs_are_purged_after_the_grace_period_with_their_files() {
    let env = setup(&["comments"], &[]).await;
    let owner = env.user("anna", "utente").await;
    let kept = env.blog("tenuto", owner, "public").await;
    let gone = env.blog("scaduto", owner, "public").await;
    let recent = env.blog("recente", owner, "public").await;
    sqlx::query("UPDATE blogs SET deleted_at = now() - interval '31 days' WHERE id = $1")
        .bind(gone)
        .execute(&env.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE blogs SET deleted_at = now() - interval '5 days' WHERE id = $1")
        .bind(recent)
        .execute(&env.pool)
        .await
        .unwrap();
    let (s, p) = env.post("/blogs/scaduto/posts", None, json!({})).await;
    assert_ne!(s, StatusCode::OK, "{p}");
    sqlx::query("INSERT INTO posts (blog_id, author_id, author_display_name, locale, translation_group_id, title, slug, content) VALUES ($1, $2, 'a', 'it', gen_random_uuid(), 't', 's', 'c')")
        .bind(gone)
        .bind(owner)
        .execute(&env.pool)
        .await
        .unwrap();
    let key = format!("{}/userdata/{owner}/{gone}/media/x.png", env.state.config.site_slug);
    let other = format!("{}/userdata/{owner}/{kept}/media/y.png", env.state.config.site_slug);
    let bucket = env.state.config.storage.bucket_content.clone();
    env.state.services.storage.ensure_bucket(&bucket).await.unwrap();
    for k in [&key, &other] {
        env.state.services.storage.put(&bucket, k, "x".into(), "image/png").await.unwrap();
    }

    let purged = noct_blogs::jobs::purge_due(&env.state).await.unwrap();
    assert_eq!(purged, 1);
    let left: Vec<String> =
        sqlx::query_scalar("SELECT slug FROM blogs ORDER BY slug").fetch_all(&env.pool).await.unwrap();
    assert_eq!(left, vec!["recente", "tenuto"]);
    let posts: i64 = sqlx::query_scalar("SELECT count(*) FROM posts").fetch_one(&env.pool).await.unwrap();
    assert_eq!(posts, 0, "i post vanno via a cascata");
    assert!(env.state.services.storage.get(&bucket, &key).await.unwrap().is_none());
    assert!(env.state.services.storage.get(&bucket, &other).await.unwrap().is_some());
    let log: Vec<serde_json::Value> = sqlx::query_scalar("SELECT payload FROM audit_log WHERE action = 'blog.purged'")
        .fetch_all(&env.pool)
        .await
        .unwrap();
    assert_eq!(log, vec![json!({"slug": "scaduto"})]);
    // idempotente
    assert_eq!(noct_blogs::jobs::purge_due(&env.state).await.unwrap(), 0);
    env.teardown().await;
}

// ---- backup ----------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn published_posts_are_copied_as_markdown_next_to_the_author_files() {
    let env = setup(&["backup"], &[]).await;
    let owner = env.user("anna", "utente").await;
    let blog = env.blog("diario", owner, "public").await;
    let (_, p) = env
        .post("/blogs/diario/posts", Some(owner), json!({"slug": "uno", "title": "Uno", "content": "# Ciao\n\ntesto"}))
        .await;
    let id = p["id"].as_str().unwrap().to_string();
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();
    env.post(&format!("/posts/{id}/publish"), Some(owner), json!({})).await;
    let key = noct_backup::events::post_key(&env.state.config.site_slug, owner, blog, id.parse().unwrap());
    let bucket = env.state.config.storage.bucket_content.clone();
    let mut copy = None;
    for _ in 0..100 {
        copy = env.state.services.storage.get(&bucket, &key).await.unwrap();
        if copy.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(copy.expect("copia del post").data, "# Ciao\n\ntesto".as_bytes());
    background.stop().await;
    env.teardown().await;
}

#[tokio::test]
async fn infrastructure_backup_dumps_postgres_and_mirrors_the_buckets_to_the_external_storage() {
    let endpoint = std::env::var("NOCT_TEST_S3_ENDPOINT").expect("NOCT_TEST_S3_ENDPOINT non impostata");
    let dest_bucket = format!("backup-{}", Uuid::new_v4().simple());
    let env = setup(
        &["backup"],
        &[
            ("NOCT_BACKUP_S3_BUCKET", dest_bucket.as_str()),
            ("NOCT_BACKUP_S3_ENDPOINT_URL", endpoint.as_str()),
            ("NOCT_BACKUP_S3_REGION", "us-east-1"),
            ("NOCT_BACKUP_S3_ACCESS_KEY_ID", "test"),
            ("NOCT_BACKUP_S3_SECRET_ACCESS_KEY", "test"),
        ],
    )
    .await;
    env.user("anna", "utente").await;
    let bucket = env.state.config.storage.bucket_content.clone();
    env.state.services.storage.ensure_bucket(&bucket).await.unwrap();
    env.state
        .services
        .storage
        .put(&bucket, "notturni/userdata/u/b/media/a.png", "PNG".into(), "image/png")
        .await
        .unwrap();

    let report = noct_backup::infra::run_once(&env.state).await.unwrap().expect("backup configurato");
    assert_eq!(report.mirrored_objects, 1);
    assert!(
        report.dump_key.starts_with("postgres/noct_e2e_") && report.dump_key.ends_with(".dump"),
        "{}",
        report.dump_key
    );

    let dest = noct_storage::S3Storage::new(&noct_storage::S3Config {
        endpoint_url: Some(endpoint),
        region: Some("us-east-1".into()),
        access_key_id: Some("test".into()),
        secret_access_key: Some("test".into()),
        public_url: None,
    })
    .await;
    use noct_storage::Storage;
    let dump = dest.get(&dest_bucket, &report.dump_key).await.unwrap().expect("dump");
    assert_eq!(&dump.data[..5], b"PGDMP", "formato custom di pg_dump");
    let mirrored =
        dest.get(&dest_bucket, &format!("s3/{bucket}/notturni/userdata/u/b/media/a.png")).await.unwrap().unwrap();
    assert_eq!(mirrored.data, "PNG".as_bytes());
    env.teardown().await;
}

#[tokio::test]
async fn infrastructure_backup_is_off_without_a_destination() {
    let env = setup(&["backup"], &[]).await;
    assert!(noct_backup::infra::run_once(&env.state).await.unwrap().is_none());
    env.teardown().await;
}

// ---- attivazione a caldo dei moduli --------------------------------------------------------------------------------

struct Fresh {
    url: String,
    admin: sqlx::PgPool,
    name: String,
}

impl Fresh {
    async fn new() -> Fresh {
        let admin_url = std::env::var("NOCT_TEST_DATABASE_URL").expect("NOCT_TEST_DATABASE_URL non impostata");
        let admin = sqlx::postgres::PgPoolOptions::new().max_connections(2).connect(&admin_url).await.unwrap();
        let name = format!("noct_hot_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE DATABASE {name}")).execute(&admin).await.unwrap();
        Fresh { url: format!("{}/{name}", admin_url.rsplit_once('/').unwrap().0), admin, name }
    }

    fn env(&self, modules: &str) -> noct_api::EnvFn {
        let vars = [
            ("NOCT_DATABASE_URL".to_string(), self.url.clone()),
            ("NOCT_JWT_SECRET".to_string(), "x".repeat(40)),
            ("NOCT_MODULES".to_string(), modules.to_string()),
            ("NOCT_LOCAL_STORAGE_BASE_PATH".to_string(), std::env::temp_dir().join(&self.name).display().to_string()),
        ];
        Arc::new(move |k| vars.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()))
    }

    async fn drop(self) {
        sqlx::query(&format!("DROP DATABASE {} WITH (FORCE)", self.name)).execute(&self.admin).await.unwrap();
    }
}

async fn call(app: &axum::Router, method: &str, path: &str, token: Option<&str>) -> (StatusCode, serde_json::Value) {
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let mut req = Request::builder().method(method).uri(format!("/api/v1{path}"));
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    let res = app.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn modules_are_enabled_and_disabled_at_runtime_without_restarting() {
    let db = Fresh::new().await;
    let mut rt = noct_api::bootstrap(db.env("taxonomy")).await.unwrap();
    assert!(rt.state.module_enabled("taxonomy") && !rt.state.module_enabled("pages"));
    let root: Uuid = sqlx::query_scalar("INSERT INTO users (username, email, platform_role) VALUES ('root', 'root@example.com', 'super_admin') RETURNING id")
        .fetch_one(&rt.state.db)
        .await
        .unwrap();
    let token = noct_core::security::jwt::encode(
        rt.state.config.jwt_secret.as_bytes(),
        "access",
        root,
        Utc::now().timestamp(),
        900,
    );
    let live = noct_api::LiveRouter::new(noct_api::app(&rt.registry, &rt.enabled, rt.state.clone()));
    let service = live.service();

    // pages è spento: niente rotte
    assert_eq!(call(&service, "GET", "/pages?locale=it", None).await.0, StatusCode::NOT_FOUND);
    let (s, list) = call(&service, "GET", "/admin/modules", Some(&token)).await;
    assert_eq!(s, StatusCode::OK, "{list}");
    let pages = list.as_array().unwrap().iter().find(|m| m["name"] == "pages").unwrap();
    assert_eq!((pages["enabled"].as_bool(), pages["active"].as_bool()), (Some(false), Some(false)));
    assert!(list.as_array().unwrap().iter().any(|m| m["name"] == "users" && m["kind"] == "core"));

    // un core non si tocca, un modulo sconosciuto non esiste
    assert_eq!(call(&service, "POST", "/admin/modules/users/disable", Some(&token)).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(call(&service, "POST", "/admin/modules/boh/enable", Some(&token)).await.0, StatusCode::NOT_FOUND);
    // senza Super Admin niente
    assert_eq!(call(&service, "POST", "/admin/modules/pages/enable", None).await.0, StatusCode::UNAUTHORIZED);
    // attivazione: lo stato voluto cambia subito, quello in esecuzione dopo il ricaricamento
    let (s, e) = call(&service, "POST", "/admin/modules/pages/enable", Some(&token)).await;
    assert_eq!(s, StatusCode::OK, "{e}");
    assert_eq!(
        (e["enabled"].as_bool(), e["active"].as_bool(), e["pending"].as_bool()),
        (Some(true), Some(false), Some(true))
    );
    let state = rt.reload_if_changed().await.unwrap().expect("ricaricamento");
    live.swap(noct_api::app(&rt.registry, &rt.enabled, state));
    assert_eq!(call(&service, "GET", "/pages?locale=it", None).await.0, StatusCode::OK);
    let tables: bool =
        sqlx::query_scalar("SELECT to_regclass('blog_pages') IS NOT NULL").fetch_one(&rt.state.db).await.unwrap();
    assert!(tables, "migrazione del modulo applicata");
    // nessun cambio: nessun ricaricamento
    assert!(rt.reload_if_changed().await.unwrap().is_none());

    let (s, e) = call(&service, "POST", "/admin/modules/pages/disable", Some(&token)).await;
    assert_eq!(s, StatusCode::OK, "{e}");
    let state = rt.reload_if_changed().await.unwrap().expect("ricaricamento");
    live.swap(noct_api::app(&rt.registry, &rt.enabled, state));
    assert_eq!(call(&service, "GET", "/pages?locale=it", None).await.0, StatusCode::NOT_FOUND);
    // dati e tabelle intatti
    let tables: bool =
        sqlx::query_scalar("SELECT to_regclass('blog_pages') IS NOT NULL").fetch_one(&rt.state.db).await.unwrap();
    assert!(tables);
    let log: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action IN ('module.enabled', 'module.disabled')")
            .fetch_one(&rt.state.db)
            .await
            .unwrap();
    assert_eq!(log, 2);

    // un nuovo processo riparte dallo stato salvato, non da NOCT_MODULES
    rt.state.db.close().await;
    let again = noct_api::bootstrap(db.env("comments")).await.unwrap();
    assert!(again.state.module_enabled("taxonomy"), "vale la tabella, non NOCT_MODULES");
    assert!(!again.state.module_enabled("comments"));
    again.state.db.close().await;
    db.drop().await;
}

// ---- CORS ----------------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn browsers_get_cors_headers_only_for_allowed_origins() {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    let env = setup_with(
        &[],
        &[],
        Some(&|c: &mut noct_core::Config| {
            c.cors_origins = vec!["http://localhost:3000".into()];
            c.cors_origin_regex = Some(r"https://([a-z0-9-]+\.)?notturni\.eu".into());
        }),
    )
    .await;
    let preflight = |origin: &'static str| {
        let req = Request::builder()
            .method("OPTIONS")
            .uri("/api/v1/blogs")
            .header("origin", origin)
            .header("access-control-request-method", "POST")
            .header("access-control-request-headers", "authorization,content-type,x-csrf-token")
            .body(Body::empty())
            .unwrap();
        env.app.clone().oneshot(req)
    };
    for allowed in ["http://localhost:3000", "https://notturni.eu", "https://anna.notturni.eu"] {
        let res = preflight(allowed).await.unwrap();
        assert_eq!(res.headers().get("access-control-allow-origin").and_then(|v| v.to_str().ok()), Some(allowed));
        assert_eq!(res.headers().get("access-control-allow-credentials").and_then(|v| v.to_str().ok()), Some("true"));
        let headers = res
            .headers()
            .get("access-control-allow-headers")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_lowercase();
        assert!(headers.contains("x-csrf-token") && headers.contains("authorization"), "{headers}");
    }
    for denied in ["https://evil.example.com", "https://notturni.eu.evil.com", "http://anna.notturni.eu"] {
        let res = preflight(denied).await.unwrap();
        assert!(res.headers().get("access-control-allow-origin").is_none(), "{denied}");
    }
    // una risposta normale da un'origine ammessa porta l'header
    let req =
        Request::builder().uri("/api/v1/health").header("origin", "http://localhost:3000").body(Body::empty()).unwrap();
    let res = env.app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        res.headers().get("access-control-allow-origin").and_then(|v| v.to_str().ok()),
        Some("http://localhost:3000")
    );
    env.teardown().await;
}
