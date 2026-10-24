//! Handler `custom` del crate: le rotte pubbliche di configurazione e footer.

use noct_core::{ApiError, RouteCtx};
use serde_json::{Value, json};

pub async fn public_config(ctx: RouteCtx) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let mut conn = st.db.acquire().await?;
    let platform = crate::get_in(&mut conn).await?;
    let mut out = json!({
        "deployment_mode": st.module_config_str("auth", "deployment_mode").unwrap_or("platform"),
        "turnstile_site_key": st.config.turnstile_site_key,
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
