//! Testi dell'interfaccia riscrivibili dagli amministratori (home, modulo newsletter): la tabella e le regole
//! stanno nel crate `platform`, che espone anche la lettura pubblica (`GET /site-texts`).

use crate::{
    config::LOCALES,
    handlers_impl::{actor_id, audit},
};
use noct_core::{ApiError, RouteCtx, revalidate};
use noct_platform::{rules, texts_of};
use serde_json::{Value, json};

pub async fn put_site_texts(ctx: RouteCtx, body: crate::AdminSiteTextsIn) -> Result<Value, ApiError> {
    let admin = actor_id(&ctx)?;
    let locale = ctx.path.get("locale").map(|l| l.to_ascii_lowercase()).unwrap_or_default();
    if !LOCALES.contains(&locale.as_str()) {
        return Err(ApiError::bad_request("Lingua non supportata."));
    }
    let changes = rules::parse_changes(&body.texts).map_err(ApiError::bad_request)?;

    let mut tx = ctx.state.db.begin().await?;
    let (mut set, mut cleared) = (Vec::new(), Vec::new());
    for (key, value) in &changes {
        match value {
            Some(v) => {
                sqlx::query(
                    "INSERT INTO site_texts (locale, key, value, updated_by) VALUES ($1, $2, $3, $4)
                     ON CONFLICT (locale, key) DO UPDATE SET value = EXCLUDED.value, updated_by = EXCLUDED.updated_by,
                        updated_at = now()",
                )
                .bind(&locale)
                .bind(key)
                .bind(v)
                .bind(admin)
                .execute(&mut *tx)
                .await?;
                set.push(key.as_str());
            }
            None => {
                sqlx::query("DELETE FROM site_texts WHERE locale = $1 AND key = $2")
                    .bind(&locale)
                    .bind(key)
                    .execute(&mut *tx)
                    .await?;
                cleared.push(key.as_str());
            }
        }
    }
    audit(&mut tx, &ctx, "site_texts.updated", None, json!({"locale": locale, "set": set, "cleared": cleared})).await?;
    let out = texts_of(&mut tx, &locale).await?;
    tx.commit().await?;
    // mostrati su ogni pagina pubblica: un solo tag condiviso
    ctx.state.services.revalidate.spawn(vec![revalidate::site_texts()]);
    Ok(out)
}
