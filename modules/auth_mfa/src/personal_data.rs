//! Dati personali del modulo: sezioni dell'esportazione e cancellazione (modulo `gdpr`).

use async_trait::async_trait;
use noct_core::{ApiError, AppState, JsonMap, PersonalData, StoredObject};
use sqlx::PgConnection;
use std::sync::Arc;
use uuid::Uuid;

pub fn all() -> Vec<Arc<dyn PersonalData>> {
    vec![Arc::new(Provider)]
}

struct Provider;

#[async_trait]
impl PersonalData for Provider {
    fn module(&self) -> &'static str {
        "auth_mfa"
    }

    async fn export(&self, _: &AppState, _: Uuid) -> Result<JsonMap, ApiError> {
        Ok(JsonMap::new())
    }

    /// Segreti TOTP e codici in sospeso. `mfa_enabled` lo azzera la cancellazione dell'account.
    async fn erase(&self, conn: &mut PgConnection, _: &AppState, user: Uuid) -> Result<Vec<StoredObject>, ApiError> {
        sqlx::query("DELETE FROM mfa_settings WHERE user_id = $1").bind(user).execute(&mut *conn).await?;
        sqlx::query("DELETE FROM mfa_email_codes WHERE user_id = $1").bind(user).execute(&mut *conn).await?;
        Ok(Vec::new())
    }
}
