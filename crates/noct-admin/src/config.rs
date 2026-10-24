//! `GET|PATCH /admin/config`: le impostazioni del core più quelle che i moduli attivi dichiarano con
//! `AdminSection`.

use crate::handlers_impl::{actor_id, audit};
use chrono::{DateTime, Utc};
use noct_core::{ApiError, JsonMap, RouteCtx, StorageBackend, body_map, revalidate};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{FromRow, PgConnection};
use std::sync::LazyLock;

const LOCALES: &[&str] = &["it", "en"];
const REGISTRATION_MODES: &[&str] = &["open", "invite", "closed"];
const MIN_RETENTION_DAYS: i32 = 7;
const MAX_RETENTION_DAYS: i32 = 3650;
const MAX_FOOTER_CHARS: usize = 5000;
const MAX_STORAGE_MB: i64 = 1_048_576;
const FOOTERS: [&str; 4] =
    ["footer_column1_markdown", "footer_column2_markdown", "footer_column3_markdown", "footer_bottom_bar_markdown"];

static SLUG_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$").expect("regex statica"));

#[derive(FromRow, Clone)]
struct Row {
    default_locale: String,
    registration_mode: String,
    mfa_required_for_admins: bool,
    reserved_blog_names: Vec<String>,
    moderation_threshold: f64,
    max_blogs_per_user: i32,
    audit_retention_days: i32,
    footer_column1_markdown: Option<String>,
    footer_column2_markdown: Option<String>,
    footer_column3_markdown: Option<String>,
    footer_bottom_bar_markdown: Option<String>,
    max_blog_storage_mb: Option<i32>,
    updated_at: Option<DateTime<Utc>>,
}

const SELECT: &str = "SELECT default_locale, registration_mode, mfa_required_for_admins, reserved_blog_names, moderation_threshold,
        max_blogs_per_user, audit_retention_days, footer_column1_markdown, footer_column2_markdown, footer_column3_markdown,
        footer_bottom_bar_markdown, max_blog_storage_mb, updated_at FROM platform_config WHERE id = 1";

async fn load(conn: &mut PgConnection, lock: bool) -> Result<Row, ApiError> {
    sqlx::query("INSERT INTO platform_config (id) VALUES (1) ON CONFLICT (id) DO NOTHING").execute(&mut *conn).await?;
    Ok(sqlx::query_as(&format!("{SELECT}{}", if lock { " FOR UPDATE" } else { "" })).fetch_one(conn).await?)
}

async fn render(ctx: &RouteCtx, r: &Row) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let storage = &st.config.storage;
    let mut reserved: Vec<&str> = noct_blogs::rules::RESERVED_BLOG_SLUGS.to_vec();
    reserved.sort_unstable();
    let mut out = json!({
        "default_locale": r.default_locale, "registration_mode": r.registration_mode,
        "mfa_required_for_admins": r.mfa_required_for_admins, "reserved_blog_names": r.reserved_blog_names,
        "reserved_builtin": reserved, "moderation_threshold": r.moderation_threshold,
        "max_blogs_per_user": r.max_blogs_per_user, "audit_retention_days": r.audit_retention_days,
        "footer_column1_markdown": r.footer_column1_markdown, "footer_column2_markdown": r.footer_column2_markdown,
        "footer_column3_markdown": r.footer_column3_markdown, "footer_bottom_bar_markdown": r.footer_bottom_bar_markdown,
        "max_blog_storage_mb": r.max_blog_storage_mb, "updated_at": r.updated_at,
        // sola lettura: riepilogo dell'ambiente (mai segreti)
        "infrastructure": {
            "deployment_mode": st.module_config_str("auth", "deployment_mode").unwrap_or("platform"),
            "instance_fqdn": st.config.instance_fqdn,
            "storage_backend": if storage.backend == StorageBackend::S3 { "s3" } else { "localstorage" },
            "s3_endpoint_url": if storage.backend == StorageBackend::S3 { storage.s3.endpoint_url.clone() } else { None },
            "moderation_service": st.config.moderation_service_url.is_some(),
            "smtp": st.config.smtp.is_some(),
            "nats": st.config.nats_url.is_some(),
            "redis": st.config.redis_url.is_some(),
        },
    });
    out.as_object_mut().expect("oggetto").extend(st.admin_sections.settings(st).await?);
    Ok(out)
}

pub async fn get_config(ctx: RouteCtx) -> Result<Value, ApiError> {
    let mut conn = ctx.state.db.acquire().await?;
    let row = load(&mut conn, false).await?;
    render(&ctx, &row).await
}

// ---- lettura dei campi ----------------------------------------------------------------------------------------------

fn text(body: &JsonMap, key: &str) -> Result<Option<String>, ApiError> {
    match body.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(ApiError::bad_request(format!("`{key}` deve essere una stringa."))),
    }
}

fn int(body: &JsonMap, key: &str) -> Result<Option<i64>, ApiError> {
    match body.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_i64().map(Some).ok_or_else(|| ApiError::bad_request(format!("`{key}` deve essere un intero."))),
    }
}

fn boolean(body: &JsonMap, key: &str) -> Result<Option<bool>, ApiError> {
    match body.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(ApiError::bad_request(format!("`{key}` deve essere booleano."))),
    }
}

fn list(body: &JsonMap, key: &str) -> Result<Option<Vec<String>>, ApiError> {
    match body.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_str().map(str::to_string).ok_or_else(|| ApiError::bad_request(format!("`{key}`: solo stringhe.")))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Some(_) => Err(ApiError::bad_request(format!("`{key}` deve essere un elenco."))),
    }
}

/// `{campo: {from, to}}` per ciò che è davvero cambiato.
#[derive(Default)]
struct Changes(JsonMap);

impl Changes {
    fn set<T: Serialize + PartialEq>(&mut self, field: &str, slot: &mut T, new: T) {
        if *slot != new {
            self.0.insert(field.into(), json!({"from": slot, "to": new}));
            *slot = new;
        }
    }
}

pub async fn update_config(ctx: RouteCtx, _body: crate::AdminConfigUpdate) -> Result<Value, ApiError> {
    let admin = actor_id(&ctx)?;
    let raw = body_map(&ctx.body);
    let mut tx = ctx.state.db.begin().await?;
    let mut row = load(&mut tx, true).await?;
    let mut ch = Changes::default();

    if let Some(v) = text(&raw, "default_locale")? {
        if !LOCALES.contains(&v.as_str()) {
            return Err(ApiError::bad_request("Lingua non supportata."));
        }
        ch.set("default_locale", &mut row.default_locale, v);
    }
    if let Some(v) = text(&raw, "registration_mode")? {
        if !REGISTRATION_MODES.contains(&v.as_str()) {
            return Err(ApiError::bad_request("Modalità di registrazione non valida."));
        }
        ch.set("registration_mode", &mut row.registration_mode, v);
    }
    if let Some(v) = boolean(&raw, "mfa_required_for_admins")? {
        ch.set("mfa_required_for_admins", &mut row.mfa_required_for_admins, v);
    }
    if let Some(names) = list(&raw, "reserved_blog_names")? {
        let mut cleaned: Vec<String> =
            names.iter().map(|n| n.trim().to_lowercase()).filter(|n| !n.is_empty()).collect();
        cleaned.sort();
        cleaned.dedup();
        let bad: Vec<&str> = cleaned.iter().filter(|n| !SLUG_RE.is_match(n)).map(String::as_str).collect();
        if !bad.is_empty() {
            return Err(ApiError::bad_request(format!("Nomi non validi: {}.", bad.join(", "))));
        }
        ch.set("reserved_blog_names", &mut row.reserved_blog_names, cleaned);
    }
    if let Some(v) = raw.get("moderation_threshold").filter(|v| !v.is_null()) {
        let t = v.as_f64().ok_or_else(|| ApiError::bad_request("`moderation_threshold` deve essere un numero."))?;
        if !(t > 0.0 && t <= 1.0) {
            return Err(ApiError::bad_request("La soglia deve essere tra 0 e 1."));
        }
        ch.set("moderation_threshold", &mut row.moderation_threshold, t);
    }
    if let Some(v) = int(&raw, "max_blogs_per_user")? {
        if !(1..=100).contains(&v) {
            return Err(ApiError::bad_request("Il massimo di blog per utente deve essere tra 1 e 100."));
        }
        ch.set("max_blogs_per_user", &mut row.max_blogs_per_user, v as i32);
    }
    if let Some(v) = int(&raw, "audit_retention_days")? {
        if !(i64::from(MIN_RETENTION_DAYS)..=i64::from(MAX_RETENTION_DAYS)).contains(&v) {
            return Err(ApiError::bad_request(format!(
                "La conservazione del registro di audit deve essere tra {MIN_RETENTION_DAYS} e {MAX_RETENTION_DAYS} giorni."
            )));
        }
        ch.set("audit_retention_days", &mut row.audit_retention_days, v as i32);
    }
    for field in FOOTERS {
        if let Some(v) = text(&raw, field)? {
            if v.chars().count() > MAX_FOOTER_CHARS {
                return Err(ApiError::bad_request(format!(
                    "Il testo del footer supera i {MAX_FOOTER_CHARS} caratteri."
                )));
            }
            // "" azzera (colonna vuota, non mostrata), assente non tocca
            let slot = match field {
                "footer_column1_markdown" => &mut row.footer_column1_markdown,
                "footer_column2_markdown" => &mut row.footer_column2_markdown,
                "footer_column3_markdown" => &mut row.footer_column3_markdown,
                _ => &mut row.footer_bottom_bar_markdown,
            };
            ch.set(field, slot, Some(v).filter(|v| !v.is_empty()));
        }
    }
    if let Some(v) = int(&raw, "max_blog_storage_mb")? {
        // 0 = nessun limite
        let new = match v {
            0 => None,
            1..=MAX_STORAGE_MB => Some(v as i32),
            _ => {
                return Err(ApiError::bad_request(
                    "Lo spazio massimo per blog deve essere tra 1 MB e 1.048.576 MB (1 TB), oppure 0 per nessun limite.",
                ));
            }
        };
        ch.set("max_blog_storage_mb", &mut row.max_blog_storage_mb, new);
    }

    // le impostazioni dei moduli attivi, nella stessa transazione
    ch.0.extend(ctx.state.admin_sections.apply_settings(&mut tx, &ctx.state, &raw).await?);

    if !ch.0.is_empty() {
        sqlx::query(
            "UPDATE platform_config SET default_locale = $1, registration_mode = $2, mfa_required_for_admins = $3,
                    reserved_blog_names = $4, moderation_threshold = $5, max_blogs_per_user = $6, audit_retention_days = $7,
                    footer_column1_markdown = $8, footer_column2_markdown = $9, footer_column3_markdown = $10,
                    footer_bottom_bar_markdown = $11, max_blog_storage_mb = $12, updated_at = now(), updated_by_id = $13
             WHERE id = 1",
        )
        .bind(&row.default_locale)
        .bind(&row.registration_mode)
        .bind(row.mfa_required_for_admins)
        .bind(&row.reserved_blog_names)
        .bind(row.moderation_threshold)
        .bind(row.max_blogs_per_user)
        .bind(row.audit_retention_days)
        .bind(&row.footer_column1_markdown)
        .bind(&row.footer_column2_markdown)
        .bind(&row.footer_column3_markdown)
        .bind(&row.footer_bottom_bar_markdown)
        .bind(row.max_blog_storage_mb)
        .bind(admin)
        .execute(&mut *tx)
        .await?;
        audit(&mut tx, &ctx, "platform.config_updated", None, json!({"target": "platform", "changes": ch.0})).await?;
    }
    let fresh = load(&mut tx, false).await?;
    tx.commit().await?;
    if ch.0.keys().any(|k| k.starts_with("footer_")) {
        // mostrato su ogni pagina pubblica di piattaforma e di ogni blog: un solo tag condiviso
        ctx.state.services.revalidate.spawn(vec![revalidate::platform_footer()]);
    }
    render(&ctx, &fresh).await
}
