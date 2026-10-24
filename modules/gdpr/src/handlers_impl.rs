//! Handler `custom` del modulo.

use crate::{ANONYMIZED_NAME, AccountDelete, DEADLINE_DAYS, GdprReject, GdprRequestCreate};
use chrono::{DateTime, Utc};
use noct_audit::{Entry, user_actor};
use noct_core::{ApiError, AppState, JsonMap, RouteCtx, StoredObject, parse_uuid};
use serde_json::{Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

fn user_id(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    ctx.actor.as_ref().map(|a| a.user_id).ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))
}

async fn audit(
    conn: &mut PgConnection,
    ctx: &RouteCtx,
    action: &str,
    actor: Uuid,
    target: Uuid,
    payload: Value,
) -> Result<(), ApiError> {
    let actor = user_actor(&mut *conn, actor).await.map_err(ApiError::internal)?;
    noct_audit::record(
        conn,
        Entry { action, actor, target: Some(("user", target)), blog_id: None, meta: &ctx.meta, payload },
    )
    .await
    .map_err(ApiError::internal)
}

#[derive(sqlx::FromRow)]
struct AccountRow {
    username: String,
    email: String,
    display_name: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
    role: String,
    mfa_enabled: bool,
    created_at: DateTime<Utc>,
}

// ---- esportazione e cancellazione ---------------------------------------------------------------------------------

/// Istantanea dei dati dell'utente: i campi dell'account che appartengono al core, più le sezioni di
/// ogni modulo attivo. Include il contenuto scritto ovunque si trovi, non solo sui propri blog.
pub async fn build_export(st: &AppState, user: Uuid) -> Result<Value, ApiError> {
    let a: AccountRow = sqlx::query_as(
        "SELECT username, email, display_name, first_name, last_name, platform_role::text AS role, mfa_enabled, created_at
         FROM users WHERE id = $1",
    )
    .bind(user)
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(|| ApiError::not_found("Utente non trovato."))?;
    let mut data: JsonMap = st.personal_data.export(st, user).await?;
    let mut account = json!({
        "id": user, "username": a.username, "email": a.email, "display_name": a.display_name, "first_name": a.first_name,
        "last_name": a.last_name, "platform_role": a.role, "mfa_enabled": a.mfa_enabled, "created_at": a.created_at,
    });
    // i campi dell'account che appartengono ai moduli (profilo, dominio…) si uniscono a quelli del core
    if let (Some(Value::Object(extra)), Some(obj)) = (data.remove("account"), account.as_object_mut()) {
        obj.extend(extra);
    }
    data.insert("account".into(), account);
    Ok(Value::Object(data))
}

/// Cancella i dati personali in ogni modulo e anonimizza la riga utente, nella transazione `conn`.
/// Restituisce gli oggetti su storage da eliminare dopo il commit.
pub async fn anonymize_account(
    conn: &mut PgConnection,
    st: &AppState,
    user: Uuid,
) -> Result<Vec<StoredObject>, ApiError> {
    let objects = st.personal_data.erase(&mut *conn, st, user).await?;
    sqlx::query(
        "UPDATE users SET username = 'utente-eliminato-' || substr(replace(id::text, '-', ''), 1, 12),
                email = 'deleted-' || id::text || '@notturni.invalid', hashed_password = NULL,
                platform_role = 'utente', is_active = false, mfa_enabled = false, display_name = $2,
                post_author_name_style = 'display_name', credentials_changed_at = now(), updated_at = now()
         WHERE id = $1",
    )
    .bind(user)
    .bind(ANONYMIZED_NAME)
    .execute(&mut *conn)
    .await?;
    Ok(objects)
}

/// Cancella gli oggetti dopo il commit: la riga è già andata, un orfano non deve far fallire la richiesta.
async fn delete_objects(st: &AppState, objects: Vec<StoredObject>) {
    for o in objects {
        if let Err(e) = st.services.storage.delete(&o.bucket, &o.key).await {
            tracing::warn!(error = %e, bucket = o.bucket, key = o.key, "cancellazione dell'oggetto su storage non riuscita");
        }
    }
}

async fn log_request(
    conn: &mut PgConnection,
    user: Uuid,
    username: &str,
    kind: &str,
    status: &str,
    note: Option<&str>,
    created_by: Option<Uuid>,
) -> Result<Uuid, ApiError> {
    Ok(sqlx::query_scalar(
        "INSERT INTO gdpr_requests (user_id, username, type, status, deadline_at, note, created_by_id, completed_at)
         VALUES ($1, $2, $3::gdpr_request_type, $4::gdpr_request_status, now() + make_interval(days => $5), $6, $7,
                 CASE WHEN $4 = 'completed' THEN now() END)
         RETURNING id",
    )
    .bind(user)
    .bind(username)
    .bind(kind)
    .bind(status)
    .bind(DEADLINE_DAYS as i32)
    .bind(note)
    .bind(created_by)
    .fetch_one(conn)
    .await?)
}

pub async fn export_my_data(ctx: RouteCtx) -> Result<Value, ApiError> {
    let me = user_id(&ctx)?;
    let data = build_export(&ctx.state, me).await?;
    let mut tx = ctx.state.db.begin().await?;
    let username: String =
        sqlx::query_scalar("SELECT username FROM users WHERE id = $1").bind(me).fetch_one(&mut *tx).await?;
    log_request(&mut tx, me, &username, "export", "completed", Some("self-service"), None).await?;
    audit(&mut tx, &ctx, "user.data_exported", me, me, json!({})).await?;
    tx.commit().await?;
    Ok(data)
}

pub async fn delete_my_account(ctx: RouteCtx, body: AccountDelete) -> Result<(), ApiError> {
    let st = &ctx.state;
    let me = user_id(&ctx)?;
    let mut tx = st.db.begin().await?;
    let (username, role): (String, String) =
        sqlx::query_as("SELECT username, platform_role::text FROM users WHERE id = $1 FOR UPDATE")
            .bind(me)
            .fetch_one(&mut *tx)
            .await?;
    if body.confirm_username != username {
        return Err(ApiError::bad_request("Username di conferma non corretto."));
    }
    // senza un Super Admin la piattaforma non si amministra più
    if role == "super_admin" {
        return Err(ApiError::bad_request("Un Super Admin non può cancellare il proprio account da qui."));
    }
    // l'evento di audit e la traccia della richiesta si scrivono prima: dopo l'anonimizzazione lo username è perso
    audit(&mut tx, &ctx, "user.account_deleted", me, me, json!({})).await?;
    log_request(&mut tx, me, &username, "deletion", "completed", Some("self-service"), None).await?;
    let objects = anonymize_account(&mut tx, st, me).await?;
    tx.commit().await?;
    delete_objects(st, objects).await;
    Ok(())
}

// ---- coda delle richieste (amministrazione) -----------------------------------------------------------------------

type RequestRow = (
    Uuid,
    String,
    String,
    String,
    DateTime<Utc>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    DateTime<Utc>,
);

const REQUEST_SELECT: &str = "SELECT r.id, r.username, r.type::text, r.status::text, r.deadline_at, r.note,
        (SELECT username FROM users WHERE id = r.created_by_id), (SELECT username FROM users WHERE id = r.approved_by_id),
        r.approved_at, r.completed_at, r.created_at FROM gdpr_requests r";

fn request_json(r: RequestRow) -> Value {
    json!({
        "id": r.0, "username": r.1, "type": r.2, "status": r.3, "deadline_at": r.4, "note": r.5,
        "created_by_username": r.6, "approved_by_username": r.7, "approved_at": r.8, "completed_at": r.9, "created_at": r.10,
    })
}

async fn request_by_id(conn: &mut PgConnection, id: Uuid) -> Result<Value, ApiError> {
    let row: RequestRow = sqlx::query_as(&format!("{REQUEST_SELECT} WHERE r.id = $1")).bind(id).fetch_one(conn).await?;
    Ok(request_json(row))
}

/// Aperte e approvate prima, per scadenza.
pub async fn list_requests(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let filter = match ctx.query.get("status").map(String::as_str) {
        None | Some("") => None,
        Some(s @ ("open" | "approved" | "completed" | "rejected")) => Some(s.to_string()),
        Some(_) => return Err(ApiError::unprocessable("`status` deve essere open, approved, completed o rejected.")),
    };
    let rows: Vec<RequestRow> = sqlx::query_as(&format!(
        "{REQUEST_SELECT} WHERE ($1::text IS NULL OR r.status::text = $1)
         ORDER BY (r.status = 'open') DESC, (r.status = 'approved') DESC, r.deadline_at, r.id"
    ))
    .bind(filter)
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(rows.into_iter().map(request_json).collect())
}

pub async fn create_request(ctx: RouteCtx, body: GdprRequestCreate) -> Result<Value, ApiError> {
    let admin = user_id(&ctx)?;
    let note = body.note.trim().to_string();
    let mut tx = ctx.state.db.begin().await?;
    let user: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE username = $1 AND is_active")
        .bind(body.username.trim())
        .fetch_optional(&mut *tx)
        .await?;
    let user = user.ok_or_else(|| ApiError::not_found("Utente non trovato."))?;
    let id = log_request(&mut tx, user, body.username.trim(), &body.r#type, "open", Some(&note), Some(admin)).await?;
    audit(&mut tx, &ctx, "gdpr.request_created", admin, user, json!({"type": body.r#type, "note": note})).await?;
    let out = request_by_id(&mut tx, id).await?;
    tx.commit().await?;
    Ok(out)
}

/// La richiesta `request_id`, bloccata per la modifica: due amministratori non agiscono insieme.
async fn lock_request(
    conn: &mut PgConnection,
    ctx: &RouteCtx,
) -> Result<(Uuid, Uuid, String, String, Option<Uuid>), ApiError> {
    let id = parse_uuid(ctx.path.get("request_id").map(String::as_str).unwrap_or_default())?;
    let row: Option<(Uuid, String, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT user_id, type::text, status::text, created_by_id FROM gdpr_requests WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    let (user, kind, status, created_by) = row.ok_or_else(|| ApiError::not_found("Richiesta non trovata."))?;
    Ok((id, user, kind, status, created_by))
}

pub async fn approve_request(ctx: RouteCtx) -> Result<Value, ApiError> {
    let admin = user_id(&ctx)?;
    let mut tx = ctx.state.db.begin().await?;
    let (id, user, kind, status, created_by) = lock_request(&mut tx, &ctx).await?;
    if status != "open" {
        return Err(ApiError::bad_request("La richiesta non è aperta."));
    }
    // una cancellazione è irreversibile: serve un secondo paio d'occhi. Un export non ha bisogno di approvazione.
    if kind == "deletion" && created_by == Some(admin) {
        return Err(ApiError::forbidden("Una cancellazione va approvata da un secondo amministratore."));
    }
    sqlx::query("UPDATE gdpr_requests SET status = 'approved', approved_by_id = $2, approved_at = now(), updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(admin)
        .execute(&mut *tx)
        .await?;
    audit(&mut tx, &ctx, "gdpr.request_approved", admin, user, json!({"type": kind})).await?;
    let out = request_by_id(&mut tx, id).await?;
    tx.commit().await?;
    Ok(out)
}

pub async fn reject_request(ctx: RouteCtx, body: GdprReject) -> Result<Value, ApiError> {
    let admin = user_id(&ctx)?;
    let note = body.note.trim().to_string();
    let mut tx = ctx.state.db.begin().await?;
    let (id, user, kind, status, _) = lock_request(&mut tx, &ctx).await?;
    if status == "completed" || status == "rejected" {
        return Err(ApiError::bad_request("La richiesta è già chiusa."));
    }
    sqlx::query(
        "UPDATE gdpr_requests SET status = 'rejected', completed_at = now(), updated_at = now(),
                note = btrim(COALESCE(note, '') || E'\\n[rifiutata] ' || $2) WHERE id = $1",
    )
    .bind(id)
    .bind(&note)
    .execute(&mut *tx)
    .await?;
    audit(&mut tx, &ctx, "gdpr.request_rejected", admin, user, json!({"type": kind, "note": note})).await?;
    let out = request_by_id(&mut tx, id).await?;
    tx.commit().await?;
    Ok(out)
}

/// Un export (aperto o approvato) restituisce il JSON dei dati e chiude la richiesta; una cancellazione
/// deve essere stata approvata da un secondo amministratore e anonimizza l'account.
pub async fn execute_request(ctx: RouteCtx) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let admin = user_id(&ctx)?;
    let mut tx = st.db.begin().await?;
    let (id, user, kind, status, _) = lock_request(&mut tx, &ctx).await?;
    let (active, role): (bool, String) =
        sqlx::query_as("SELECT is_active, platform_role::text FROM users WHERE id = $1 FOR UPDATE")
            .bind(user)
            .fetch_one(&mut *tx)
            .await?;
    if kind == "export" {
        if status != "open" && status != "approved" {
            return Err(ApiError::bad_request("La richiesta è già chiusa."));
        }
        let data = build_export(st, user).await?;
        sqlx::query(
            "UPDATE gdpr_requests SET status = 'completed', completed_at = now(), updated_at = now() WHERE id = $1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        audit(&mut tx, &ctx, "gdpr.export_executed", admin, user, json!({})).await?;
        tx.commit().await?;
        return Ok(data);
    }
    if status != "approved" {
        return Err(ApiError::bad_request(
            "La cancellazione richiede prima l'approvazione di un secondo amministratore.",
        ));
    }
    if role == "super_admin" {
        return Err(ApiError::bad_request("Non è possibile cancellare un Super Admin da qui."));
    }
    if !active {
        return Err(ApiError::bad_request("L'account è già stato cancellato."));
    }
    let username: String =
        sqlx::query_scalar("SELECT username FROM users WHERE id = $1").bind(user).fetch_one(&mut *tx).await?;
    audit(&mut tx, &ctx, "gdpr.deletion_executed", admin, user, json!({"username": username})).await?;
    sqlx::query(
        "UPDATE gdpr_requests SET status = 'completed', completed_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    let objects = anonymize_account(&mut tx, st, user).await?;
    tx.commit().await?;
    delete_objects(st, objects).await;
    Ok(json!({"status": "completed"}))
}
