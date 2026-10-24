//! Lettura e serializzazione del profilo.

use crate::rules::{self, VerificationLists};
use chrono::{DateTime, Duration, Utc};
use noct_core::{ApiError, AppState, Entity};
use serde_json::{Value, json};
use sqlx::FromRow;
use uuid::Uuid;

/// Colonne di [`UserRow`] (alias `u`).
pub const USER_COLUMNS: &str = "u.id, u.username, u.email, u.bio, u.first_name, u.last_name, u.display_name, \
    u.post_author_name_style::text AS post_author_name_style, u.country, u.native_language, u.fallback_languages, \
    u.interests, u.avatar_object_key, u.created_at, u.verified_domain, u.directory_listed, u.username_changed_at";

#[derive(Debug, Clone, FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub bio: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub display_name: Option<String>,
    pub post_author_name_style: String,
    pub country: Option<String>,
    pub native_language: Option<String>,
    pub fallback_languages: Vec<String>,
    pub interests: Vec<String>,
    pub avatar_object_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub verified_domain: Option<String>,
    pub directory_listed: bool,
    pub username_changed_at: Option<DateTime<Utc>>,
}

pub fn instance_fqdn(st: &AppState) -> String {
    st.config.instance_fqdn.clone()
}

pub fn avatar_url(st: &AppState, key: Option<&str>) -> Option<String> {
    key.map(|k| st.services.storage.public_url(&st.config.storage.bucket_avatars, k))
}

pub async fn verification_lists(st: &AppState) -> Result<VerificationLists, ApiError> {
    let row: Option<(Vec<String>, Vec<String>, Vec<String>)> = sqlx::query_as(
        "SELECT gold_identifiers, silver_identifiers, blue_domains FROM verification_settings WHERE id = 1",
    )
    .fetch_optional(&st.db)
    .await?;
    Ok(row.map(|(gold, silver, blue_domains)| VerificationLists { gold, silver, blue_domains }).unwrap_or_default())
}

pub async fn fetch(st: &AppState, id: Uuid) -> Result<UserRow, ApiError> {
    sqlx::query_as(&format!("SELECT {USER_COLUMNS} FROM users u WHERE u.id = $1"))
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or_else(|| ApiError::not_found("Utente non trovato."))
}

/// Un utente attivo per username o, in mancanza, per dominio personale verificato. Gli account
/// disattivati (sospesi, cancellati) non hanno un profilo pubblico.
pub async fn find_public(st: &AppState, identifier: &str) -> Result<UserRow, ApiError> {
    sqlx::query_as(&format!(
        "SELECT {USER_COLUMNS} FROM users u
         WHERE u.is_active AND (lower(u.username) = lower($1) OR u.verified_domain = lower($1))
         ORDER BY (lower(u.username) = lower($1)) DESC LIMIT 1"
    ))
    .bind(identifier)
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(|| ApiError::not_found("Utente non trovato."))
}

async fn social_links(st: &AppState, user: Uuid) -> Result<Vec<Value>, ApiError> {
    let rows: Vec<(Uuid, String, String, i32)> = sqlx::query_as(
        "SELECT id, label, url, position FROM social_links WHERE user_id = $1 ORDER BY position, created_at, id",
    )
    .bind(user)
    .fetch_all(&st.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, label, url, position)| json!({"id": id, "label": label, "url": url, "position": position}))
        .collect())
}

/// Profilo pubblico (`ProfileOut`).
pub async fn public_json(st: &AppState, u: &UserRow) -> Result<Value, ApiError> {
    let fqdn = instance_fqdn(st);
    let lists = verification_lists(st).await?;
    Ok(json!({
        "username": u.username,
        "bio": u.bio,
        "first_name": u.first_name,
        "last_name": u.last_name,
        "display_name": u.display_name,
        "post_author_name_style": u.post_author_name_style,
        "country": u.country,
        "native_language": u.native_language,
        "fallback_languages": u.fallback_languages,
        "interests": u.interests,
        "avatar_url": avatar_url(st, u.avatar_object_key.as_deref()),
        "social_links": social_links(st, u.id).await?,
        "created_at": u.created_at,
        "verification_tier": rules::verification_tier(&lists, &fqdn, &u.email, &u.username, u.verified_domain.as_deref()),
        "custom_domain": u.verified_domain,
        "atproto_did": rules::atproto_did(&fqdn, u.id),
        "activitypub_actor_id": rules::activitypub_actor_id(&fqdn, u.id),
    }))
}

/// Profilo privato (`MeProfileOut`): quello pubblico più email, cooldown dello username, cambio
/// email in corso e, con i moduli che lo prevedono, lo stato del dominio personale.
pub async fn me_json(st: &AppState, u: &UserRow) -> Result<Value, ApiError> {
    let mut v = public_json(st, u).await?;
    let pending: Option<(String, bool)> = sqlx::query_as(
        "SELECT new_email, old_consumed_at IS NOT NULL FROM email_change_requests
         WHERE user_id = $1 AND completed_at IS NULL ORDER BY created_at DESC LIMIT 1",
    )
    .bind(u.id)
    .fetch_optional(&st.db)
    .await?;
    let pending = pending.map(|(new_email, old_done)| {
        json!({"new_email": new_email, "stage": if old_done { "awaiting_new_confirmation" } else { "awaiting_old_confirmation" }})
    });
    let obj = v.as_object_mut().expect("oggetto");
    obj.insert("email".into(), json!(u.email));
    obj.insert("username_changed_at".into(), json!(u.username_changed_at));
    obj.insert(
        "next_username_change_allowed_at".into(),
        json!(u.username_changed_at.map(|t| t + Duration::days(rules::USERNAME_COOLDOWN_DAYS))),
    );
    obj.insert("pending_email_change".into(), json!(pending));
    obj.insert("directory_listed".into(), json!(u.directory_listed));
    // valori di base di quando il modulo dei domini personali è spento
    obj.insert("domain_pending_verification".into(), Value::Null);
    obj.insert("domain_status".into(), Value::Null);
    obj.insert("domain_verification_instructions".into(), Value::Null);
    if let Some(fields) = st.extensions.enrich(&st.db, Entity::User, &[u.id]).await?.remove(&u.id) {
        obj.extend(fields);
    }
    Ok(v)
}
