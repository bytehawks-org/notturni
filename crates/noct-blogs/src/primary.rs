//! Blog primario dell'istanza in modalità `solo`: creato in modo idempotente alla prima
//! registrazione e all'avvio, marcato in `platform_config.primary_blog_id`.

use crate::rules;
use noct_audit::{AuditActor, Entry};
use noct_core::{ApiError, RequestMeta};
use serde_json::json;
use sqlx::PgConnection;
use uuid::Uuid;

/// Lock transazionale condiviso con la registrazione (`noct-auth`): serializza la prima
/// registrazione, l'avvio e le repliche che partono insieme.
pub const REGISTRATION_LOCK: i64 = 727_274_001;

/// Dati del sito usati per nominare il blog primario.
#[derive(Debug, Clone, Default)]
pub struct SiteInfo {
    /// `NOCT_SITE_NAME`; se assente si usa lo username del proprietario.
    pub name: Option<String>,
}

impl SiteInfo {
    /// Legge `auth.site_name` (`NOCT_SITE_NAME`) dalla configurazione dei moduli.
    pub fn from_state(st: &noct_core::AppState) -> Self {
        Self { name: st.module_config_str("auth", "site_name").map(str::to_string) }
    }
}

/// Slug derivato da un nome libero: minuscole ASCII, cifre e trattini; mai vuoto.
pub fn slugify(input: &str) -> String {
    let mut out = String::new();
    for c in input.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    // lascia spazio al suffisso `-N` entro i 63 caratteri
    let out = out.trim_end_matches('-');
    let out: String = out.chars().take(55).collect();
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "blog".into() } else { out }
}

/// Logga (una sola riga, solo slug) i blog esistenti in collisione con le rotte del sito in
/// modalità solo, escluso il blog primario. Non rinomina né cancella nulla: un errore della
/// query viene solo loggato, non blocca l'avvio.
async fn warn_route_collisions(conn: &mut PgConnection, primary: Option<Uuid>) -> Result<(), ApiError> {
    let reserved: Vec<&str> = rules::SOLO_ROUTE_COLLISIONS.to_vec();
    let found: Result<Vec<String>, sqlx::Error> =
        sqlx::query_scalar("SELECT slug FROM blogs WHERE slug = ANY($1) AND id IS DISTINCT FROM $2 ORDER BY slug")
            .bind(reserved)
            .bind(primary)
            .fetch_all(&mut *conn)
            .await;
    match found {
        Ok(slugs) if !slugs.is_empty() => {
            tracing::warn!(slugs = ?slugs, "blog solo con slug in collisione con le rotte del sito");
        }
        Ok(_) => {}
        Err(e) => tracing::error!(error = %e, "controllo delle collisioni di slug non riuscito"),
    }
    Ok(())
}

/// Garantisce che esista il blog primario. `Ok(None)` se c'era già (marker valido), `Ok(Some(id))`
/// se lo ha creato. Il marker vale solo se punta a un blog esistente: dopo una cancellazione
/// definitiva (`purge_deleted`) il blog viene ricreato al prossimo avvio. Un blog in tolleranza
/// (`deleted_at` valorizzato) esiste ancora e resta ripristinabile.
///
/// Da chiamare dentro una transazione: prende [`REGISTRATION_LOCK`] e scrive blog, marker e
/// audit (`blog.primary_created`, attore sistema) insieme.
pub async fn ensure_primary_blog(
    conn: &mut PgConnection,
    owner_id: Uuid,
    site: &SiteInfo,
) -> Result<Option<Uuid>, ApiError> {
    sqlx::query("SELECT pg_advisory_xact_lock($1)").bind(REGISTRATION_LOCK).execute(&mut *conn).await?;
    let platform = noct_platform::get_in(&mut *conn).await?;
    let current: Option<Uuid> =
        sqlx::query_scalar("SELECT primary_blog_id FROM platform_config WHERE id = 1 FOR UPDATE")
            .fetch_one(&mut *conn)
            .await?;
    warn_route_collisions(conn, current).await?;
    if let Some(id) = current {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM blogs WHERE id = $1)")
            .bind(id)
            .fetch_one(&mut *conn)
            .await?;
        if exists {
            return Ok(None);
        }
    }
    let username: String =
        sqlx::query_scalar("SELECT username FROM users WHERE id = $1").bind(owner_id).fetch_one(&mut *conn).await?;
    let name = site.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&username);
    let title: String = name.chars().take(255).collect();
    let base = slugify(name);
    let mut n = 1u32;
    let slug = loop {
        let candidate = if n == 1 { base.clone() } else { format!("{base}-{n}") };
        n += 1;
        if rules::validate_slug_solo(&candidate).is_err() {
            continue;
        }
        let taken: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM blogs WHERE slug = $1)")
            .bind(&candidate)
            .fetch_one(&mut *conn)
            .await?;
        if !taken {
            break candidate;
        }
    };
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO blogs (slug, title, owner_id, visibility, default_locale, kind, display_mode)
         VALUES ($1, $2, $3, 'public', $4, 'blog', 'feed') RETURNING id",
    )
    .bind(&slug)
    .bind(&title)
    .bind(owner_id)
    .bind(&platform.default_locale)
    .fetch_one(&mut *conn)
    .await?;
    sqlx::query("UPDATE platform_config SET primary_blog_id = $1 WHERE id = 1").bind(id).execute(&mut *conn).await?;
    noct_audit::record(
        conn,
        Entry {
            action: "blog.primary_created",
            actor: AuditActor::System,
            target: Some(("blog", id)),
            blog_id: Some(id),
            meta: &RequestMeta::default(),
            payload: json!({ "slug": slug, "owner_id": owner_id }),
        },
    )
    .await
    .map_err(ApiError::internal)?;
    Ok(Some(id))
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn site_named_like_a_root_page_gets_a_suffixed_solo_slug() {
        // stessa logica del ciclo in `ensure_primary_blog`: i candidati riservati vengono saltati
        let base = slugify("Media");
        assert_eq!(base, "media");
        assert!(crate::rules::validate_slug_solo(&base).is_err());
        assert!(crate::rules::validate_slug_solo("media-2").is_ok());
    }

    #[test]
    fn slugify_produces_valid_slugs() {
        assert_eq!(slugify("Il Mio Diario"), "il-mio-diario");
        assert_eq!(slugify("  --Anna_Rossi.. "), "anna-rossi");
        assert_eq!(slugify("Città"), "citt");
        assert_eq!(slugify("???"), "blog");
        assert_eq!(slugify(&"a".repeat(100)).len(), 55);
    }
}
