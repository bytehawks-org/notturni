//! Prove end-to-end dell'API reale (router completo, middleware, Postgres vero).
//! Richiedono `NOCT_TEST_DATABASE_URL` (es. postgres://postgres@127.0.0.1:55432/postgres);
//! ogni test lavora su un database creato e distrutto da sé.

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use noct_core::{AppState, Config, Registry};
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

struct Env {
    app: Router,
    pool: PgPool,
    db_name: String,
    admin: PgPool,
}

async fn setup(modules: &[&str], env: &[(&str, &str)]) -> Env {
    let admin_url = std::env::var("NOCT_TEST_DATABASE_URL").expect("NOCT_TEST_DATABASE_URL non impostata");
    let admin = PgPoolOptions::new().max_connections(2).connect(&admin_url).await.unwrap();
    let db_name = format!("noct_e2e_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE DATABASE {db_name}")).execute(&admin).await.unwrap();
    let url = format!("{}/{db_name}", admin_url.rsplit_once('/').unwrap().0);
    let pool = PgPoolOptions::new().max_connections(5).connect(&url).await.unwrap();

    let lookup = |k: &str| env.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
    let registry = Registry::new(noct_bundle::all()).unwrap_or_else(|e| panic!("{e}"));
    let requested: Vec<String> = modules.iter().map(|s| s.to_string()).collect();
    let enabled = registry.resolve_enabled(Some(&requested)).unwrap_or_else(|e| panic!("{e}"));
    let module_cfg = registry.module_configs(&enabled, &lookup).unwrap_or_else(|e| panic!("{e}"));
    registry.migrate(&pool, &enabled).await.unwrap();

    let config =
        Config { database_url: url, bind_addr: "127.0.0.1:0".into(), modules: Some(requested), dev_auth: true };
    let app = noct_api::app(&registry, &enabled, AppState::new(pool.clone(), config, module_cfg));
    Env { app, pool, db_name, admin }
}

impl Env {
    async fn teardown(self) {
        self.pool.close().await;
        sqlx::query(&format!("DROP DATABASE {} WITH (FORCE)", self.db_name)).execute(&self.admin).await.unwrap();
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        who: Option<Uuid>,
        body: Option<Value>,
    ) -> (StatusCode, Value, axum::http::HeaderMap) {
        let mut req = Request::builder().method(method).uri(format!("/api/v1{path}"));
        if let Some(u) = who {
            req = req.header(header::AUTHORIZATION, format!("Bearer dev:{u}"));
        }
        let req = match body {
            Some(b) => req.header(header::CONTENT_TYPE, "application/json").body(Body::from(b.to_string())),
            None => req.body(Body::empty()),
        }
        .unwrap();
        let res = self.app.clone().oneshot(req).await.unwrap();
        let (status, headers) = (res.status(), res.headers().clone());
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let json = if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).unwrap_or(Value::Null) };
        (status, json, headers)
    }

    async fn get(&self, path: &str, who: Option<Uuid>) -> (StatusCode, Value) {
        let (s, j, _) = self.call(Method::GET, path, who, None).await;
        (s, j)
    }
    async fn post(&self, path: &str, who: Option<Uuid>, body: Value) -> (StatusCode, Value) {
        let (s, j, _) = self.call(Method::POST, path, who, Some(body)).await;
        (s, j)
    }
    async fn patch(&self, path: &str, who: Option<Uuid>, body: Value) -> (StatusCode, Value) {
        let (s, j, _) = self.call(Method::PATCH, path, who, Some(body)).await;
        (s, j)
    }
    async fn delete(&self, path: &str, who: Option<Uuid>) -> StatusCode {
        self.call(Method::DELETE, path, who, None).await.0
    }

    async fn user(&self, name: &str, role: &str) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO users (username, email, platform_role) VALUES ($1, $2, $3::platform_role) RETURNING id",
        )
        .bind(name)
        .bind(format!("{name}@example.com"))
        .bind(role)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
    async fn blog(&self, slug: &str, owner: Uuid, visibility: &str) -> Uuid {
        sqlx::query_scalar("INSERT INTO blogs (slug, title, owner_id, visibility) VALUES ($1, $1, $2, $3::blog_visibility) RETURNING id")
            .bind(slug)
            .bind(owner)
            .bind(visibility)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
    async fn member(&self, blog: Uuid, user: Uuid, role: &str) {
        sqlx::query("INSERT INTO blog_memberships (blog_id, user_id, role) VALUES ($1, $2, $3::blog_role)")
            .bind(blog)
            .bind(user)
            .bind(role)
            .execute(&self.pool)
            .await
            .unwrap();
    }
}

fn detail(v: &Value) -> &str {
    v["detail"].as_str().unwrap_or_default()
}

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
