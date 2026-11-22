//! L'elenco degli interessi si modifica da `PATCH /admin/config` (Super Admin).

use async_trait::async_trait;
use noct_core::{AdminSection, ApiError, AppState, JsonMap};
use serde_json::{Value, json};
use sqlx::PgConnection;
use std::{
    collections::HashSet,
    sync::{Arc, LazyLock},
};

const MAX_PLATFORM_INTERESTS: usize = 200;

static KEY_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[a-z0-9_-]{1,40}$").expect("regex statica"));

pub fn all() -> Vec<Arc<dyn AdminSection>> {
    vec![Arc::new(Interests)]
}

/// Chiave canonica univoca in formato slug e almeno una traduzione non vuota per voce.
pub fn validate_list(items: &[Value]) -> Result<Vec<Value>, ApiError> {
    if items.len() > MAX_PLATFORM_INTERESTS {
        return Err(ApiError::bad_request(format!("Al massimo {MAX_PLATFORM_INTERESTS} interessi.")));
    }
    let mut seen = HashSet::new();
    let mut cleaned = Vec::new();
    for item in items {
        let key = item.get("key").and_then(Value::as_str).unwrap_or_default().trim().to_lowercase();
        if !KEY_RE.is_match(&key) {
            return Err(ApiError::bad_request(format!("Chiave interesse non valida: '{key}'.")));
        }
        if !seen.insert(key.clone()) {
            return Err(ApiError::bad_request(format!("Chiave interesse duplicata: '{key}'.")));
        }
        let translations: serde_json::Map<String, Value> = item
            .get("translations")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter_map(|(loc, label)| {
                        let label = label.as_str()?.trim();
                        (!label.is_empty()).then(|| (loc.trim().to_lowercase(), json!(label)))
                    })
                    .collect()
            })
            .unwrap_or_default();
        if translations.is_empty() {
            return Err(ApiError::bad_request(format!("L'interesse '{key}' non ha nessuna traduzione.")));
        }
        cleaned.push(json!({"key": key, "translations": translations}));
    }
    Ok(cleaned)
}

struct Interests;

#[async_trait]
impl AdminSection for Interests {
    fn module(&self) -> &'static str {
        "interests"
    }

    fn settings_keys(&self) -> &'static [&'static str] {
        &["interests"]
    }

    async fn settings(&self, st: &AppState) -> Result<JsonMap, ApiError> {
        let items: Option<Value> =
            sqlx::query_scalar("SELECT items FROM interests_settings WHERE id = 1").fetch_optional(&st.db).await?;
        let mut out = JsonMap::new();
        out.insert("interests".into(), items.filter(Value::is_array).unwrap_or_else(|| json!([])));
        Ok(out)
    }

    async fn apply_settings(
        &self,
        conn: &mut PgConnection,
        _st: &AppState,
        patch: &JsonMap,
    ) -> Result<JsonMap, ApiError> {
        let Some(Value::Array(items)) = patch.get("interests") else {
            return Err(ApiError::bad_request("`interests` deve essere un elenco."));
        };
        let cleaned = validate_list(items)?;
        let old: Value = sqlx::query_scalar("SELECT items FROM interests_settings WHERE id = 1 FOR UPDATE")
            .fetch_optional(&mut *conn)
            .await?
            .unwrap_or_else(|| json!([]));
        let new = Value::Array(cleaned);
        if old == new {
            return Ok(JsonMap::new());
        }
        sqlx::query(
            "INSERT INTO interests_settings (id, items) VALUES (1, $1) ON CONFLICT (id) DO UPDATE SET items = EXCLUDED.items",
        )
        .bind(&new)
        .execute(&mut *conn)
        .await?;
        // le chiavi tolte dall'elenco spariscono anche dalle scelte già salvate: sarebbero orfane
        let keep: Vec<String> =
            new.as_array().into_iter().flatten().filter_map(|i| i["key"].as_str().map(str::to_string)).collect();
        sqlx::query(
            "UPDATE users SET interests = ARRAY(SELECT k FROM unnest(interests) k WHERE k = ANY($1)), updated_at = now()
             WHERE NOT interests <@ $1::text[]",
        )
        .bind(&keep)
        .execute(&mut *conn)
        .await?;
        let mut changes = JsonMap::new();
        changes.insert("interests".into(), json!({"from": old, "to": new}));
        Ok(changes)
    }
}
