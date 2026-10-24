//! Handler `custom` del crate: raccolti dai file per area.

pub use crate::audit_log::list_audit_log;
pub use crate::blogs::{list_blogs, list_posts, update_blog, update_post};
pub use crate::config::{get_config, update_config};
pub use crate::modules::{disable_module, enable_module, list_modules};
pub use crate::overview::admin_overview;
pub use crate::users::{list_users, reset_user_password, update_user};

use noct_audit::{AuditActor, Entry, user_actor};
use noct_core::{ApiError, RouteCtx, parse_uuid};
use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

pub(crate) fn actor_id(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    ctx.actor.as_ref().map(|a| a.user_id).ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))
}

pub(crate) fn is_super_admin(ctx: &RouteCtx) -> bool {
    ctx.actor.as_ref().is_some_and(|a| a.platform_role == "super_admin")
}

pub(crate) fn path_uuid_free(ctx: &RouteCtx, name: &str) -> String {
    ctx.path.get(name).cloned().unwrap_or_default()
}

pub(crate) fn path_uuid(ctx: &RouteCtx, name: &str) -> Result<Uuid, ApiError> {
    parse_uuid(ctx.path.get(name).map(String::as_str).unwrap_or_default())
}

/// Ogni azione che cambia lo stato di un utente, blog o post richiede una nota che finisce nel
/// registro (`payload.note`).
pub(crate) fn require_note(note: Option<&str>) -> Result<String, ApiError> {
    let cleaned = note.unwrap_or_default().trim();
    if cleaned.chars().count() < 3 {
        return Err(ApiError::bad_request("Serve una nota per il registro (almeno 3 caratteri)."));
    }
    Ok(cleaned.chars().take(1000).collect())
}

/// Filtro di testo: sottostringa letterale, senza distinguere le maiuscole (`%` e `_` non sono jolly).
pub(crate) fn text_filter(ctx: &RouteCtx, key: &str) -> Option<String> {
    ctx.query.get(key).map(|q| q.trim().to_string()).filter(|q| !q.is_empty())
}

pub(crate) struct Target<'a> {
    pub kind: &'a str,
    pub id: Uuid,
    pub blog: Option<Uuid>,
}

pub(crate) async fn audit(
    conn: &mut PgConnection,
    ctx: &RouteCtx,
    action: &str,
    target: Option<Target<'_>>,
    payload: Value,
) -> Result<(), ApiError> {
    let actor: AuditActor = user_actor(&mut *conn, actor_id(ctx)?).await.map_err(ApiError::internal)?;
    let (target, blog_id) = match target {
        Some(t) => (Some((t.kind, t.id)), t.blog),
        None => (None, None),
    };
    noct_audit::record(conn, Entry { action, actor, target, blog_id, meta: &ctx.meta, payload })
        .await
        .map_err(ApiError::internal)
}
