//! Elenchi del sigillo di verifica (oro e argento per identificatore, blu per dominio email). Il
//! livello si calcola alla lettura: cambiare un elenco ha effetto subito, senza ricalcoli.

use async_trait::async_trait;
use noct_core::{AdminSection, ApiError, AppState, JsonMap};
use serde_json::{Value, json};
use sqlx::PgConnection;
use std::sync::{Arc, LazyLock};

static DOMAIN_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$").expect("regex statica")
});

pub fn all() -> Vec<Arc<dyn AdminSection>> {
    vec![Arc::new(VerificationAdmin)]
}

const KEYS: [&str; 3] =
    ["verification_gold_identifiers", "verification_silver_identifiers", "verification_blue_domains"];

struct VerificationAdmin;

fn list(patch: &JsonMap, key: &str) -> Result<Option<Vec<String>>, ApiError> {
    let Some(v) = patch.get(key) else { return Ok(None) };
    let Value::Array(items) = v else {
        return Err(ApiError::bad_request(format!("`{key}` deve essere un elenco.")));
    };
    let mut out: Vec<String> = items
        .iter()
        .map(|i| {
            i.as_str()
                .map(|s| s.trim().to_lowercase())
                .ok_or_else(|| ApiError::bad_request(format!("`{key}`: solo stringhe.")))
        })
        .collect::<Result<_, _>>()?;
    out.retain(|s| !s.is_empty());
    out.sort();
    out.dedup();
    Ok(Some(out))
}

#[async_trait]
impl AdminSection for VerificationAdmin {
    fn module(&self) -> &'static str {
        "profiles"
    }

    fn settings_keys(&self) -> &'static [&'static str] {
        &KEYS
    }

    async fn settings(&self, st: &AppState) -> Result<JsonMap, ApiError> {
        let l = crate::view::verification_lists(st).await?;
        let mut out = JsonMap::new();
        out.insert(KEYS[0].into(), json!(l.gold));
        out.insert(KEYS[1].into(), json!(l.silver));
        out.insert(KEYS[2].into(), json!(l.blue_domains));
        Ok(out)
    }

    async fn apply_settings(
        &self,
        conn: &mut PgConnection,
        _st: &AppState,
        patch: &JsonMap,
    ) -> Result<JsonMap, ApiError> {
        let (gold, silver, blue) = (list(patch, KEYS[0])?, list(patch, KEYS[1])?, list(patch, KEYS[2])?);
        if let Some(domains) = &blue {
            let bad: Vec<&str> = domains.iter().filter(|d| !DOMAIN_RE.is_match(d)).map(String::as_str).collect();
            if !bad.is_empty() {
                return Err(ApiError::bad_request(format!("Domini non validi: {}.", bad.join(", "))));
            }
        }
        sqlx::query("INSERT INTO verification_settings (id) VALUES (1) ON CONFLICT (id) DO NOTHING")
            .execute(&mut *conn)
            .await?;
        let (og, os, ob): (Vec<String>, Vec<String>, Vec<String>) = sqlx::query_as(
            "SELECT gold_identifiers, silver_identifiers, blue_domains FROM verification_settings WHERE id = 1 FOR UPDATE",
        )
        .fetch_one(&mut *conn)
        .await?;
        let (ng, ns, nb) = (
            gold.unwrap_or_else(|| og.clone()),
            silver.unwrap_or_else(|| os.clone()),
            blue.unwrap_or_else(|| ob.clone()),
        );
        sqlx::query("UPDATE verification_settings SET gold_identifiers = $1, silver_identifiers = $2, blue_domains = $3 WHERE id = 1")
            .bind(&ng)
            .bind(&ns)
            .bind(&nb)
            .execute(&mut *conn)
            .await?;
        let mut changes = JsonMap::new();
        for (key, old, new) in [(KEYS[0], og, ng), (KEYS[1], os, ns), (KEYS[2], ob, nb)] {
            if old != new {
                changes.insert(key.into(), json!({"from": old, "to": new}));
            }
        }
        Ok(changes)
    }
}
