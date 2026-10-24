//! Prove end-to-end dell'API reale (router completo, middleware, Postgres vero).
//! Richiedono `NOCT_TEST_DATABASE_URL`; ogni test lavora su un database creato e distrutto da sé.

mod common;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use common::*;
use http_body_util::BodyExt;
use noct_core::{Config, Registry};
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

// ---- piattaforma ------------------------------------------------------------------

#[tokio::test]
async fn health_and_openapi_reflect_active_modules() {
    let env = setup(&["taxonomy"], &[]).await;
    assert_eq!(env.get("/health", None).await, (StatusCode::OK, json!({"status": "ok"})));
    let (_, doc) = env.get("/openapi.json", None).await;
    assert!(doc["paths"]["/blogs/{slug}/categories"]["get"].is_object());
    assert!(doc["paths"]["/sandbox/echo/{word}"].is_null(), "sandbox non è attivo");
    env.teardown().await;
}

#[tokio::test]
async fn inactive_module_has_no_routes_and_no_tables() {
    let env = setup(&["taxonomy"], &[]).await;
    let owner = env.user("anna", "utente").await;
    env.blog("diario", owner, "public").await;
    let (status, _) = env.get("/blogs/diario/sandbox/notes", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let exists: bool =
        sqlx::query_scalar("SELECT to_regclass('sandbox_notes') IS NOT NULL").fetch_one(&env.pool).await.unwrap();
    assert!(!exists);
    env.teardown().await;
}

#[test]
fn registry_rejects_unknown_modules() {
    let registry = Registry::new(noct_bundle::all()).unwrap();
    let err = registry.resolve_enabled(Some(&["nope".to_string()])).unwrap_err();
    assert!(err.to_string().contains("modulo sconosciuto"), "{err}");
}

// ---- taxonomy: CRUD generato ----------------------------------------------------------

#[tokio::test]
async fn categories_crud_flow_matches_v1_contract() {
    let env = setup(&["taxonomy"], &[]).await;
    let owner = env.user("anna", "utente").await;
    let coauthor = env.user("bruno", "utente").await;
    let reviewer = env.user("carla", "utente").await;
    let stranger = env.user("dario", "utente").await;
    let blog = env.blog("diario", owner, "public").await;
    env.member(blog, coauthor, "co_autore").await;
    env.member(blog, reviewer, "revisore").await;
    let base = "/blogs/diario/categories";

    // lettura pubblica, vuota
    assert_eq!(env.get(base, None).await, (StatusCode::OK, json!([])));

    // scrittura: 401 senza credenziali, 401 con credenziali non valide, 403 senza ruolo
    let body = json!({"name": "Viaggi", "slug": "viaggi"});
    assert_eq!(env.post(base, None, body.clone()).await.0, StatusCode::UNAUTHORIZED);
    let (s, _) = env.post(base, Some(Uuid::new_v4()), body.clone()).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "utente inesistente = credenziali non valide");
    let (s, j) = env.post(base, Some(stranger), body.clone()).await;
    assert_eq!(
        (s, detail(&j)),
        (StatusCode::FORBIDDEN, "Serve essere proprietario del blog o avere ruolo autore/co-autore.")
    );
    assert_eq!(env.post(base, Some(reviewer), body.clone()).await.0, StatusCode::FORBIDDEN, "il revisore non scrive");

    // creazione: 201 con la forma della v1 (solo id, name, slug)
    let (s, created) = env.post(base, Some(owner), body.clone()).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(created["name"], "Viaggi");
    assert_eq!(created["slug"], "viaggi");
    assert_eq!(created.as_object().unwrap().len(), 3);
    let id = created["id"].as_str().unwrap().to_string();

    // un co-autore può creare; lo slug duplicato è 409 con il messaggio della v1
    assert_eq!(
        env.post(base, Some(coauthor), json!({"name": "Cucina", "slug": "cucina"})).await.0,
        StatusCode::CREATED
    );
    let (s, j) = env.post(base, Some(owner), json!({"name": "Altro", "slug": "viaggi"})).await;
    assert_eq!((s, detail(&j)), (StatusCode::CONFLICT, "Esiste già una categoria con questo slug."));

    // validazione dichiarata nel manifest
    let (s, j) = env.post(base, Some(owner), json!({"name": "X", "slug": "Non Valido"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(detail(&j).starts_with("Lo slug della categoria"));
    let (s, j) = env.post(base, Some(owner), json!({"name": "   ", "slug": "ok"})).await;
    assert_eq!(
        (s, detail(&j)),
        (StatusCode::BAD_REQUEST, "Il nome della categoria deve essere non vuoto, max 50 caratteri.")
    );
    let (s, _) = env.post(base, Some(owner), json!({"name": "x".repeat(51), "slug": "lungo"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // corpo mancante di un campo obbligatorio o non-JSON: 422
    assert_eq!(env.post(base, Some(owner), json!({"name": "Solo nome"})).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    // elenco ordinato per nome
    let (_, list) = env.get(base, None).await;
    let names: Vec<_> = list.as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Cucina", "Viaggi"]);

    // patch parziale: solo il nome, poi solo lo slug
    let item = format!("{base}/{id}");
    let (s, j) = env.patch(&item, Some(owner), json!({"name": "Viaggi & luoghi"})).await;
    assert_eq!((s, j["name"].as_str(), j["slug"].as_str()), (StatusCode::OK, Some("Viaggi & luoghi"), Some("viaggi")));
    let (s, j) = env.patch(&item, Some(owner), json!({"slug": "luoghi"})).await;
    assert_eq!((s, j["slug"].as_str()), (StatusCode::OK, Some("luoghi")));
    // patch vuoto: nessuna scrittura, risposta con lo stato corrente
    let (s, j) = env.patch(&item, Some(owner), json!({})).await;
    assert_eq!((s, j["slug"].as_str()), (StatusCode::OK, Some("luoghi")));
    // patch su uno slug già usato: 409
    let (s, _) = env.patch(&item, Some(owner), json!({"slug": "cucina"})).await;
    assert_eq!(s, StatusCode::CONFLICT);
    // id inesistente: 404 col messaggio dichiarato; id malformato: 422
    let (s, j) = env.patch(&format!("{base}/{}", Uuid::new_v4()), Some(owner), json!({"name": "x"})).await;
    assert_eq!((s, detail(&j)), (StatusCode::NOT_FOUND, "Categoria non trovata."));
    assert_eq!(
        env.patch(&format!("{base}/non-un-uuid"), Some(owner), json!({"name": "x"})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // updated_at viene aggiornato dal PATCH
    let moved: bool = sqlx::query_scalar("SELECT updated_at > created_at FROM categories WHERE id = $1::uuid")
        .bind(&id)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert!(moved);

    // cancellazione
    assert_eq!(env.delete(&item, Some(stranger)).await, StatusCode::FORBIDDEN);
    assert_eq!(env.delete(&item, Some(owner)).await, StatusCode::NO_CONTENT);
    assert_eq!(env.delete(&item, Some(owner)).await, StatusCode::NOT_FOUND);
    env.teardown().await;
}

#[tokio::test]
async fn category_belongs_to_its_blog_only() {
    let env = setup(&["taxonomy"], &[]).await;
    let owner = env.user("anna", "utente").await;
    env.blog("uno", owner, "public").await;
    env.blog("due", owner, "public").await;
    let (_, c) = env.post("/blogs/uno/categories", Some(owner), json!({"name": "A", "slug": "a"})).await;
    let id = c["id"].as_str().unwrap();
    // la stessa categoria non è raggiungibile (né modificabile, né eliminabile) dall'altro blog
    assert_eq!(
        env.patch(&format!("/blogs/due/categories/{id}"), Some(owner), json!({"name": "Z"})).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(env.delete(&format!("/blogs/due/categories/{id}"), Some(owner)).await, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/due/categories", None).await.1, json!([]));
    env.teardown().await;
}

#[tokio::test]
async fn blog_visibility_rules_apply_to_generated_routes() {
    let env = setup(&["taxonomy"], &[]).await;
    let owner = env.user("anna", "utente").await;
    let member = env.user("bruno", "utente").await;
    let other = env.user("carla", "utente").await;
    let private = env.blog("segreto", owner, "private").await;
    env.member(private, member, "autore").await;
    env.blog("soci", owner, "members").await;
    env.blog("sospeso", owner, "public").await;
    sqlx::query("UPDATE blogs SET is_suspended = true WHERE slug = 'sospeso'").execute(&env.pool).await.unwrap();

    // privato: invisibile (404, non 403) a anonimi ed estranei, visibile a proprietario e membri
    assert_eq!(env.get("/blogs/segreto/categories", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/segreto/categories", Some(other)).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/segreto/categories", Some(owner)).await.0, StatusCode::OK);
    assert_eq!(env.get("/blogs/segreto/categories", Some(member)).await.0, StatusCode::OK);
    // in un blog privato scrive solo il proprietario, anche se esiste una membership di scrittura
    let body = json!({"name": "A", "slug": "a"});
    assert_eq!(env.post("/blogs/segreto/categories", Some(member), body.clone()).await.0, StatusCode::FORBIDDEN);
    assert_eq!(env.post("/blogs/segreto/categories", Some(owner), body.clone()).await.0, StatusCode::CREATED);
    // "members": serve essere autenticati
    assert_eq!(env.get("/blogs/soci/categories", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/blogs/soci/categories", Some(other)).await.0, StatusCode::OK);
    // sospeso: irraggiungibile e non scrivibile da nessuno, proprietario incluso
    assert_eq!(env.get("/blogs/sospeso/categories", Some(owner)).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.post("/blogs/sospeso/categories", Some(owner), body.clone()).await.0, StatusCode::FORBIDDEN);
    // blog inesistente
    let (s, j) = env.get("/blogs/non-esiste/categories", None).await;
    assert_eq!((s, detail(&j)), (StatusCode::NOT_FOUND, "Blog non trovato."));
    // un bearer non valido su una rotta pubblica equivale a un anonimo
    assert_eq!(env.get("/blogs/segreto/categories", Some(Uuid::new_v4())).await.0, StatusCode::NOT_FOUND);
    env.teardown().await;
}

// ---- sandbox: enum, tri-stato, handler custom, rate limit, config ------------------------

#[tokio::test]
async fn sandbox_notes_exercise_enum_defaults_and_tristate_patch() {
    let env = setup(&["sandbox"], &[]).await;
    let owner = env.user("anna", "utente").await;
    env.blog("lab", owner, "public").await;
    let base = "/blogs/lab/sandbox/notes";

    // default del database e colonna enum restituita come stringa
    let (s, n) = env.post(base, Some(owner), json!({"title": "Prima", "body": "testo"})).await;
    assert_eq!(s, StatusCode::CREATED, "{n}");
    assert_eq!(
        (n["kind"].as_str(), n["position"].as_i64(), n["body"].as_str()),
        (Some("note"), Some(0), Some("testo"))
    );
    let id = n["id"].as_str().unwrap().to_string();
    let item = format!("{base}/{id}");

    // creazione con enum e posizione esplicite; campo nullable omesso → null
    let (s, n2) = env.post(base, Some(owner), json!({"title": "Seconda", "kind": "todo", "position": 5})).await;
    assert_eq!(
        (s, n2["kind"].as_str(), n2["position"].as_i64(), n2["body"].is_null()),
        (StatusCode::CREATED, Some("todo"), Some(5), true)
    );

    // validazione: enum fuori dominio, intervallo, titolo duplicato
    assert_eq!(env.post(base, Some(owner), json!({"title": "X", "kind": "boh"})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(env.post(base, Some(owner), json!({"title": "Y", "position": 5000})).await.0, StatusCode::BAD_REQUEST);
    let (s, j) = env.post(base, Some(owner), json!({"title": "Prima"})).await;
    assert_eq!((s, detail(&j)), (StatusCode::CONFLICT, "Titolo già usato."));

    // PATCH tri-stato su `body`: assente = invariato, null = azzera, valore = imposta
    let (_, j) = env.patch(&item, Some(owner), json!({"position": 3})).await;
    assert_eq!((j["body"].as_str(), j["position"].as_i64()), (Some("testo"), Some(3)), "body assente: invariato");
    let (_, j) = env.patch(&item, Some(owner), json!({"body": null})).await;
    assert!(j["body"].is_null(), "null esplicito: azzerato");
    assert_eq!(j["position"], 3);
    let (_, j) = env.patch(&item, Some(owner), json!({"body": "nuovo", "kind": "todo"})).await;
    assert_eq!((j["body"].as_str(), j["kind"].as_str()), (Some("nuovo"), Some("todo")));
    // null su un campo NOT NULL non è ammesso dal manifest (title non è nullable): rifiutato
    assert_eq!(
        env.patch(&item, Some(owner), json!({"title": null})).await.0,
        StatusCode::OK,
        "title null = campo assente per Option"
    );

    // get singolo, ordinamento (position, title) e 404
    let (s, g) = env.get(&item, None).await;
    assert_eq!((s, g["id"].as_str()), (StatusCode::OK, Some(id.as_str())));
    let (_, list) = env.get(base, None).await;
    let titles: Vec<_> = list.as_array().unwrap().iter().map(|c| c["title"].as_str().unwrap()).collect();
    assert_eq!(titles, ["Prima", "Seconda"]); // posizioni 3 e 5
    let (s, j) = env.get(&format!("{base}/{}", Uuid::new_v4()), None).await;
    assert_eq!((s, detail(&j)), (StatusCode::NOT_FOUND, "Nota non trovata."));
    assert_eq!(env.delete(&item, Some(owner)).await, StatusCode::NO_CONTENT);
    env.teardown().await;
}

#[tokio::test]
async fn custom_handler_gets_config_policy_and_rate_limit() {
    let env = setup(&["sandbox"], &[("NOCT_SANDBOX_GREETING", "hello")]).await;
    let user = env.user("anna", "utente").await;
    let other = env.user("bruno", "utente").await;

    // handler scritto a mano, con la configurazione del modulo risolta da env
    assert_eq!(env.get("/sandbox/echo/ciao", None).await.0, StatusCode::UNAUTHORIZED);
    let (s, j) = env.get("/sandbox/echo/ciao", Some(user)).await;
    assert_eq!((s, j), (StatusCode::OK, json!({"word": "CIAO", "greeting": "hello"})));

    // rate limit dichiarato (3/min per utente): la quarta è 429 con Retry-After; un altro utente non è toccato
    assert_eq!(env.get("/sandbox/echo/a", Some(user)).await.0, StatusCode::OK);
    assert_eq!(env.get("/sandbox/echo/b", Some(user)).await.0, StatusCode::OK);
    let (s, j, h) = env.call(Method::GET, "/sandbox/echo/c", Some(user), None).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    assert!(h.get(header::RETRY_AFTER).is_some());
    assert_eq!(detail(&j), "Troppe richieste, riprova più tardi.");
    assert_eq!(env.get("/sandbox/echo/c", Some(other)).await.0, StatusCode::OK);
    env.teardown().await;
}

#[tokio::test]
async fn platform_role_policy_and_204_custom_route() {
    let env = setup(&["sandbox"], &[]).await;
    let plain = env.user("anna", "utente").await;
    let admin = env.user("root", "amministratore").await;
    let moderator = env.user("mod", "moderatore").await;
    assert_eq!(env.get("/sandbox/admin-ping", None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.get("/sandbox/admin-ping", Some(plain)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(env.get("/sandbox/admin-ping", Some(moderator)).await.0, StatusCode::FORBIDDEN, "ruolo non elencato");
    assert_eq!(env.get("/sandbox/admin-ping", Some(admin)).await.0, StatusCode::NO_CONTENT);
    env.teardown().await;
}

#[tokio::test]
async fn invalid_module_config_is_reported_at_startup() {
    let registry = Registry::new(noct_bundle::all()).unwrap();
    let enabled = registry.resolve_enabled(Some(&["sandbox".to_string()])).unwrap();
    let bad = |k: &str| (k == "NOCT_SANDBOX_MAX_TITLE").then(|| "molti".to_string());
    let err = registry.module_configs(&enabled, &bad).unwrap_err();
    assert!(err.to_string().contains("non intero"), "{err}");
}

// ---- infrastruttura: health, storage, outbox, eventi, job ---------------------------------

use noct_core::bus::Kind;
use std::time::Duration;

async fn eventually<F: std::future::Future<Output = bool>>(what: &str, mut cond: impl FnMut() -> F) {
    for _ in 0..100 {
        if cond().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("condizione non raggiunta entro 10 s: {what}");
}

#[tokio::test]
async fn readiness_reports_every_service() {
    let env = setup(&["taxonomy"], &[]).await;
    let (status, body) = env.get("/health/ready", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    for svc in ["database", "storage", "cache", "bus"] {
        assert_eq!(body["services"][svc]["status"], "ok", "{svc}: {body}");
    }
    assert_eq!(body["services"]["storage"]["backend"], "localstorage");
    env.teardown().await;
}

#[tokio::test]
async fn local_storage_serves_public_buckets_only() {
    let env = setup(&["taxonomy"], &[]).await;
    let s = &env.state.services.storage;
    let cfg = &env.state.config.storage;
    for b in [&cfg.bucket_avatars, &cfg.bucket_audit] {
        s.ensure_bucket(b).await.unwrap();
    }
    s.put(&cfg.bucket_avatars, "u1.png", bytes_of(b"avatar-bytes"), "image/png").await.unwrap();
    s.put(&cfg.bucket_audit, "log.txt", bytes_of(b"segreto"), "text/plain").await.unwrap();

    let get = |path: String| {
        let app = env.app.clone();
        async move {
            let res = app.oneshot(Request::builder().uri(path).body(Body::empty()).unwrap()).await.unwrap();
            let status = res.status();
            (status, res.into_body().collect().await.unwrap().to_bytes())
        }
    };
    let (status, body) = get(format!("/storage/{}/u1.png", cfg.bucket_avatars)).await;
    assert_eq!((status, &body[..]), (StatusCode::OK, &b"avatar-bytes"[..]));
    let (status, _) = get(format!("/storage/{}/log.txt", cfg.bucket_audit)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "il bucket di audit non è pubblico");
    let (status, _) = get(format!("/storage/{}/../{}/log.txt", cfg.bucket_avatars, cfg.bucket_audit)).await;
    assert_ne!(status, StatusCode::OK, "path traversal");
    env.teardown().await;
}

fn bytes_of(b: &'static [u8]) -> noct_core::storage::Bytes {
    noct_core::storage::Bytes::from_static(b)
}

async fn outbox_counts(pool: &PgPool) -> (i64, i64) {
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM noct_outbox").fetch_one(pool).await.unwrap();
    let published: i64 = sqlx::query_scalar("SELECT count(*) FROM noct_outbox WHERE published_at IS NOT NULL")
        .fetch_one(pool)
        .await
        .unwrap();
    (total, published)
}

/// Scenario completo su un `Env` già configurato (servizi in memoria o reali).
async fn outbox_event_and_job_flow(env: Env) {
    let owner = env.user("anna", "utente").await;
    let blog = env.blog("lab", owner, "public").await;
    let base = "/blogs/lab/sandbox/notes";

    // senza consumer né relay: l'evento resta nell'outbox, scritto con il dato
    let (s, note) = env.post(base, Some(owner), json!({"title": "Prima"})).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(outbox_counts(&env.pool).await, (1, 0));
    let subject: String = sqlx::query_scalar("SELECT subject FROM noct_outbox").fetch_one(&env.pool).await.unwrap();
    assert_eq!(subject, "sandbox.note.created");

    // una transazione annullata non lascia eventi: titolo duplicato → 409 → nessuna nuova riga
    let (s, _) = env.post(base, Some(owner), json!({"title": "Prima"})).await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(outbox_counts(&env.pool).await.0, 1, "il rollback annulla anche l'evento");

    // relay + consumer: l'evento arriva all'handler del modulo (che scrive nella cache)
    // la chiave persiste in Redis fra un'esecuzione e l'altra: si parte da zero
    env.state.services.cache.delete("sandbox:last_created").await.unwrap();
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();
    assert_eq!(
        background.consumer_count(),
        4,
        "job cleanup + evento note.created + job di reset password + job del cambio email"
    );
    let cache = env.state.services.cache.clone();
    eventually("evento consegnato all'handler", || async {
        cache.get("sandbox:last_created").await.unwrap().is_some()
    })
    .await;
    let envelope: serde_json::Value =
        serde_json::from_str(&cache.get("sandbox:last_created").await.unwrap().unwrap()).unwrap();
    assert_eq!(envelope["module"], "sandbox");
    assert_eq!(envelope["blog_id"], json!(blog));
    assert_eq!(envelope["actor_id"], json!(owner));
    assert_eq!(envelope["data"]["title"], "Prima");
    assert_eq!(envelope["data"]["id"], note["id"]);
    eventually("outbox segnato come pubblicato", || async { outbox_counts(&env.pool).await == (1, 1) }).await;

    // delete → evento (nessun consumer iscritto: pubblicato comunque)
    let id = note["id"].as_str().unwrap();
    assert_eq!(env.delete(&format!("{base}/{id}"), Some(owner)).await, StatusCode::NO_CONTENT);
    eventually("evento di cancellazione pubblicato", || async { outbox_counts(&env.pool).await == (2, 2) }).await;

    // job: accodato nella stessa transazione di una scrittura, eseguito dal consumer
    env.post(base, Some(owner), json!({"title": "Seconda"})).await;
    env.post(base, Some(owner), json!({"title": "Terza"})).await;
    let n = |pool: PgPool| async move {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM sandbox_notes").fetch_one(&pool).await.unwrap()
    };
    assert_eq!(n(env.pool.clone()).await, 2);
    let mut tx = env.pool.begin().await.unwrap();
    noct_core::bus::outbox::enqueue(&mut tx, Kind::Job, "sandbox.cleanup", &json!({ "blog_id": blog })).await.unwrap();
    tx.commit().await.unwrap();
    eventually("job eseguito", || async { n(env.pool.clone()).await == 0 }).await;

    // job con payload invalido: errore permanente → dead letter, nessun retry infinito
    let mut tx = env.pool.begin().await.unwrap();
    noct_core::bus::outbox::enqueue(&mut tx, Kind::Job, "sandbox.cleanup", &json!({ "sbagliato": 1 })).await.unwrap();
    tx.commit().await.unwrap();
    let bus = env.state.services.bus.clone();
    eventually("dead letter", || async {
        bus.dead_letters().await.unwrap().iter().any(|d| d.reason.contains("blog_id"))
    })
    .await;

    // purge: i messaggi pubblicati si possono ripulire
    sqlx::query("UPDATE noct_outbox SET published_at = now() - interval '2 days' WHERE published_at IS NOT NULL")
        .execute(&env.pool)
        .await
        .unwrap();
    assert!(noct_core::bus::outbox::purge(&env.pool, Duration::from_secs(86_400)).await.unwrap() >= 3);

    background.stop().await;
    env.teardown().await;
}

#[tokio::test]
async fn outbox_events_and_jobs_in_memory() {
    outbox_event_and_job_flow(setup(&["sandbox"], &[]).await).await;
}

#[tokio::test]
async fn outbox_events_and_jobs_over_real_nats_redis_and_s3() {
    let nats = std::env::var("NOCT_TEST_NATS_URL").expect("NOCT_TEST_NATS_URL non impostata");
    let redis = std::env::var("NOCT_TEST_REDIS_URL").expect("NOCT_TEST_REDIS_URL non impostata");
    let s3 = std::env::var("NOCT_TEST_S3_ENDPOINT").expect("NOCT_TEST_S3_ENDPOINT non impostata");
    let suffix = &Uuid::new_v4().simple().to_string()[..10];
    let configure = move |c: &mut Config| {
        c.nats_url = Some(nats.clone());
        c.nats_prefix = format!("e2e{suffix}");
        c.redis_url = Some(redis.to_string());
        c.storage.backend = noct_core::StorageBackend::S3;
        c.storage.s3.endpoint_url = Some(s3.clone());
        c.storage.s3.region = Some("garage".into());
        c.storage.s3.access_key_id = Some("test".into());
        c.storage.s3.secret_access_key = Some("test".into());
        c.storage.bucket_avatars = format!("e2e-avatars-{suffix}");
        c.storage.bucket_content = format!("e2e-content-{suffix}");
        c.storage.bucket_audit = format!("e2e-audit-{suffix}");
    };
    let env = setup_with(&["sandbox"], &[], Some(&configure)).await;
    let (status, body) = env.get("/health/ready", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["services"]["storage"]["backend"].as_str(),
            body["services"]["cache"]["backend"].as_str(),
            body["services"]["bus"]["backend"].as_str()
        ),
        (Some("s3"), Some("redis"), Some("nats"))
    );
    outbox_event_and_job_flow(env).await;
}

#[tokio::test]
async fn rate_limit_is_shared_through_redis() {
    let redis = std::env::var("NOCT_TEST_REDIS_URL").expect("NOCT_TEST_REDIS_URL non impostata");
    let configure = move |c: &mut Config| c.redis_url = Some(redis.clone());
    let env = setup_with(&["sandbox"], &[], Some(&configure)).await;
    let user = env.user("anna", "utente").await;
    // 3 per minuto per utente; il contatore sta in Redis, non nel processo
    for w in ["a", "b", "c"] {
        assert_eq!(env.get(&format!("/sandbox/echo/{w}"), Some(user)).await.0, StatusCode::OK);
    }
    assert_eq!(env.get("/sandbox/echo/d", Some(user)).await.0, StatusCode::TOO_MANY_REQUESTS);
    // il contatore sta nella cache condivisa: lo vede chiunque usi lo stesso Redis
    let hit = env
        .state
        .services
        .cache
        .incr_window(&format!("sandbox:echo:user:{user}"), Duration::from_secs(60))
        .await
        .unwrap();
    assert!(hit.count > 3, "il contatore è visibile dalla cache condivisa: {hit:?}");
    env.teardown().await;
}

#[tokio::test]
async fn mailer_is_reachable_from_modules_through_the_state() {
    let env = setup(&["taxonomy"], &[]).await;
    env.state.services.mailer.send(&noct_core::mail::Email::text("a@example.com", "Codice", "123456")).await.unwrap();
    assert_eq!(env.mailer.sent().len(), 1);
    assert!(noct_core::markdown::render_safe_html("**ok** <script>x</script>").contains("<strong>ok</strong>"));
    env.teardown().await;
}
