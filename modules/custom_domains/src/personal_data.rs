//! Dati personali del modulo: sezioni dell'esportazione e cancellazione (modulo `gdpr`).

use async_trait::async_trait;
use noct_core::{ApiError, AppState, JsonMap, PersonalData, StoredObject};
use serde_json::{Value, json};
use sqlx::PgConnection;
use std::sync::Arc;
use uuid::Uuid;

pub fn all() -> Vec<Arc<dyn PersonalData>> {
    vec![Arc::new(Provider)]
}

struct Provider;

fn sections(v: Value) -> JsonMap {
    v.as_object().cloned().unwrap_or_default()
}

#[async_trait]
impl PersonalData for Provider {
    fn module(&self) -> &'static str {
        "custom_domains"
    }

    async fn export(&self, st: &AppState, user: Uuid) -> Result<JsonMap, ApiError> {
        // anche la richiesta non ancora verificata, mai il token di verifica (un segreto operativo)
        let row: Option<(String, String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)> =
            sqlx::query_as(
                "SELECT domain, status::text, created_at, verified_at FROM custom_domains WHERE user_id = $1",
            )
            .bind(user)
            .fetch_optional(&st.db)
            .await?;
        let claim = row.map(|(domain, status, created_at, verified_at)| {
            json!({"domain": domain, "status": status, "created_at": created_at, "verified_at": verified_at})
        });
        Ok(sections(json!({"account": {"custom_domain_claim": claim}})))
    }

    async fn erase(&self, conn: &mut PgConnection, _: &AppState, user: Uuid) -> Result<Vec<StoredObject>, ApiError> {
        sqlx::query("DELETE FROM custom_domains WHERE user_id = $1").bind(user).execute(&mut *conn).await?;
        Ok(Vec::new())
    }
}
