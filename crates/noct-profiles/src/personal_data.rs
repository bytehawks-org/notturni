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
        "profiles"
    }

    async fn export(&self, st: &AppState, user: Uuid) -> Result<JsonMap, ApiError> {
        let u = crate::view::fetch(st, user).await?;
        let links: Vec<(String, String, i32)> = sqlx::query_as(
            "SELECT label, url, position FROM social_links WHERE user_id = $1 ORDER BY position, created_at",
        )
        .bind(user)
        .fetch_all(&st.db)
        .await?;
        let lists = crate::view::verification_lists(st).await?;
        let tier = crate::rules::verification_tier(
            &lists,
            &st.config.instance_fqdn,
            &u.email,
            &u.username,
            u.verified_domain.as_deref(),
        );
        Ok(sections(json!({
            "account": {
                "bio": u.bio, "country": u.country, "native_language": u.native_language,
                "fallback_languages": u.fallback_languages, "interests": u.interests,
                "verified_domain": u.verified_domain, "verification_tier": tier,
            },
            "social_links": links.into_iter().map(|(label, url, position)| json!({"label": label, "url": url, "position": position})).collect::<Vec<_>>(),
        })))
    }

    async fn erase(&self, conn: &mut PgConnection, st: &AppState, user: Uuid) -> Result<Vec<StoredObject>, ApiError> {
        let avatar: Option<String> = sqlx::query_scalar("SELECT avatar_object_key FROM users WHERE id = $1")
            .bind(user)
            .fetch_one(&mut *conn)
            .await?;
        for table in ["social_links", "email_change_requests"] {
            sqlx::query(&format!("DELETE FROM {table} WHERE user_id = $1")).bind(user).execute(&mut *conn).await?;
        }
        // il dominio verificato è un identificativo pubblico quanto lo username
        sqlx::query(
            "UPDATE users SET bio = NULL, first_name = NULL, last_name = NULL, country = NULL, native_language = NULL,
                    fallback_languages = '{}', interests = '{}', avatar_object_key = NULL, verified_domain = NULL,
                    username_changed_at = NULL, ui_locale = NULL, directory_listed = false WHERE id = $1",
        )
        .bind(user)
        .execute(&mut *conn)
        .await?;
        Ok(avatar
            .map(|key| StoredObject { bucket: st.config.storage.bucket_avatars.clone(), key })
            .into_iter()
            .collect())
    }
}
