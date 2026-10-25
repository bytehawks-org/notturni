//! Handler `custom` del modulo: configurazione della sincronizzazione Git di uno Spazio. Solo il
//! proprietario; il token si cifra a riposo e non viene mai restituito.

use crate::{GitSyncIn, SUBJECT_RUN, rules, ssh, sync};
use chrono::{DateTime, Utc};
use noct_audit::{Entry, user_actor};
use noct_core::{
    Actor, ApiError, BlogRef, RouteCtx,
    bus::{Kind, outbox},
};
use noct_security::secrets;
use reqwest::Url;
use serde_json::{Value, json};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

const DEFAULT_BRANCH: &str = "notturni-backup";
const DEFAULT_USERNAME: &str = "git";

fn actor(ctx: &RouteCtx) -> Result<Actor, ApiError> {
    ctx.actor.clone().ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))
}

/// Lo Spazio dello scope, se `actor` ne è il proprietario.
fn owned_space<'a>(ctx: &'a RouteCtx, actor: &Actor) -> Result<&'a BlogRef, ApiError> {
    let blog = ctx.blog.as_ref().ok_or_else(|| ApiError::internal("scope `blog` assente"))?;
    if blog.owner_id != actor.user_id {
        return Err(ApiError::forbidden("Solo il proprietario può gestire la sincronizzazione Git."));
    }
    Ok(blog)
}

#[derive(FromRow)]
struct Row {
    remote_url: String,
    branch: String,
    username: String,
    auth_method: String,
    secret: String,
    ssh_public_key: Option<String>,
    ssh_host_key: Option<String>,
    ssh_host_fingerprints: Option<String>,
    include_drafts: bool,
    enabled: bool,
    last_status: Option<String>,
    last_run_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
    last_commit: Option<String>,
    last_pages: Option<i32>,
}

const SELECT: &str = "SELECT remote_url, branch, username, auth_method, secret, ssh_public_key, ssh_host_key,
    ssh_host_fingerprints, include_drafts, enabled, last_status, last_run_at, last_error, last_commit, last_pages
    FROM git_sync_configs WHERE blog_id = $1";

fn render(row: Option<Row>) -> Value {
    match row {
        None => json!({
            "configured": false, "remote_url": null, "branch": null, "username": null, "auth_method": null,
            "has_secret": false, "ssh_public_key": null, "ssh_host_fingerprints": [],
            "include_drafts": false, "enabled": false, "last_status": null, "last_run_at": null, "last_error": null,
            "last_commit": null, "last_pages": null,
        }),
        Some(r) => {
            let ssh = r.auth_method == "ssh";
            let fingerprints: Vec<&str> = r.ssh_host_fingerprints.as_deref().unwrap_or_default().lines().collect();
            json!({
                "configured": true, "remote_url": r.remote_url, "branch": r.branch, "username": r.username,
                "auth_method": r.auth_method, "has_secret": !r.secret.is_empty(),
                "ssh_public_key": if ssh { r.ssh_public_key } else { None }, "ssh_host_fingerprints": fingerprints,
                "include_drafts": r.include_drafts, "enabled": r.enabled,
                "last_status": r.last_status, "last_run_at": r.last_run_at, "last_error": r.last_error,
                "last_commit": r.last_commit, "last_pages": r.last_pages,
            })
        }
    }
}

async fn audit(
    conn: &mut PgConnection,
    ctx: &RouteCtx,
    user: Uuid,
    action: &str,
    blog: &BlogRef,
) -> Result<(), ApiError> {
    let who = user_actor(&mut *conn, user).await.map_err(ApiError::internal)?;
    noct_audit::record(
        conn,
        Entry {
            action,
            actor: who,
            target: Some(("blog", blog.id)),
            blog_id: Some(blog.id),
            meta: &ctx.meta,
            payload: json!({"blog_slug": blog.slug}),
        },
    )
    .await
    .map_err(ApiError::internal)
}

pub async fn get_git_sync(ctx: RouteCtx) -> Result<Value, ApiError> {
    let actor = actor(&ctx)?;
    let blog = owned_space(&ctx, &actor)?;
    let row: Option<Row> = sqlx::query_as(SELECT).bind(blog.id).fetch_optional(&ctx.state.db).await?;
    Ok(render(row))
}

pub async fn put_git_sync(ctx: RouteCtx, body: GitSyncIn) -> Result<Value, ApiError> {
    let actor = actor(&ctx)?;
    let blog = owned_space(&ctx, &actor)?;
    let st = &ctx.state;
    let existing: Option<Row> = sqlx::query_as(SELECT).bind(blog.id).fetch_optional(&st.db).await?;

    let method = match body.auth_method.as_deref() {
        Some(m) => rules::validate_method(m).map_err(ApiError::bad_request)?,
        None => rules::validate_method(existing.as_ref().map_or("https", |e| e.auth_method.as_str()))
            .map_err(ApiError::bad_request)?,
    };
    let switched = existing.as_ref().is_some_and(|e| e.auth_method != method);
    let ssh_mode = method == "ssh";
    let url = if ssh_mode {
        rules::validate_ssh_remote(&body.remote_url)
    } else {
        rules::validate_remote(&body.remote_url, sync::allow_private(st))
    }
    .map_err(ApiError::bad_request)?;
    sync::check_host(&url, sync::allow_private(st)).await.map_err(ApiError::bad_request)?;

    // branch e utente assenti: restano quelli già impostati (o i predefiniti alla prima configurazione)
    let branch = body
        .branch
        .clone()
        .filter(|b| !b.is_empty())
        .or_else(|| existing.as_ref().map(|e| e.branch.clone()))
        .unwrap_or_else(|| DEFAULT_BRANCH.into());
    rules::validate_branch(&branch).map_err(ApiError::bad_request)?;
    let username = body
        .username
        .clone()
        .filter(|u| !u.trim().is_empty())
        .or_else(|| existing.as_ref().filter(|_| !switched).map(|e| e.username.clone()))
        .unwrap_or_else(|| if ssh_mode { url.username().to_string() } else { DEFAULT_USERNAME.into() });
    if username.chars().any(|c| c.is_control() || c == ':') {
        return Err(ApiError::bad_request("Nome utente non valido."));
    }

    // credenziale: token (https) oppure chiave di deploy generata qui (ssh)
    let key = sync::secrets_key(st);
    let (secret, public_key) = if ssh_mode {
        if body.secret.is_some() {
            return Err(ApiError::bad_request("Con SSH la chiave la genera la piattaforma: non inviare `secret`."));
        }
        match existing.as_ref() {
            Some(e) if !switched && !body.regenerate_key.unwrap_or(false) && e.ssh_public_key.is_some() => {
                (e.secret.clone(), e.ssh_public_key.clone())
            }
            _ => {
                let pair =
                    ssh::generate_keypair(&format!("notturni-{}", blog.slug)).await.map_err(ApiError::internal)?;
                (secrets::seal(&key, &pair.private), Some(pair.public))
            }
        }
    } else {
        match (body.secret.as_deref(), existing.as_ref().filter(|_| !switched)) {
            (Some(s), _) if s.trim().is_empty() => return Err(ApiError::bad_request("Il token non può essere vuoto.")),
            (Some(s), _) if s.trim() != s || s.chars().any(char::is_control) => {
                return Err(ApiError::bad_request("Il token contiene spazi o caratteri non validi."));
            }
            (Some(s), _) => (secrets::seal(&key, s), None),
            (None, Some(e)) => (e.secret.clone(), None),
            (None, None) => return Err(ApiError::bad_request("Il token è obbligatorio alla prima configurazione.")),
        }
    };

    // host key SSH: rilevata ora e fissata; cambia solo se cambia l'host o si chiede un'impronta diversa
    let (host_key, fingerprints) = if ssh_mode {
        let (host, port) = (url.host_str().unwrap_or_default().to_string(), url.port().unwrap_or(22));
        let keep = existing.as_ref().filter(|e| {
            !switched
                && body.host_fingerprint.is_none()
                && e.ssh_host_key.is_some()
                && Url::parse(&e.remote_url)
                    .is_ok_and(|old| old.host_str() == url.host_str() && old.port() == url.port())
        });
        match keep {
            Some(e) => (e.ssh_host_key.clone(), e.ssh_host_fingerprints.clone()),
            None => {
                let keyscan = st.module_config_str("git_sync", "keyscan_bin").unwrap_or("ssh-keyscan");
                let scanned = ssh::scan_host(keyscan, &host, port).await.map_err(ApiError::bad_request)?;
                let found = ssh::fingerprints(&scanned).await.map_err(ApiError::internal)?;
                if let Some(expected) =
                    body.host_fingerprint.as_deref().filter(|e| !found.iter().any(|f| f == e.trim()))
                {
                    return Err(ApiError::bad_request(format!(
                        "L'impronta dell'host ({}) non corrisponde a quella attesa. Ricevute: {}.",
                        expected.trim(),
                        found.join(", ")
                    )));
                }
                (Some(scanned), Some(found.join("\n")))
            }
        }
    } else {
        (None, None)
    };

    let include_drafts = body.include_drafts.unwrap_or_else(|| existing.as_ref().is_some_and(|e| e.include_drafts));
    let enabled = body.enabled.unwrap_or_else(|| existing.as_ref().is_none_or(|e| e.enabled));
    // un repository, un accesso, un branch o l'inclusione delle bozze diversi richiedono un nuovo invio
    let changed = existing.as_ref().is_none_or(|e| {
        e.remote_url != url.as_str()
            || e.branch != branch
            || e.username != username
            || e.auth_method != method
            || e.secret != secret
            || e.ssh_host_key != host_key
            || e.include_drafts != include_drafts
    });
    let mut tx = st.db.begin().await?;
    sqlx::query(
        "INSERT INTO git_sync_configs (blog_id, owner_id, remote_url, branch, username, auth_method, secret,
                ssh_public_key, ssh_host_key, ssh_host_fingerprints, include_drafts, enabled)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
         ON CONFLICT (blog_id) DO UPDATE SET owner_id = EXCLUDED.owner_id, remote_url = EXCLUDED.remote_url,
            branch = EXCLUDED.branch, username = EXCLUDED.username, auth_method = EXCLUDED.auth_method,
            secret = EXCLUDED.secret, ssh_public_key = EXCLUDED.ssh_public_key, ssh_host_key = EXCLUDED.ssh_host_key,
            ssh_host_fingerprints = EXCLUDED.ssh_host_fingerprints, include_drafts = EXCLUDED.include_drafts,
            enabled = EXCLUDED.enabled, updated_at = now(),
            last_hash = CASE WHEN $13 THEN NULL ELSE git_sync_configs.last_hash END,
            last_status = CASE WHEN $13 THEN NULL ELSE git_sync_configs.last_status END,
            last_error = CASE WHEN $13 THEN NULL ELSE git_sync_configs.last_error END",
    )
    .bind(blog.id)
    .bind(actor.user_id)
    .bind(url.as_str())
    .bind(&branch)
    .bind(&username)
    .bind(method)
    .bind(&secret)
    .bind(&public_key)
    .bind(&host_key)
    .bind(&fingerprints)
    .bind(include_drafts)
    .bind(enabled)
    .bind(changed)
    .execute(&mut *tx)
    .await?;
    audit(&mut tx, &ctx, actor.user_id, "git_sync.configured", blog).await?;
    let row: Option<Row> = sqlx::query_as(SELECT).bind(blog.id).fetch_optional(&mut *tx).await?;
    tx.commit().await?;
    Ok(render(row))
}

pub async fn delete_git_sync(ctx: RouteCtx) -> Result<(), ApiError> {
    let actor = actor(&ctx)?;
    let blog = owned_space(&ctx, &actor)?;
    let mut tx = ctx.state.db.begin().await?;
    let removed =
        sqlx::query("DELETE FROM git_sync_configs WHERE blog_id = $1").bind(blog.id).execute(&mut *tx).await?;
    if removed.rows_affected() == 0 {
        return Err(ApiError::not_found("Nessuna sincronizzazione configurata."));
    }
    audit(&mut tx, &ctx, actor.user_id, "git_sync.removed", blog).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn run_git_sync(ctx: RouteCtx) -> Result<(), ApiError> {
    let actor = actor(&ctx)?;
    let blog = owned_space(&ctx, &actor)?;
    let mut tx = ctx.state.db.begin().await?;
    let row: Option<(Uuid, bool, Option<String>)> =
        sqlx::query_as("SELECT id, enabled, last_status FROM git_sync_configs WHERE blog_id = $1 FOR UPDATE")
            .bind(blog.id)
            .fetch_optional(&mut *tx)
            .await?;
    let (id, enabled, status) = row.ok_or_else(|| ApiError::not_found("Nessuna sincronizzazione configurata."))?;
    if !enabled {
        return Err(ApiError::bad_request("La sincronizzazione è disattivata."));
    }
    if status.as_deref() == Some("running") {
        return Err(ApiError::conflict("Una sincronizzazione è già in corso."));
    }
    outbox::enqueue(&mut tx, Kind::Job, SUBJECT_RUN, &json!({"config_id": id, "force": true}))
        .await
        .map_err(ApiError::internal)?;
    tx.commit().await?;
    Ok(())
}
