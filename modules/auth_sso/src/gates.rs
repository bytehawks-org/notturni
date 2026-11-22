//! Gate del login: completa il collegamento SSO rimasto in sospeso dopo la verifica MFA.

use crate::providers::Provider;
use async_trait::async_trait;
use noct_core::{ApiError, AppState, LoginGate};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

pub fn all() -> Vec<Arc<dyn LoginGate>> {
    vec![Arc::new(SsoGate)]
}

struct SsoGate;

#[async_trait]
impl LoginGate for SsoGate {
    fn module(&self) -> &'static str {
        "auth_sso"
    }

    /// Questo gate non pone sfide: il login con password non lo riguarda.
    async fn challenge(&self, _st: &AppState, _user: Uuid, _extra: Option<Value>) -> Result<Option<Value>, ApiError> {
        Ok(None)
    }

    async fn completed(&self, st: &AppState, user_id: Uuid, extra: &Value) -> Result<(), ApiError> {
        let field = |k: &str| extra.get(k).and_then(Value::as_str);
        // `extra` è firmato dal server nella sfida; se non è un collegamento SSO non è affar nostro
        let (Some(provider), Some(external_id), Some(email)) =
            (field("provider"), field("provider_user_id"), field("email"))
        else {
            return Ok(());
        };
        if Provider::parse(provider).is_none() {
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO sso_identities (user_id, provider, provider_user_id, email) VALUES ($1, $2::sso_provider, $3, $4)
             ON CONFLICT (provider, provider_user_id) DO NOTHING",
        )
        .bind(user_id)
        .bind(provider)
        .bind(external_id)
        .bind(email)
        .execute(&st.db)
        .await?;
        Ok(())
    }
}
