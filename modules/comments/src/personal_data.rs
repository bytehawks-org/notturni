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
        "comments"
    }

    async fn export(&self, st: &AppState, user: Uuid) -> Result<JsonMap, ApiError> {
        let rows: Vec<(Uuid, Uuid, String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
            "SELECT id, post_id, content, status::text, created_at FROM comments WHERE author_id = $1 ORDER BY created_at",
        )
        .bind(user)
        .fetch_all(&st.db)
        .await?;
        Ok(sections(json!({
            "comments_authored": rows.into_iter().map(|(id, post_id, content, status, created_at)| {
                json!({"id": id, "post_id": post_id, "content": content, "status": status, "created_at": created_at})
            }).collect::<Vec<_>>(),
        })))
    }

    /// I commenti restano (con l'autore anonimizzato). Nelle liste dei bloccati di un blog l'etichetta
    /// è il nome dell'utente: si sostituisce, la voce resta a impedirgli di tornare.
    async fn erase(&self, conn: &mut PgConnection, _: &AppState, user: Uuid) -> Result<Vec<StoredObject>, ApiError> {
        sqlx::query("UPDATE blog_blocked_authors SET label = 'Utente eliminato', note = NULL WHERE user_id = $1")
            .bind(user)
            .execute(&mut *conn)
            .await?;
        Ok(Vec::new())
    }
}
