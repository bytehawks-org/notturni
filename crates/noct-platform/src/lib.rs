//! Impostazioni di piattaforma. Questo crate espone la lettura (con creazione lazy
//! della riga di default); le rotte di amministrazione arrivano con la fase operazioni.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

mod handlers_impl;
pub mod rules;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}

pub use handlers_impl::texts_of;

use noct_core::ApiError;
use sqlx::{FromRow, PgConnection, PgPool};

#[derive(Debug, Clone, FromRow)]
pub struct PlatformConfig {
    pub default_locale: String,
    pub registration_mode: String,
    pub reserved_blog_names: Vec<String>,
    pub moderation_threshold: f64,
    pub max_blogs_per_user: i32,
    pub max_blog_storage_mb: Option<i32>,
    pub audit_retention_days: i32,
}

const SELECT: &str = "SELECT default_locale, registration_mode, reserved_blog_names, moderation_threshold, max_blogs_per_user, max_blog_storage_mb, audit_retention_days FROM platform_config WHERE id = 1";

/// La configurazione corrente. Su un'installazione nuova la riga viene creata con i default
/// (`ON CONFLICT DO NOTHING`: due prime richieste concorrenti non si ostacolano).
pub async fn get(db: &PgPool) -> Result<PlatformConfig, ApiError> {
    sqlx::query("INSERT INTO platform_config (id) VALUES (1) ON CONFLICT (id) DO NOTHING").execute(db).await?;
    Ok(sqlx::query_as::<_, PlatformConfig>(SELECT).fetch_one(db).await?)
}

/// Come [`get`], dentro una transazione già aperta.
pub async fn get_in(conn: &mut PgConnection) -> Result<PlatformConfig, ApiError> {
    sqlx::query("INSERT INTO platform_config (id) VALUES (1) ON CONFLICT (id) DO NOTHING").execute(&mut *conn).await?;
    Ok(sqlx::query_as::<_, PlatformConfig>(SELECT).fetch_one(&mut *conn).await?)
}
