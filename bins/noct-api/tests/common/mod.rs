//! Ambiente comune dei test end-to-end: router reale, Postgres vero (un database per test),
//! servizi in memoria o reali. Richiede `NOCT_TEST_DATABASE_URL`.
#![allow(dead_code)]

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use noct_core::{AppState, Config, Registry, Services};
use serde_json::Value;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

pub struct Env {
    pub app: Router,
    pub pool: PgPool,
    pub db_name: String,
    pub admin: PgPool,
    pub state: AppState,
    pub registry: Registry,
    pub enabled: Vec<usize>,
    pub storage_dir: std::path::PathBuf,
    pub mailer: std::sync::Arc<noct_core::mail::MemoryMailer>,
}

pub async fn setup(modules: &[&str], env: &[(&str, &str)]) -> Env {
    setup_with(modules, env, None).await
}

/// `config`: configurazione dei servizi (Redis, NATS, S3...) al posto dei servizi in memoria.
pub async fn setup_with(modules: &[&str], env: &[(&str, &str)], infra: Option<&dyn Fn(&mut Config)>) -> Env {
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

    let mut config = Config::minimal(url);
    config.modules = Some(requested);
    let storage_dir = std::env::temp_dir().join(format!("noct-e2e-{}", Uuid::new_v4().simple()));
    config.storage.local_base_path = storage_dir.clone();
    let (mut services, mailer) = Services::in_memory(&storage_dir);
    if let Some(configure) = infra {
        configure(&mut config);
        services = Services::connect(&config).await.unwrap_or_else(|e| panic!("{e}"));
    }
    let extensions = registry.extensions(&enabled).unwrap_or_else(|e| panic!("{e}"));
    let state = AppState::new(pool.clone(), config, services, module_cfg).with_extensions(extensions);
    registry.run_startup(&enabled, &state).await.unwrap_or_else(|e| panic!("{e}"));
    let app = noct_api::app(&registry, &enabled, state.clone());
    Env { app, pool, db_name, admin, state, registry, enabled, storage_dir, mailer }
}

impl Env {
    pub async fn teardown(self) {
        let _ = std::fs::remove_dir_all(&self.storage_dir);
        self.pool.close().await;
        sqlx::query(&format!("DROP DATABASE {} WITH (FORCE)", self.db_name)).execute(&self.admin).await.unwrap();
    }

    /// Invia una richiesta già costruita; restituisce status, header e corpo JSON (Null se vuoto).
    pub async fn send(&self, req: Request<Body>) -> (StatusCode, axum::http::HeaderMap, Value) {
        let res = self.app.clone().oneshot(req).await.unwrap();
        let (status, headers) = (res.status(), res.headers().clone());
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let json = if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).unwrap_or(Value::Null) };
        (status, headers, json)
    }

    pub async fn call(
        &self,
        method: Method,
        path: &str,
        who: Option<Uuid>,
        body: Option<Value>,
    ) -> (StatusCode, Value, axum::http::HeaderMap) {
        let mut req = Request::builder().method(method).uri(format!("/api/v1{path}"));
        if let Some(u) = who {
            req = req.header(header::AUTHORIZATION, format!("Bearer {}", self.access_token(u)));
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

    pub async fn get(&self, path: &str, who: Option<Uuid>) -> (StatusCode, Value) {
        let (s, j, _) = self.call(Method::GET, path, who, None).await;
        (s, j)
    }
    pub async fn post(&self, path: &str, who: Option<Uuid>, body: Value) -> (StatusCode, Value) {
        let (s, j, _) = self.call(Method::POST, path, who, Some(body)).await;
        (s, j)
    }
    pub async fn patch(&self, path: &str, who: Option<Uuid>, body: Value) -> (StatusCode, Value) {
        let (s, j, _) = self.call(Method::PATCH, path, who, Some(body)).await;
        (s, j)
    }
    pub async fn delete_json(&self, path: &str, who: Option<Uuid>, body: Value) -> (StatusCode, Value) {
        let (s, j, _) = self.call(Method::DELETE, path, who, Some(body)).await;
        (s, j)
    }
    pub async fn put(&self, path: &str, who: Option<Uuid>, body: Value) -> (StatusCode, Value) {
        let (s, j, _) = self.call(Method::PUT, path, who, Some(body)).await;
        (s, j)
    }
    pub async fn delete(&self, path: &str, who: Option<Uuid>) -> StatusCode {
        self.call(Method::DELETE, path, who, None).await.0
    }

    /// Access token valido per `user` (come lo emetterebbe il login).
    pub fn access_token(&self, user: Uuid) -> String {
        noct_core::security::jwt::encode(
            self.state.config.jwt_secret.as_bytes(),
            "access",
            user,
            chrono::Utc::now().timestamp(),
            900,
        )
    }

    pub async fn user(&self, name: &str, role: &str) -> Uuid {
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
    pub async fn blog(&self, slug: &str, owner: Uuid, visibility: &str) -> Uuid {
        sqlx::query_scalar("INSERT INTO blogs (slug, title, owner_id, visibility) VALUES ($1, $1, $2, $3::blog_visibility) RETURNING id")
            .bind(slug)
            .bind(owner)
            .bind(visibility)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
    pub async fn member(&self, blog: Uuid, user: Uuid, role: &str) {
        sqlx::query("INSERT INTO blog_memberships (blog_id, user_id, role) VALUES ($1, $2, $3::blog_role)")
            .bind(blog)
            .bind(user)
            .bind(role)
            .execute(&self.pool)
            .await
            .unwrap();
    }
}

/// `(status, corpo)` di una risposta di `Env::call`.
pub trait IntoPair {
    fn into_pair(self) -> (StatusCode, Value);
}
impl IntoPair for (StatusCode, Value, axum::http::HeaderMap) {
    fn into_pair(self) -> (StatusCode, Value) {
        (self.0, self.1)
    }
}

pub fn detail(v: &Value) -> &str {
    v["detail"].as_str().unwrap_or_default()
}

/// Valore del cookie `name` tra gli header `Set-Cookie` di una risposta.
pub fn set_cookie_value(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|c| c.strip_prefix(&format!("{name}=")).map(|rest| rest.split(';').next().unwrap_or("").to_string()))
}

/// L'intero header `Set-Cookie` del cookie `name` (con gli attributi).
pub fn set_cookie_header(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|c| c.starts_with(&format!("{name}=")))
        .map(str::to_string)
}
