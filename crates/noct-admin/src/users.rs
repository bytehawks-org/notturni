use crate::ADMIN_ROLES;
use crate::handlers_impl::{Target, actor_id, audit, is_super_admin, path_uuid, require_note, text_filter};
use chrono::{DateTime, Utc};
use noct_core::{ApiError, RouteCtx};
use serde_json::{Value, json};
use sqlx::FromRow;
use uuid::Uuid;

/// Solo un Super Admin assegna o toglie questi ruoli: un Amministratore non crea altri amministratori.
const PRIVILEGED: &[&str] = &["amministratore", "super_admin"];
const ROLES: &[&str] = &["utente", "moderatore", "amministratore", "super_admin"];

#[derive(FromRow)]
struct UserRow {
    id: Uuid,
    username: String,
    email: String,
    platform_role: String,
    is_active: bool,
    mfa_enabled: bool,
    created_at: DateTime<Utc>,
    blogs_count: i64,
    last_seen_at: Option<DateTime<Utc>>,
}

const SELECT: &str =
    "SELECT u.id, u.username, u.email, u.platform_role::text AS platform_role, u.is_active, u.mfa_enabled,
        u.created_at, (SELECT count(*) FROM blogs b WHERE b.owner_id = u.id) AS blogs_count,
        (SELECT max(s.last_used_at) FROM user_sessions s WHERE s.user_id = u.id) AS last_seen_at
    FROM users u";

fn out(r: UserRow) -> Value {
    json!({
        "id": r.id, "username": r.username, "email": r.email, "platform_role": r.platform_role,
        "is_active": r.is_active, "mfa_enabled": r.mfa_enabled, "created_at": r.created_at,
        "blogs_count": r.blogs_count, "last_seen_at": r.last_seen_at,
    })
}

pub async fn list_users(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let q = text_filter(&ctx, "q");
    let rows: Vec<UserRow> = sqlx::query_as(&format!(
        "{SELECT} WHERE $1::text IS NULL OR position(lower($1) IN lower(u.username)) > 0
            OR position(lower($1) IN lower(u.email)) > 0
         ORDER BY u.created_at, u.id"
    ))
    .bind(q)
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(rows.into_iter().map(out).collect())
}

pub async fn update_user(ctx: RouteCtx, body: crate::AdminUserUpdate) -> Result<Value, ApiError> {
    let admin = actor_id(&ctx)?;
    let id = path_uuid(&ctx, "user_id")?;
    let mut tx = ctx.state.db.begin().await?;
    let target: Option<(String, bool)> =
        sqlx::query_as("SELECT platform_role::text, is_active FROM users WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let (role, active) = target.ok_or_else(|| ApiError::not_found("Utente non trovato."))?;

    let new_role = body.platform_role.as_deref();
    if let Some(r) = new_role
        && !ROLES.contains(&r)
    {
        return Err(ApiError::bad_request("Ruolo non valido."));
    }
    let role_changes = new_role.is_some_and(|r| r != role);
    let active_changes = body.is_active.is_some_and(|a| a != active);
    let note = if role_changes || active_changes { Some(require_note(body.note.as_deref())?) } else { None };

    if let Some(r) = new_role {
        if (PRIVILEGED.contains(&r) || PRIVILEGED.contains(&role.as_str())) && !is_super_admin(&ctx) {
            return Err(ApiError::forbidden("Solo un Super Admin può assegnare o rimuovere ruoli di amministrazione."));
        }
        if role_changes {
            // l'ultimo Super Admin non si retrocede: senza, la piattaforma non si amministra
            if role == "super_admin" {
                let others: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM users WHERE platform_role = 'super_admin' AND is_active AND id <> $1",
                )
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
                if others == 0 {
                    return Err(ApiError::bad_request("Non si può retrocedere l'ultimo Super Admin."));
                }
            }
            audit(
                &mut tx,
                &ctx,
                "user.role_change",
                Some(Target { kind: "user", id, blog: None }),
                json!({"from": role, "to": r, "note": note}),
            )
            .await?;
            sqlx::query("UPDATE users SET platform_role = $2::platform_role, updated_at = now() WHERE id = $1")
                .bind(id)
                .bind(r)
                .execute(&mut *tx)
                .await?;
        }
    }
    if let Some(a) = body.is_active
        && active_changes
    {
        if id == admin && !a {
            return Err(ApiError::bad_request("Non puoi disattivare il tuo stesso account."));
        }
        // un Amministratore non disattiva un altro amministratore
        if PRIVILEGED.contains(&role.as_str()) && !is_super_admin(&ctx) {
            return Err(ApiError::forbidden("Solo un Super Admin può disattivare un amministratore."));
        }
        audit(
            &mut tx,
            &ctx,
            if a { "user.activated" } else { "user.deactivated" },
            Some(Target { kind: "user", id, blog: None }),
            json!({"note": note}),
        )
        .await?;
        // disattivare revoca anche le sessioni e invalida i token già emessi
        sqlx::query(
            "UPDATE users SET is_active = $2, updated_at = now(),
                    credentials_changed_at = CASE WHEN $2 THEN credentials_changed_at ELSE now() END WHERE id = $1",
        )
        .bind(id)
        .bind(a)
        .execute(&mut *tx)
        .await?;
        if !a {
            sqlx::query("UPDATE user_sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
    }
    let row: UserRow = sqlx::query_as(&format!("{SELECT} WHERE u.id = $1")).bind(id).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(out(row))
}

/// Reset forzoso: stesso ciclo email del «password dimenticata», avviato da un amministratore.
pub async fn reset_user_password(ctx: RouteCtx) -> Result<(), ApiError> {
    let id = path_uuid(&ctx, "user_id")?;
    let mut tx = ctx.state.db.begin().await?;
    let active: Option<bool> =
        sqlx::query_scalar("SELECT is_active FROM users WHERE id = $1").bind(id).fetch_optional(&mut *tx).await?;
    match active {
        None => return Err(ApiError::not_found("Utente non trovato.")),
        Some(false) => return Err(ApiError::bad_request("L'utente non è attivo.")),
        Some(true) => {}
    }
    // un Amministratore non può far reimpostare la password di un altro amministratore
    let role: String =
        sqlx::query_scalar("SELECT platform_role::text FROM users WHERE id = $1").bind(id).fetch_one(&mut *tx).await?;
    if ADMIN_ROLES.contains(&role.as_str()) && !is_super_admin(&ctx) && id != actor_id(&ctx)? {
        return Err(ApiError::forbidden("Solo un Super Admin può reimpostare la password di un amministratore."));
    }
    audit(&mut tx, &ctx, "user.password_reset_triggered", Some(Target { kind: "user", id, blog: None }), json!({}))
        .await?;
    noct_auth::enqueue_password_reset(&mut tx, id).await?;
    tx.commit().await?;
    Ok(())
}
