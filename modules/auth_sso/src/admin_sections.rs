//! Provider di accesso consentiti dall'amministratore (`sso_providers`) e quelli configurati
//! nell'ambiente (`sso_configured`, sola lettura).

use crate::providers::Provider;
use async_trait::async_trait;
use noct_core::{AdminSection, ApiError, AppState, JsonMap};
use serde_json::{Value, json};
use sqlx::PgConnection;
use std::sync::Arc;

pub fn all() -> Vec<Arc<dyn AdminSection>> {
    vec![Arc::new(SsoAdmin)]
}

fn configured(st: &AppState) -> Vec<&'static str> {
    let set = |key: String| st.module_config_str("auth_sso", &key).is_some();
    let mut names: Vec<&'static str> = Provider::ALL
        .into_iter()
        .filter(|p| set(format!("{}_client_id", p.name())) && set(format!("{}_client_secret", p.name())))
        .map(Provider::name)
        .collect();
    names.sort_unstable();
    names
}

struct SsoAdmin;

#[async_trait]
impl AdminSection for SsoAdmin {
    fn module(&self) -> &'static str {
        "auth_sso"
    }

    async fn public_config(&self, st: &AppState) -> Result<JsonMap, ApiError> {
        let enabled: Option<Vec<String>> =
            sqlx::query_scalar("SELECT enabled_providers FROM sso_settings WHERE id = 1")
                .fetch_optional(&st.db)
                .await?;
        let enabled = enabled.unwrap_or_default();
        // vuoto = tutti quelli configurati
        let names: Vec<&str> =
            configured(st).into_iter().filter(|n| enabled.is_empty() || enabled.iter().any(|e| e == n)).collect();
        let mut out = JsonMap::new();
        out.insert("sso_providers".into(), json!(names));
        Ok(out)
    }

    fn settings_keys(&self) -> &'static [&'static str] {
        &["sso_providers"]
    }

    async fn settings(&self, st: &AppState) -> Result<JsonMap, ApiError> {
        let enabled: Option<Vec<String>> =
            sqlx::query_scalar("SELECT enabled_providers FROM sso_settings WHERE id = 1")
                .fetch_optional(&st.db)
                .await?;
        let mut out = JsonMap::new();
        out.insert("sso_providers".into(), json!(enabled.unwrap_or_default()));
        out.insert("sso_configured".into(), json!(configured(st)));
        Ok(out)
    }

    async fn apply_settings(
        &self,
        conn: &mut PgConnection,
        _st: &AppState,
        patch: &JsonMap,
    ) -> Result<JsonMap, ApiError> {
        let Some(Value::Array(items)) = patch.get("sso_providers") else {
            return Err(ApiError::bad_request("`sso_providers` deve essere un elenco."));
        };
        let mut names: Vec<String> = items.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
        let unknown: Vec<&str> = names.iter().filter(|n| Provider::parse(n).is_none()).map(String::as_str).collect();
        if !unknown.is_empty() || names.len() != items.len() {
            return Err(ApiError::bad_request(format!("Provider sconosciuti: {}.", unknown.join(", "))));
        }
        names.sort();
        names.dedup();
        let old: Vec<String> = sqlx::query_scalar("SELECT enabled_providers FROM sso_settings WHERE id = 1 FOR UPDATE")
            .fetch_optional(&mut *conn)
            .await?
            .unwrap_or_default();
        sqlx::query(
            "INSERT INTO sso_settings (id, enabled_providers) VALUES (1, $1)
             ON CONFLICT (id) DO UPDATE SET enabled_providers = EXCLUDED.enabled_providers",
        )
        .bind(&names)
        .execute(&mut *conn)
        .await?;
        let mut changes = JsonMap::new();
        if old != names {
            changes.insert("sso_providers".into(), json!({"from": old, "to": names}));
        }
        Ok(changes)
    }
}
