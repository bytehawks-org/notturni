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
        "blogs"
    }

    async fn export(&self, st: &AppState, user: Uuid) -> Result<JsonMap, ApiError> {
        let rows: Vec<(String, String, Option<String>, Option<String>, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
            "SELECT slug, title, subtitle, description, visibility::text, created_at FROM blogs WHERE owner_id = $1 ORDER BY created_at",
        )
        .bind(user)
        .fetch_all(&st.db)
        .await?;
        let memberships: Vec<(String, String)> = sqlx::query_as(
            "SELECT b.slug, m.role::text FROM blog_memberships m JOIN blogs b ON b.id = m.blog_id WHERE m.user_id = $1 ORDER BY b.slug",
        )
        .bind(user)
        .fetch_all(&st.db)
        .await?;
        Ok(sections(json!({
            "blogs_owned": rows.into_iter().map(|(slug, title, subtitle, description, visibility, created_at)| {
                json!({"slug": slug, "title": title, "subtitle": subtitle, "description": description, "visibility": visibility, "created_at": created_at})
            }).collect::<Vec<_>>(),
            "memberships": memberships.into_iter().map(|(blog, role)| json!({"blog_slug": blog, "role": role})).collect::<Vec<_>>(),
        })))
    }

    /// Le membership su blog altrui spariscono (l'account anonimizzato non è più un collaboratore);
    /// i blog di proprietà restano, come i loro contenuti.
    async fn erase(&self, conn: &mut PgConnection, _: &AppState, user: Uuid) -> Result<Vec<StoredObject>, ApiError> {
        sqlx::query("DELETE FROM blog_memberships WHERE user_id = $1").bind(user).execute(&mut *conn).await?;
        sqlx::query("DELETE FROM blog_invitations WHERE invited_user_id = $1").bind(user).execute(&mut *conn).await?;
        Ok(Vec::new())
    }
}
