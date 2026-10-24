//! Consumer dei job del modulo (`[jobs.*]` di module.toml).

use crate::BLOG_DELETE_GRACE_DAYS;
use noct_audit::{AuditActor, Entry};
use noct_core::{
    AppState, RequestMeta,
    bus::{HandlerError, Message},
};
use serde_json::json;
use uuid::Uuid;

fn retry(e: impl ToString) -> HandlerError {
    HandlerError::Retry(e.to_string())
}

/// Elimina definitivamente i blog con `deleted_at` oltre il periodo di tolleranza: le righe vanno via
/// a cascata nella stessa transazione, gli oggetti su storage (media, copie dei post) dopo il commit,
/// senza far fallire il giro se uno storage è temporaneamente giù.
pub async fn purge_deleted(st: AppState, _msg: Message) -> Result<(), HandlerError> {
    let purged = purge_due(&st).await.map_err(retry)?;
    if purged > 0 {
        tracing::info!(blogs = purged, "blog eliminati definitivamente");
    }
    Ok(())
}

pub async fn purge_due(st: &AppState) -> Result<usize, sqlx::Error> {
    let due: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, slug FROM blogs WHERE deleted_at IS NOT NULL AND deleted_at <= now() - make_interval(days => $1)",
    )
    .bind(BLOG_DELETE_GRACE_DAYS as i32)
    .fetch_all(&st.db)
    .await?;
    for (id, slug) in &due {
        // chi ha caricato file nel blog: il proprietario e chiunque sia (stato) membro
        let uploaders: Vec<Uuid> = sqlx::query_scalar(
            "SELECT owner_id FROM blogs WHERE id = $1 UNION SELECT user_id FROM blog_memberships WHERE blog_id = $1",
        )
        .bind(id)
        .fetch_all(&st.db)
        .await?;
        let mut tx = st.db.begin().await?;
        let deleted = sqlx::query("DELETE FROM blogs WHERE id = $1 AND deleted_at IS NOT NULL")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if deleted.rows_affected() == 0 {
            continue;
        }
        noct_audit::record(
            &mut tx,
            Entry {
                action: "blog.purged",
                actor: AuditActor::System,
                target: Some(("blog", *id)),
                blog_id: Some(*id),
                meta: &RequestMeta::default(),
                payload: json!({"slug": slug}),
            },
        )
        .await
        .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        tx.commit().await?;
        let bucket = &st.config.storage.bucket_content;
        for uploader in uploaders {
            let prefix = format!("{}/userdata/{uploader}/{id}/", st.config.site_slug);
            match st.services.storage.list(bucket, &prefix).await {
                Ok(keys) => {
                    for key in keys {
                        if let Err(e) = st.services.storage.delete(bucket, &key).await {
                            tracing::warn!(error = %e, key, "oggetto di un blog eliminato non rimosso");
                        }
                    }
                }
                Err(e) => tracing::warn!(error = %e, prefix, "storage di un blog eliminato non ripulito"),
            }
        }
    }
    Ok(due.len())
}
