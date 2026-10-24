//! Estensione del profilo privato: stato del dominio non ancora verificato.

use crate::rules;
use async_trait::async_trait;
use noct_core::{ApiError, Entity, EntityExtension, ExtCtx, JsonMap};
use serde_json::json;
use sqlx::{PgConnection, PgPool};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

pub fn all() -> Vec<Arc<dyn EntityExtension>> {
    vec![Arc::new(UserDomain)]
}

struct UserDomain;

#[async_trait]
impl EntityExtension for UserDomain {
    fn module(&self) -> &'static str {
        "custom_domains"
    }
    fn entity(&self) -> Entity {
        Entity::User
    }
    fn fields(&self) -> &'static [&'static str] {
        &["domain_pending_verification", "domain_status", "domain_verification_instructions"]
    }

    /// Il profilo non è scritto da qui: i domini hanno le loro rotte.
    async fn apply(&self, _: &mut PgConnection, _: &ExtCtx<'_>, _: &JsonMap) -> Result<(), ApiError> {
        Ok(())
    }

    async fn enrich(&self, db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, JsonMap>, ApiError> {
        let rows: Vec<(Uuid, String, String, String)> = sqlx::query_as(
            "SELECT user_id, domain, status::text, verification_token FROM custom_domains
             WHERE user_id = ANY($1) AND status <> 'verified'",
        )
        .bind(ids)
        .fetch_all(db)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(user, domain, status, token)| {
                let fields = json!({
                    "domain_pending_verification": domain,
                    "domain_status": status,
                    "domain_verification_instructions": {
                        "txt_record_name": rules::txt_record_name(&domain),
                        "txt_record_value": rules::txt_record_value(&token),
                    },
                });
                (user, fields.as_object().cloned().unwrap_or_default())
            })
            .collect())
    }
}
