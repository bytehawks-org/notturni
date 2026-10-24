use noct_core::{ApiError, RouteCtx};
use serde_json::{Value, json};

/// Panoramica di piattaforma: KPI, code aperte, audit di oggi, salute dei servizi. Un moderatore vede
/// gli stessi numeri (sono aggregati, nessun dato personale). Le code dei moduli opzionali valgono 0
/// finché il modulo è spento; quando è attivo il modulo sovrascrive il contatore.
pub async fn admin_overview(ctx: RouteCtx) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let (
        users_total,
        users_new_7d,
        blogs_total,
        blogs_suspended,
        posts_published,
        posts_in_review,
        hidden,
        audit_today,
    ): (i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM users),
                (SELECT count(*) FROM users WHERE created_at >= now() - interval '7 days'),
                (SELECT count(*) FROM blogs),
                (SELECT count(*) FROM blogs WHERE is_suspended),
                (SELECT count(*) FROM posts WHERE status = 'published' AND published_at <= now()),
                (SELECT count(*) FROM posts WHERE status = 'pending_review'),
                (SELECT count(*) FROM posts WHERE is_hidden),
                (SELECT count(*) FROM audit_log WHERE occurred_at >= date_trunc('day', now()))",
    )
    .fetch_one(&st.db)
    .await?;
    let mut out = json!({
        "users_total": users_total, "users_new_7d": users_new_7d, "blogs_total": blogs_total,
        "blogs_suspended": blogs_suspended, "posts_published": posts_published,
        "queue_pending_comments": 0, "queue_posts_in_review": posts_in_review, "queue_hidden_posts": hidden,
        "queue_open_reports": 0, "audit_today": audit_today,
        "deployment_mode": st.module_config_str("auth", "deployment_mode").unwrap_or("platform"),
    });
    let map = out.as_object_mut().expect("oggetto");
    map.extend(st.admin_sections.overview(st).await?);

    let mut services = vec![json!({"name": "postgres", "status": "ok", "detail": null})];
    for (name, backend, result) in st.services.check().await {
        services.push(json!({
            "name": name, "status": if result.is_ok() { "ok" } else { "down" }, "detail": backend,
        }));
    }
    services.push(match st.services.moderation.health().await {
        None => json!({"name": "moderation", "status": "unconfigured", "detail": null}),
        Some(up) => json!({"name": "moderation", "status": if up { "ok" } else { "down" }, "detail": null}),
    });
    map.insert("services".into(), Value::Array(services));
    Ok(out)
}
