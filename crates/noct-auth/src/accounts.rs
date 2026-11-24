//! Account management without the public flows: creation by an administrator and password reset
//! by an operator with shell access (no SMTP). Used by the admin API and by `noct-ctl user`.
//!
//! The validations and the hashing are the same as `register` and `reset_password`.

use crate::util::hash_password;
use noct_audit::{AuditActor, Entry};
use noct_core::{
    ApiError, RequestMeta, map_db_error,
    security::validate::{normalize_email, validate_email, validate_password_policy, validate_username},
};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

/// Same advisory lock as `register`: user inserts are serialized.
const USERS_LOCK: i64 = 727274001;

/// Role of every account created through this module. Promotions go through `PATCH /admin/users/{id}`.
pub const CREATED_ROLE: &str = "utente";

pub struct NewUser {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct CreatedUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub platform_role: String,
}

/// Creates an active account (`users.is_active` defaults to true; there is no email verification step).
/// The audit event `admin.user_created` is written in the same transaction and never holds the password.
/// `via` is `api` or `noct-ctl`.
pub async fn create_user(
    db: &PgPool,
    input: NewUser,
    actor: AuditActor,
    meta: &RequestMeta,
    via: &str,
) -> Result<CreatedUser, ApiError> {
    let email = normalize_email(&input.email);
    validate_username(&input.username).map_err(ApiError::bad_request)?;
    validate_email(&email).map_err(ApiError::bad_request)?;
    validate_password_policy(&input.password).map_err(ApiError::bad_request)?;
    let hash = hash_password(input.password).await?;

    let mut tx = db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)").bind(USERS_LOCK).execute(&mut *tx).await?;
    let taken = "Username o email già in uso.";
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (username, email, hashed_password, platform_role) VALUES ($1, $2, $3, $4::platform_role) RETURNING id",
    )
    .bind(&input.username)
    .bind(&email)
    .bind(&hash)
    .bind(CREATED_ROLE)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| map_db_error(e, &[("uq_users_username", 409, taken), ("uq_users_email", 409, taken)]))?;
    noct_audit::record(
        &mut tx,
        Entry {
            action: "admin.user_created",
            actor,
            target: Some(("user", id)),
            blog_id: None,
            meta,
            payload: json!({ "platform_role": CREATED_ROLE, "via": via }),
        },
    )
    .await
    .map_err(ApiError::internal)?;
    tx.commit().await?;
    Ok(CreatedUser { id, username: input.username, email, platform_role: CREATED_ROLE.into() })
}

/// Sets a new password without any code: revokes every session, invalidates the access tokens already
/// issued (`credentials_changed_at`) and writes `admin.user_password_reset` (no password data).
/// Unknown email and inactive users give the same generic error; SSO-only accounts are refused.
pub async fn reset_password(
    db: &PgPool,
    email: &str,
    new_password: String,
    actor: AuditActor,
    meta: &RequestMeta,
    via: &str,
) -> Result<Uuid, ApiError> {
    let email = normalize_email(email);
    validate_password_policy(&new_password).map_err(ApiError::bad_request)?;
    let generic = || ApiError::not_found("Nessun utente attivo con questa email.");

    let row: Option<(Uuid, bool, bool)> =
        sqlx::query_as("SELECT id, is_active, hashed_password IS NOT NULL FROM users WHERE email = $1")
            .bind(&email)
            .fetch_optional(db)
            .await?;
    let Some((id, true, has_password)) = row else { return Err(generic()) };
    if !has_password {
        return Err(ApiError::bad_request(
            "L'account non ha una password locale (accesso solo tramite SSO): nulla da reimpostare.",
        ));
    }
    let hash = hash_password(new_password).await?;

    let mut tx = db.begin().await?;
    let updated = sqlx::query(
        "UPDATE users SET hashed_password = $2, credentials_changed_at = now(), updated_at = now()
         WHERE id = $1 AND is_active AND hashed_password IS NOT NULL",
    )
    .bind(id)
    .bind(&hash)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(generic());
    }
    sqlx::query("DELETE FROM user_sessions WHERE user_id = $1").bind(id).execute(&mut *tx).await?;
    noct_audit::record(
        &mut tx,
        Entry {
            action: "admin.user_password_reset",
            actor,
            target: Some(("user", id)),
            blog_id: None,
            meta,
            payload: json!({ "via": via }),
        },
    )
    .await
    .map_err(ApiError::internal)?;
    tx.commit().await?;
    Ok(id)
}
