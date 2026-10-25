//! Handler `custom` del crate: le rotte pubbliche di configurazione e footer.

use noct_core::{ApiError, RouteCtx};
use serde_json::{Map, Value, json};

pub async fn public_config(ctx: RouteCtx) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let mut conn = st.db.acquire().await?;
    let platform = crate::get_in(&mut conn).await?;
    let mut out = json!({
        "deployment_mode": st.module_config_str("auth", "deployment_mode").unwrap_or("platform"),
        "turnstile_site_key": st.config.turnstile_site_key,
        "experimental_features": st.config.experimental_features,
        "default_locale": platform.default_locale,
        "registration_mode": platform.registration_mode,
        "sso_providers": [],
    });
    out.as_object_mut().expect("oggetto").extend(st.admin_sections.public_config(st).await?);
    Ok(out)
}

pub async fn public_footer(ctx: RouteCtx) -> Result<Value, ApiError> {
    let row: (Option<String>, Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT footer_column1_markdown, footer_column2_markdown, footer_column3_markdown, footer_bottom_bar_markdown
         FROM platform_config WHERE id = 1",
    )
    .fetch_optional(&ctx.state.db)
    .await?
    .unwrap_or_default();
    Ok(json!({"column1": row.0, "column2": row.1, "column3": row.2, "bottom_bar": row.3}))
}

// ---- testi dell'interfaccia ------------------------------------------------------------------------------------------

pub async fn texts_of(conn: &mut sqlx::PgConnection, locale: &str) -> Result<Value, ApiError> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM site_texts WHERE locale = $1 ORDER BY key")
            .bind(locale)
            .fetch_all(conn)
            .await?;
    let texts: Map<String, Value> = rows.into_iter().map(|(k, v)| (k, Value::String(v))).collect();
    Ok(json!({"locale": locale, "texts": texts}))
}

pub async fn get_site_texts(ctx: RouteCtx) -> Result<Value, ApiError> {
    let locale = ctx.query.get("locale").map(|l| l.to_ascii_lowercase()).unwrap_or_default();
    crate::rules::validate_text_locale(&locale).map_err(ApiError::bad_request)?;
    let mut conn = ctx.state.db.acquire().await?;
    texts_of(&mut conn, &locale).await
}
