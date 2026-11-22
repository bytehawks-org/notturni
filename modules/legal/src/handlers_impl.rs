//! Handler `custom` del modulo. Il testo sorgente sta in `legal_documents`; pubblicare scrive il testo risolto
//! (dati del titolare e funzioni attive inclusi) nella pagina del sito con lo stesso indirizzo.

use crate::{
    LegalDocIn, LegalPublishIn, LegalResetIn, LegalSettingsIn,
    engine::{Context, Rendered, render},
    facts::{Facts, SETTINGS_SELECT, Settings, compute, today},
    templates::{DOCS, TEMPLATE_LOCALES, slug, template},
};
use chrono::{DateTime, Utc};
use noct_audit::{Entry, user_actor};
use noct_core::{ApiError, AppState, RouteCtx, revalidate};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgConnection};
use std::collections::BTreeMap;
use uuid::Uuid;

const MAX_LINE: usize = 1000;

#[derive(FromRow, Clone)]
struct DocRow {
    doc: String,
    locale: String,
    title: String,
    content: String,
    customized: bool,
    published_hash: Option<String>,
    published_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

const DOC_SELECT: &str =
    "SELECT doc, locale, title, content, customized, published_hash, published_at, updated_at FROM legal_documents";

fn admin(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    ctx.actor.as_ref().map(|a| a.user_id).ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))
}

/// `(doc, locale)` dal percorso, validati.
fn target(ctx: &RouteCtx) -> Result<(String, String), ApiError> {
    let doc = ctx.path.get("doc").cloned().unwrap_or_default();
    let locale = ctx.path.get("locale").map(|l| l.to_ascii_lowercase()).unwrap_or_default();
    if !DOCS.contains(&doc.as_str()) {
        return Err(ApiError::not_found("Documento sconosciuto (privacy o cookie)."));
    }
    if locale.len() != 2 || !locale.chars().all(|c| c.is_ascii_lowercase()) {
        return Err(ApiError::bad_request("Lingua non valida: due lettere minuscole (es. it, en)."));
    }
    Ok((doc, locale))
}

async fn audit(
    conn: &mut PgConnection,
    ctx: &RouteCtx,
    user: Uuid,
    action: &str,
    payload: Value,
) -> Result<(), ApiError> {
    let who = user_actor(&mut *conn, user).await.map_err(ApiError::internal)?;
    noct_audit::record(conn, Entry { action, actor: who, target: None, blog_id: None, meta: &ctx.meta, payload })
        .await
        .map_err(ApiError::internal)
}

/// I modelli forniti sono sempre presenti; quelli non personalizzati seguono la versione installata.
async fn ensure_defaults(conn: &mut PgConnection) -> Result<(), ApiError> {
    for doc in DOCS {
        for locale in TEMPLATE_LOCALES {
            let Some((title, content)) = template(doc, locale) else { continue };
            sqlx::query(
                "INSERT INTO legal_documents (doc, locale, title, content) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (doc, locale) DO UPDATE SET title = EXCLUDED.title, content = EXCLUDED.content, updated_at = now()
                 WHERE legal_documents.customized = false
                   AND (legal_documents.title <> EXCLUDED.title OR legal_documents.content <> EXCLUDED.content)",
            )
            .bind(doc)
            .bind(locale)
            .bind(title)
            .bind(content)
            .execute(&mut *conn)
            .await?;
        }
    }
    Ok(())
}

fn resolve(row: &DocRow, facts: &Facts, date: &str) -> Rendered {
    let mut values = facts.values.clone();
    values.insert("last_update".into(), date.to_string());
    let mut flags = facts.flags.clone();
    // `last_update` è un valore sempre presente: non serve come funzione
    flags.remove("last_update");
    render(&row.content, &Context { flags: &flags, values: &values })
}

fn fingerprint(title: &str, text: &str) -> String {
    let mut h = Sha256::new();
    h.update(title.as_bytes());
    h.update([0]);
    h.update(text.as_bytes());
    hex::encode(h.finalize())
}

struct View {
    rendered: Rendered,
    published: bool,
    stale: bool,
}

fn view(row: &DocRow, facts: &Facts) -> View {
    let published = row.published_at.is_some();
    // per capire se è cambiato qualcosa si ripete la risoluzione con la data della pubblicazione
    let at = row.published_at.map_or_else(today, |d| d.format("%Y-%m-%d").to_string());
    let probe = resolve(row, facts, &at);
    let stale = published && row.published_hash.as_deref() != Some(fingerprint(&row.title, &probe.text).as_str());
    let rendered = if published { resolve(row, facts, &today()) } else { probe };
    View { rendered, published, stale }
}

fn summary(row: &DocRow, v: &View) -> Value {
    json!({
        "doc": row.doc, "locale": row.locale, "title": row.title, "customized": row.customized,
        "published": v.published, "stale": v.stale, "missing": v.rendered.missing, "issues": v.rendered.issues,
        "has_template": template(&row.doc, &row.locale).is_some(), "published_at": row.published_at, "updated_at": row.updated_at,
    })
}

fn full(row: &DocRow, v: &View) -> Value {
    let mut out = summary(row, v);
    let o = out.as_object_mut().expect("oggetto");
    o.insert("content".into(), json!(row.content));
    o.insert("rendered".into(), json!(v.rendered.text));
    o.insert("default_content".into(), template(&row.doc, &row.locale).map_or(Value::Null, |(_, c)| json!(c)));
    out
}

async fn fetch_row(conn: &mut PgConnection, doc: &str, locale: &str) -> Result<Option<DocRow>, ApiError> {
    Ok(sqlx::query_as(&format!("{DOC_SELECT} WHERE doc = $1 AND locale = $2"))
        .bind(doc)
        .bind(locale)
        .fetch_optional(conn)
        .await?)
}

async fn doc_response(st: &AppState, conn: &mut PgConnection, doc: &str, locale: &str) -> Result<Value, ApiError> {
    let row = fetch_row(conn, doc, locale)
        .await?
        .ok_or_else(|| ApiError::not_found("Documento non trovato in questa lingua."))?;
    let facts = compute(st, &Settings::load(&st.db).await?).await?;
    Ok(full(&row, &view(&row, &facts)))
}

async fn overview(st: &AppState, conn: &mut PgConnection) -> Result<Value, ApiError> {
    let settings = Settings::load_in(conn).await?;
    let facts = compute(st, &settings).await?;
    let rows: Vec<DocRow> = sqlx::query_as(&format!("{DOC_SELECT} ORDER BY doc, locale")).fetch_all(conn).await?;
    let documents: Vec<Value> = rows.iter().map(|r| summary(r, &view(r, &facts))).collect();
    let shown = ["instance_fqdn", "session_days", "audit_days", "deletion_days", "grace_days"];
    let values: BTreeMap<&str, &str> =
        shown.iter().map(|k| (*k, facts.values.get(*k).map_or("", String::as_str))).collect();
    let filled: BTreeMap<String, String> = settings.values().into_iter().filter(|(_, v)| !v.is_empty()).collect();
    Ok(json!({
        "settings": filled,
        "values": values,
        "flags": facts.flags,
        "documents": documents,
        "template_locales": TEMPLATE_LOCALES,
    }))
}

pub async fn get_legal(ctx: RouteCtx) -> Result<Value, ApiError> {
    admin(&ctx)?;
    let mut conn = ctx.state.db.acquire().await?;
    ensure_defaults(&mut conn).await?;
    overview(&ctx.state, &mut conn).await
}

// ---- pubblicazione ------------------------------------------------------------------------------------------------

fn problems(r: &Rendered) -> Option<String> {
    if r.missing.is_empty() && r.issues.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    if !r.missing.is_empty() {
        parts.push(format!("dati mancanti: {}", r.missing.join(", ")));
    }
    if !r.issues.is_empty() {
        parts.push(format!("errori nel testo: {}", r.issues.join("; ")));
    }
    Some(parts.join(" — "))
}

/// Scrive il testo risolto nella pagina del sito; le traduzioni della stessa pagina condividono il gruppo.
async fn write_page(conn: &mut PgConnection, slug: &str, row: &DocRow, text: &str, user: Uuid) -> Result<(), ApiError> {
    let group: Option<Uuid> =
        sqlx::query_scalar("SELECT translation_group_id FROM platform_pages WHERE slug = $1 LIMIT 1")
            .bind(slug)
            .fetch_optional(&mut *conn)
            .await?;
    sqlx::query(
        "INSERT INTO platform_pages (slug, locale, translation_group_id, title, content, is_published, updated_by_id)
         VALUES ($1, $2, COALESCE($3, gen_random_uuid()), $4, $5, true, $6)
         ON CONFLICT (slug, locale) DO UPDATE SET title = EXCLUDED.title, content = EXCLUDED.content, is_published = true,
            updated_by_id = EXCLUDED.updated_by_id, updated_at = now()",
    )
    .bind(slug)
    .bind(&row.locale)
    .bind(group)
    .bind(&row.title)
    .bind(text)
    .bind(user)
    .execute(conn)
    .await?;
    Ok(())
}

fn revalidate_pages(st: &AppState, slugs: &[&str]) {
    let mut tags = vec![revalidate::platform_pages()];
    tags.extend(slugs.iter().map(|s| revalidate::platform_page(s)));
    st.services.revalidate.spawn(tags);
}

pub async fn publish_legal_doc(ctx: RouteCtx, body: LegalPublishIn) -> Result<Value, ApiError> {
    let user = admin(&ctx)?;
    let (doc, locale) = target(&ctx)?;
    let st = &ctx.state;
    let mut tx = st.db.begin().await?;
    ensure_defaults(&mut tx).await?;
    let row = fetch_row(&mut tx, &doc, &locale)
        .await?
        .ok_or_else(|| ApiError::not_found("Documento non trovato in questa lingua."))?;
    let facts = compute(st, &Settings::load_in(&mut tx).await?).await?;
    let text = resolve(&row, &facts, &today());
    if let Some(p) = problems(&text) {
        return Err(ApiError::bad_request(format!("Non si può pubblicare: {p}.")));
    }
    let slug = slug(&doc).expect("documento valido");
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM platform_pages WHERE slug = $1 AND locale = $2)")
            .bind(slug)
            .bind(&locale)
            .fetch_one(&mut *tx)
            .await?;
    if exists && row.published_at.is_none() && !body.replace_existing.unwrap_or(false) {
        return Err(ApiError::conflict(format!(
            "Esiste già una pagina /p/{slug} in lingua {locale} che non è stata creata da qui: confermane la sostituzione."
        )));
    }
    write_page(&mut tx, slug, &row, &text.text, user).await?;
    sqlx::query("UPDATE legal_documents SET published_at = now(), published_hash = $3 WHERE doc = $1 AND locale = $2")
        .bind(&doc)
        .bind(&locale)
        .bind(fingerprint(&row.title, &resolve(&row, &facts, &today()).text))
        .execute(&mut *tx)
        .await?;
    audit(&mut tx, &ctx, user, "legal.document_published", json!({"doc": doc, "locale": locale, "slug": slug})).await?;
    let out = doc_response(st, &mut tx, &doc, &locale).await?;
    tx.commit().await?;
    revalidate_pages(st, &[slug]);
    Ok(out)
}

pub async fn unpublish_legal_doc(ctx: RouteCtx) -> Result<Value, ApiError> {
    let user = admin(&ctx)?;
    let (doc, locale) = target(&ctx)?;
    let st = &ctx.state;
    let slug = slug(&doc).expect("documento valido");
    let mut tx = st.db.begin().await?;
    fetch_row(&mut tx, &doc, &locale)
        .await?
        .ok_or_else(|| ApiError::not_found("Documento non trovato in questa lingua."))?;
    sqlx::query("UPDATE platform_pages SET is_published = false, updated_at = now() WHERE slug = $1 AND locale = $2")
        .bind(slug)
        .bind(&locale)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE legal_documents SET published_at = NULL, published_hash = NULL WHERE doc = $1 AND locale = $2")
        .bind(&doc)
        .bind(&locale)
        .execute(&mut *tx)
        .await?;
    audit(&mut tx, &ctx, user, "legal.document_unpublished", json!({"doc": doc, "locale": locale})).await?;
    let out = doc_response(st, &mut tx, &doc, &locale).await?;
    tx.commit().await?;
    revalidate_pages(st, &[slug]);
    Ok(out)
}

// ---- dati del titolare ----------------------------------------------------------------------------------------------

fn clean(name: &str, v: &Option<String>, max: usize) -> Result<Option<String>, ApiError> {
    let Some(v) = v.as_deref().map(str::trim).filter(|s| !s.is_empty()) else { return Ok(None) };
    if v.chars().count() > max.min(MAX_LINE) || v.chars().any(char::is_control) || v.contains("{{") || v.contains("}}")
    {
        return Err(ApiError::bad_request(format!(
            "`{name}`: testo su una sola riga, fino a {max} caratteri, senza «{{{{» o «}}}}»."
        )));
    }
    Ok(Some(v.to_string()))
}

pub async fn put_legal_settings(ctx: RouteCtx, body: LegalSettingsIn) -> Result<Value, ApiError> {
    let user = admin(&ctx)?;
    let st = &ctx.state;
    let minimum_age = match body.minimum_age {
        None => None,
        Some(a) if (10..=18).contains(&a) => Some(a as i32),
        Some(_) => return Err(ApiError::bad_request("`minimum_age`: tra 10 e 18 anni.")),
    };
    let new = Settings {
        controller_name: clean("controller_name", &body.controller_name, 255)?,
        controller_address: clean("controller_address", &body.controller_address, 500)?,
        controller_email: clean("controller_email", &body.controller_email, 255)?,
        controller_registry: clean("controller_registry", &body.controller_registry, 255)?,
        dpo_contact: clean("dpo_contact", &body.dpo_contact, 500)?,
        hosting_description: clean("hosting_description", &body.hosting_description, 500)?,
        processors: clean("processors", &body.processors, 1000)?,
        backup_retention: clean("backup_retention", &body.backup_retention, 255)?,
        audit_archive_retention: clean("audit_archive_retention", &body.audit_archive_retention, 255)?,
        minimum_age,
    };
    if new.controller_email.as_deref().is_some_and(|e| !e.contains('@') || e.contains(' ')) {
        return Err(ApiError::bad_request("`controller_email`: indirizzo email non valido."));
    }
    let old = Settings::load(&st.db).await?;
    let facts = compute(st, &new).await?;

    let mut tx = st.db.begin().await?;
    ensure_defaults(&mut tx).await?;
    // i documenti pubblicati vengono riscritti con i nuovi valori: non devono restare incompleti
    let published: Vec<DocRow> =
        sqlx::query_as(&format!("{DOC_SELECT} WHERE published_at IS NOT NULL ORDER BY doc, locale"))
            .fetch_all(&mut *tx)
            .await?;
    let date = today();
    let mut rewrites = Vec::new();
    for row in &published {
        let text = resolve(row, &facts, &date);
        if let Some(p) = problems(&text) {
            return Err(ApiError::bad_request(format!(
                "Il documento «{}» ({}) è pubblicato e con questi dati risulterebbe incompleto: {p}.",
                row.doc, row.locale
            )));
        }
        rewrites.push((row, text));
    }
    sqlx::query(
        "INSERT INTO legal_settings (id, controller_name, controller_address, controller_email, controller_registry, dpo_contact,
                hosting_description, processors, backup_retention, audit_archive_retention, minimum_age, updated_by)
         VALUES (1, $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         ON CONFLICT (id) DO UPDATE SET controller_name = EXCLUDED.controller_name, controller_address = EXCLUDED.controller_address,
            controller_email = EXCLUDED.controller_email, controller_registry = EXCLUDED.controller_registry,
            dpo_contact = EXCLUDED.dpo_contact, hosting_description = EXCLUDED.hosting_description,
            processors = EXCLUDED.processors, backup_retention = EXCLUDED.backup_retention,
            audit_archive_retention = EXCLUDED.audit_archive_retention, minimum_age = EXCLUDED.minimum_age,
            updated_by = EXCLUDED.updated_by, updated_at = now()",
    )
    .bind(&new.controller_name)
    .bind(&new.controller_address)
    .bind(&new.controller_email)
    .bind(&new.controller_registry)
    .bind(&new.dpo_contact)
    .bind(&new.hosting_description)
    .bind(&new.processors)
    .bind(&new.backup_retention)
    .bind(&new.audit_archive_retention)
    .bind(new.minimum_age)
    .bind(user)
    .execute(&mut *tx)
    .await?;
    let mut slugs = Vec::new();
    for (row, text) in rewrites {
        let s = slug(&row.doc).expect("documento valido");
        write_page(&mut tx, s, row, &text.text, user).await?;
        sqlx::query(
            "UPDATE legal_documents SET published_at = now(), published_hash = $3 WHERE doc = $1 AND locale = $2",
        )
        .bind(&row.doc)
        .bind(&row.locale)
        .bind(fingerprint(&row.title, &text.text))
        .execute(&mut *tx)
        .await?;
        slugs.push(s);
    }
    let changed: Vec<String> = {
        let (a, b) = (old.values(), new.values());
        a.iter().filter(|(k, v)| b.get(*k) != Some(*v)).map(|(k, _)| k.clone()).collect()
    };
    audit(&mut tx, &ctx, user, "legal.settings_updated", json!({"fields": changed, "republished": slugs.len()}))
        .await?;
    let out = overview(st, &mut tx).await?;
    tx.commit().await?;
    if !slugs.is_empty() {
        revalidate_pages(st, &slugs);
    }
    Ok(out)
}

// ---- documenti ------------------------------------------------------------------------------------------------------

pub async fn get_legal_doc(ctx: RouteCtx) -> Result<Value, ApiError> {
    admin(&ctx)?;
    let (doc, locale) = target(&ctx)?;
    let mut conn = ctx.state.db.acquire().await?;
    ensure_defaults(&mut conn).await?;
    doc_response(&ctx.state, &mut conn, &doc, &locale).await
}

async fn save_doc(
    conn: &mut PgConnection,
    doc: &str,
    locale: &str,
    title: &str,
    content: &str,
    user: Uuid,
) -> Result<(), ApiError> {
    // coincide con il modello della lingua = non personalizzato (segue gli aggiornamenti della piattaforma)
    let customized = template(doc, locale).is_none_or(|(t, c)| t != title || c != content);
    sqlx::query(
        "INSERT INTO legal_documents (doc, locale, title, content, customized, updated_by) VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (doc, locale) DO UPDATE SET title = EXCLUDED.title, content = EXCLUDED.content,
            customized = EXCLUDED.customized, updated_by = EXCLUDED.updated_by, updated_at = now()",
    )
    .bind(doc)
    .bind(locale)
    .bind(title)
    .bind(content)
    .bind(customized)
    .bind(user)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn put_legal_doc(ctx: RouteCtx, body: LegalDocIn) -> Result<Value, ApiError> {
    let user = admin(&ctx)?;
    let (doc, locale) = target(&ctx)?;
    let (title, content) = (body.title.trim(), body.content.replace("\r\n", "\n"));
    if title.is_empty() || content.trim().is_empty() {
        return Err(ApiError::bad_request("Titolo e testo non possono essere vuoti."));
    }
    let mut tx = ctx.state.db.begin().await?;
    ensure_defaults(&mut tx).await?;
    save_doc(&mut tx, &doc, &locale, title, &content, user).await?;
    audit(&mut tx, &ctx, user, "legal.document_saved", json!({"doc": doc, "locale": locale})).await?;
    let out = doc_response(&ctx.state, &mut tx, &doc, &locale).await?;
    tx.commit().await?;
    Ok(out)
}

pub async fn reset_legal_doc(ctx: RouteCtx, body: LegalResetIn) -> Result<Value, ApiError> {
    let user = admin(&ctx)?;
    let (doc, locale) = target(&ctx)?;
    let mut tx = ctx.state.db.begin().await?;
    ensure_defaults(&mut tx).await?;
    let (title, content) = match body.from.as_deref().map(str::to_ascii_lowercase) {
        // ripartire da un'altra lingua: il suo testo attuale, o il suo modello
        Some(from) => match fetch_row(&mut tx, &doc, &from).await? {
            Some(r) => (r.title, r.content),
            None => template(&doc, &from)
                .map(|(t, c)| (t.to_string(), c.to_string()))
                .ok_or_else(|| ApiError::bad_request("Lingua di partenza sconosciuta."))?,
        },
        None => template(&doc, &locale).map(|(t, c)| (t.to_string(), c.to_string())).ok_or_else(|| {
            ApiError::bad_request("Per questa lingua non c'è un modello: indica da quale lingua partire (`from`).")
        })?,
    };
    save_doc(&mut tx, &doc, &locale, &title, &content, user).await?;
    audit(&mut tx, &ctx, user, "legal.document_reset", json!({"doc": doc, "locale": locale, "from": body.from}))
        .await?;
    let out = doc_response(&ctx.state, &mut tx, &doc, &locale).await?;
    tx.commit().await?;
    Ok(out)
}

pub async fn delete_legal_doc(ctx: RouteCtx) -> Result<(), ApiError> {
    let user = admin(&ctx)?;
    let (doc, locale) = target(&ctx)?;
    if TEMPLATE_LOCALES.contains(&locale.as_str()) {
        return Err(ApiError::bad_request(
            "Le lingue del modello non si eliminano: ripristinale o ritirale dalla pubblicazione.",
        ));
    }
    let st = &ctx.state;
    let slug = slug(&doc).expect("documento valido");
    let mut tx = st.db.begin().await?;
    let row = fetch_row(&mut tx, &doc, &locale)
        .await?
        .ok_or_else(|| ApiError::not_found("Documento non trovato in questa lingua."))?;
    if row.published_at.is_some() {
        sqlx::query("DELETE FROM platform_pages WHERE slug = $1 AND locale = $2")
            .bind(slug)
            .bind(&locale)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("DELETE FROM legal_documents WHERE doc = $1 AND locale = $2")
        .bind(&doc)
        .bind(&locale)
        .execute(&mut *tx)
        .await?;
    audit(&mut tx, &ctx, user, "legal.document_deleted", json!({"doc": doc, "locale": locale})).await?;
    tx.commit().await?;
    revalidate_pages(st, &[slug]);
    Ok(())
}

// ---- lettura pubblica -----------------------------------------------------------------------------------------------

/// Per ogni documento pubblicato la lingua richiesta, altrimenti quella predefinita della piattaforma, `it`, `en`
/// o la prima disponibile: il footer deve poter linkare qualcosa in ogni caso.
pub async fn list_public_legal(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let st = &ctx.state;
    let asked = ctx.query.get("locale").map(|l| l.to_ascii_lowercase()).filter(|l| l.len() == 2);
    let default = noct_platform::get(&st.db).await?.default_locale;
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT d.doc, d.locale, d.title FROM legal_documents d
         JOIN platform_pages p ON p.locale = d.locale AND p.slug = CASE d.doc WHEN 'privacy' THEN 'privacy' ELSE 'cookie-policy' END
         WHERE d.published_at IS NOT NULL AND p.is_published ORDER BY d.doc, d.locale",
    )
    .fetch_all(&st.db)
    .await?;
    let mut out = Vec::new();
    for doc in DOCS {
        let mine: Vec<&(String, String, String)> = rows.iter().filter(|r| r.0 == doc).collect();
        let order = [asked.clone(), Some(default.clone()), Some("it".into()), Some("en".into())];
        let pick = order.iter().flatten().find_map(|l| mine.iter().find(|r| &r.1 == l)).or(mine.first());
        if let Some((d, locale, title)) = pick.copied() {
            out.push(json!({"doc": d, "slug": slug(d), "locale": locale, "title": title}));
        }
    }
    Ok(out)
}

// SETTINGS_SELECT è usato da `facts`; qui evita un avviso se il modulo cresce senza leggerlo.
const _: &str = SETTINGS_SELECT;
