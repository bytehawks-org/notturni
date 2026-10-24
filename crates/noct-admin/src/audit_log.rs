use chrono::{DateTime, Utc};
use noct_core::{ApiError, RouteCtx};
use serde_json::{Value, json};
use sqlx::FromRow;
use uuid::Uuid;

/// Canale da cui è partita l'azione, derivato da `actor_type` (nessuna colonna dedicata).
fn channel(actor_type: &str) -> &'static str {
    match actor_type {
        "core_token" | "user_token" => "api",
        "system" => "system",
        _ => "web",
    }
}

fn channel_types(channel: &str) -> Result<&'static [&'static str], ApiError> {
    match channel {
        "web" => Ok(&["user", "anonymous"]),
        "api" => Ok(&["core_token", "user_token"]),
        "system" => Ok(&["system"]),
        _ => Err(ApiError::unprocessable("`channel` deve essere web, api o system.")),
    }
}

#[derive(FromRow)]
struct Row {
    id: Uuid,
    occurred_at: DateTime<Utc>,
    actor_type: String,
    actor_id: Option<Uuid>,
    actor_label: Option<String>,
    action: String,
    target_type: Option<String>,
    target_id: Option<Uuid>,
    blog_id: Option<Uuid>,
    ip: Option<String>,
    user_agent: Option<String>,
    payload: Value,
}

fn uuid_param(ctx: &RouteCtx, key: &str) -> Result<Option<Uuid>, ApiError> {
    ctx.query
        .get(key)
        .filter(|v| !v.is_empty())
        .map(|v| Uuid::parse_str(v).map_err(|_| ApiError::unprocessable(format!("`{key}` non è un UUID."))))
        .transpose()
}

fn time_param(ctx: &RouteCtx, key: &str) -> Result<Option<DateTime<Utc>>, ApiError> {
    ctx.query
        .get(key)
        .filter(|v| !v.is_empty())
        .map(|v| {
            DateTime::parse_from_rfc3339(v)
                .map(|d| d.with_timezone(&Utc))
                .map_err(|_| ApiError::unprocessable(format!("`{key}` non è una data RFC 3339.")))
        })
        .transpose()
}

fn int_param(ctx: &RouteCtx, key: &str, default: i64) -> Result<i64, ApiError> {
    Ok(ctx
        .query
        .get(key)
        .filter(|v| !v.is_empty())
        .map(|v| {
            v.trim().parse::<i64>().map_err(|_| ApiError::unprocessable(format!("`{key}` deve essere un intero.")))
        })
        .transpose()?
        .unwrap_or(default))
}

/// Registro di audit (append-only), dal più recente. Gli eventi oltre la conservazione stanno negli
/// archivi su storage (job `audit.maintenance`).
pub async fn list_audit_log(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let channel_filter: Option<Vec<String>> = ctx
        .query
        .get("channel")
        .filter(|v| !v.is_empty())
        .map(|c| channel_types(c).map(|t| t.iter().map(|s| s.to_string()).collect()))
        .transpose()?;
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, occurred_at, actor_type::text AS actor_type, actor_id, actor_label, action, target_type, target_id,
                blog_id, ip, user_agent, payload
         FROM audit_log
         WHERE ($1::text IS NULL OR action = $1) AND ($2::uuid IS NULL OR actor_id = $2)
           AND ($3::uuid IS NULL OR target_id = $3) AND ($4::uuid IS NULL OR blog_id = $4)
           AND ($5::text[] IS NULL OR actor_type::text = ANY($5))
           AND ($6::timestamptz IS NULL OR occurred_at >= $6) AND ($7::timestamptz IS NULL OR occurred_at < $7)
         ORDER BY occurred_at DESC, id LIMIT $8 OFFSET $9",
    )
    .bind(ctx.query.get("action").filter(|v| !v.is_empty()))
    .bind(uuid_param(&ctx, "actor_id")?)
    .bind(uuid_param(&ctx, "target_id")?)
    .bind(uuid_param(&ctx, "blog_id")?)
    .bind(channel_filter)
    .bind(time_param(&ctx, "since")?)
    .bind(time_param(&ctx, "until")?)
    .bind(int_param(&ctx, "limit", 100)?.clamp(1, 500))
    .bind(int_param(&ctx, "offset", 0)?.max(0))
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id, "occurred_at": r.occurred_at, "actor_type": r.actor_type, "channel": channel(&r.actor_type),
                "actor_id": r.actor_id, "actor_label": r.actor_label, "action": r.action,
                "target_type": r.target_type, "target_id": r.target_id, "blog_id": r.blog_id, "ip": r.ip,
                "user_agent": r.user_agent, "payload": r.payload,
            })
        })
        .collect())
}
