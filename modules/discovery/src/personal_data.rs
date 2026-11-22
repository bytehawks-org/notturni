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
        "discovery"
    }

    async fn export(&self, st: &AppState, user: Uuid) -> Result<JsonMap, ApiError> {
        let following: Vec<Uuid> =
            sqlx::query_scalar("SELECT followed_user_id FROM user_follows WHERE follower_id = $1")
                .bind(user)
                .fetch_all(&st.db)
                .await?;
        let followers: Vec<Uuid> =
            sqlx::query_scalar("SELECT follower_id FROM user_follows WHERE followed_user_id = $1")
                .bind(user)
                .fetch_all(&st.db)
                .await?;
        let blogs: Vec<Uuid> = sqlx::query_scalar("SELECT blog_id FROM blog_follows WHERE follower_id = $1")
            .bind(user)
            .fetch_all(&st.db)
            .await?;
        Ok(sections(json!({
            "follows": {"following_user_ids": following, "follower_user_ids": followers, "blogs_followed_ids": blogs},
        })))
    }

    /// I follow, in entrambe le direzioni.
    async fn erase(&self, conn: &mut PgConnection, _: &AppState, user: Uuid) -> Result<Vec<StoredObject>, ApiError> {
        sqlx::query("DELETE FROM user_follows WHERE follower_id = $1 OR followed_user_id = $1")
            .bind(user)
            .execute(&mut *conn)
            .await?;
        sqlx::query("DELETE FROM blog_follows WHERE follower_id = $1").bind(user).execute(&mut *conn).await?;
        Ok(Vec::new())
    }
}
