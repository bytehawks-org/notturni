//! Coda delle segnalazioni aperte nella panoramica di amministrazione.

use async_trait::async_trait;
use noct_core::{AdminSection, ApiError, AppState, JsonMap};
use serde_json::json;
use std::sync::Arc;

pub fn all() -> Vec<Arc<dyn AdminSection>> {
    vec![Arc::new(ReportsAdmin)]
}

struct ReportsAdmin;

#[async_trait]
impl AdminSection for ReportsAdmin {
    fn module(&self) -> &'static str {
        "reports"
    }

    async fn overview(&self, st: &AppState) -> Result<JsonMap, ApiError> {
        let open: i64 =
            sqlx::query_scalar("SELECT count(*) FROM content_reports WHERE status = 'open'").fetch_one(&st.db).await?;
        let mut out = JsonMap::new();
        out.insert("queue_open_reports".into(), json!(open));
        Ok(out)
    }
}
