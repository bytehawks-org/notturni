//! Manutenzione del registro: archivio su storage delle settimane ISO concluse e cancellazione di
//! ciò che supera la conservazione. Idempotente: si può rieseguire in qualsiasi momento.

use bytes::Bytes;
use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use noct_core::AppState;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use std::io::{Read, Write};
use uuid::Uuid;

const PRUNE_BATCH: i64 = 5000;
/// Tappo di sicurezza: dieci anni di settimane per giro.
const MAX_WEEKS_PER_RUN: usize = 520;
/// Meno di una settimana di conservazione non è una politica sensata: la cancellazione si salta.
const MIN_RETENTION_DAYS: i32 = 7;

#[derive(Debug, thiserror::Error)]
pub enum MaintenanceError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("storage: {0}")]
    Storage(String),
    #[error("archivio non valido: {0}")]
    Archive(String),
    #[error("{0}")]
    Platform(String),
}

#[derive(Debug, Default)]
pub struct Report {
    pub archived_weeks: Vec<String>,
    pub deleted_rows: u64,
}

/// (inizio, fine, etichetta) della settimana ISO che contiene `dt`: il lunedì a mezzanotte UTC,
/// il lunedì dopo (intervallo semiaperto) e `AAAAwSS`.
pub fn iso_week_bounds(dt: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>, String) {
    let monday = dt.date_naive() - Duration::days(i64::from(dt.weekday().num_days_from_monday()));
    let start = Utc.from_utc_datetime(&monday.and_hms_opt(0, 0, 0).expect("mezzanotte valida"));
    let week = monday.iso_week();
    (start, start + Duration::days(7), format!("{}w{:02}", week.year(), week.week()))
}

fn archive_key(st: &AppState, label: &str) -> String {
    format!("{}/audit-archive/{label}.jsonl.gz", st.config.site_slug)
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

fn to_json(r: &Row) -> Value {
    json!({
        "id": r.id, "occurred_at": r.occurred_at.to_rfc3339(), "actor_type": r.actor_type, "actor_id": r.actor_id,
        "actor_label": r.actor_label, "action": r.action, "target_type": r.target_type, "target_id": r.target_id,
        "blog_id": r.blog_id, "ip": r.ip, "user_agent": r.user_agent, "payload": r.payload,
    })
}

async fn archive_week(
    st: &AppState,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    label: &str,
) -> Result<usize, MaintenanceError> {
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, occurred_at, actor_type::text AS actor_type, actor_id, actor_label, action, target_type, target_id,
                blog_id, ip, user_agent, payload
         FROM audit_log WHERE occurred_at >= $1 AND occurred_at < $2 ORDER BY occurred_at, id",
    )
    .bind(start)
    .bind(end)
    .fetch_all(&st.db)
    .await?;
    let (mut key, mut digest, mut size) = (None, None, 0_i64);
    if !rows.is_empty() {
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        for r in &rows {
            let mut line = serde_json::to_vec(&to_json(r)).map_err(|e| MaintenanceError::Archive(e.to_string()))?;
            line.push(b'\n');
            enc.write_all(&line).map_err(|e| MaintenanceError::Archive(e.to_string()))?;
        }
        let blob = enc.finish().map_err(|e| MaintenanceError::Archive(e.to_string()))?;
        digest = Some(hex::encode(Sha256::digest(&blob)));
        size = blob.len() as i64;
        let (bucket, k) = (&st.config.storage.bucket_audit, archive_key(st, label));
        let storage = &st.services.storage;
        storage.ensure_bucket(bucket).await.map_err(|e| MaintenanceError::Storage(e.to_string()))?;
        storage
            .put(bucket, &k, Bytes::from(blob), "application/gzip")
            .await
            .map_err(|e| MaintenanceError::Storage(e.to_string()))?;
        key = Some(k);
    }
    sqlx::query(
        "INSERT INTO audit_archive_runs (period_start, period_end, week_label, object_key, storage_backend, row_count, byte_size, sha256)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT ON CONSTRAINT uq_audit_archive_week DO NOTHING",
    )
    .bind(start)
    .bind(end)
    .bind(label)
    .bind(key)
    .bind(st.services.storage.backend())
    .bind(rows.len() as i32)
    .bind(size)
    .bind(digest)
    .execute(&st.db)
    .await?;
    Ok(rows.len())
}

/// Archivia le settimane concluse non ancora archiviate, dal watermark in poi.
pub async fn archive(st: &AppState, now: DateTime<Utc>) -> Result<Vec<String>, MaintenanceError> {
    let watermark: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT max(period_end) FROM audit_archive_runs").fetch_one(&st.db).await?;
    let mut week_start = match watermark {
        Some(w) => w,
        None => {
            let earliest: Option<DateTime<Utc>> =
                sqlx::query_scalar("SELECT min(occurred_at) FROM audit_log").fetch_one(&st.db).await?;
            match earliest {
                Some(e) => iso_week_bounds(e).0,
                None => return Ok(Vec::new()),
            }
        }
    };
    let current = iso_week_bounds(now).0;
    let mut done = Vec::new();
    while week_start < current && done.len() < MAX_WEEKS_PER_RUN {
        let (start, end, label) = iso_week_bounds(week_start);
        let n = archive_week(st, start, end, &label).await?;
        tracing::info!(week = %label, rows = n, "settimana di audit archiviata");
        done.push(label);
        week_start = end;
    }
    Ok(done)
}

/// Cancella gli eventi oltre la conservazione, a lotti, mai più recenti dell'ultima settimana archiviata.
pub async fn prune(st: &AppState, now: DateTime<Utc>, archive_enabled: bool) -> Result<u64, MaintenanceError> {
    let days = noct_platform::get(&st.db).await.map_err(|e| MaintenanceError::Platform(e.detail))?.audit_retention_days;
    if days < MIN_RETENTION_DAYS {
        tracing::warn!(days, "audit_retention_days sotto una settimana: cancellazione saltata");
        return Ok(0);
    }
    let mut floor = now - Duration::days(i64::from(days));
    if archive_enabled {
        let watermark: Option<DateTime<Utc>> =
            sqlx::query_scalar("SELECT max(period_end) FROM audit_archive_runs").fetch_one(&st.db).await?;
        let Some(w) = watermark else {
            tracing::warn!("archiviazione attiva ma nessuna settimana archiviata: cancellazione saltata");
            return Ok(0);
        };
        floor = floor.min(w);
    }
    let mut total = 0;
    loop {
        let n = sqlx::query(
            "DELETE FROM audit_log WHERE id IN (SELECT id FROM audit_log WHERE occurred_at < $1 ORDER BY occurred_at LIMIT $2)",
        )
        .bind(floor)
        .bind(PRUNE_BATCH)
        .execute(&st.db)
        .await?
        .rows_affected();
        total += n;
        if (n as i64) < PRUNE_BATCH {
            return Ok(total);
        }
    }
}

pub async fn run_once(st: &AppState, archive_enabled: bool) -> Result<Report, MaintenanceError> {
    let now = Utc::now();
    let archived_weeks = if archive_enabled { archive(st, now).await? } else { Vec::new() };
    let deleted_rows = prune(st, now, archive_enabled).await?;
    Ok(Report { archived_weeks, deleted_rows })
}

/// Rimette a database gli eventi di una settimana archiviata (`2026w36`). Gli id già presenti non si
/// toccano: ritorna quanti ne ha reinseriti.
pub async fn restore_week(st: &AppState, label: &str) -> Result<usize, MaintenanceError> {
    if label.len() > 10 || !label.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(MaintenanceError::Archive(format!("etichetta non valida: `{label}`")));
    }
    let object = st
        .services
        .storage
        .get(&st.config.storage.bucket_audit, &archive_key(st, label))
        .await
        .map_err(|e| MaintenanceError::Storage(e.to_string()))?
        .ok_or_else(|| MaintenanceError::Archive(format!("nessun archivio per `{label}`")))?;
    let mut text = String::new();
    GzDecoder::new(&object.data[..]).read_to_string(&mut text).map_err(|e| MaintenanceError::Archive(e.to_string()))?;
    let mut tx = st.db.begin().await?;
    let mut inserted = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(line).map_err(|e| MaintenanceError::Archive(e.to_string()))?;
        let text_of = |k: &str| v[k].as_str().map(str::to_string);
        let uuid_of = |k: &str| v[k].as_str().and_then(|s| Uuid::parse_str(s).ok());
        let at = text_of("occurred_at")
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|d| d.with_timezone(&Utc))
            .ok_or_else(|| MaintenanceError::Archive("riga senza `occurred_at`".into()))?;
        inserted += sqlx::query(
            "INSERT INTO audit_log (id, occurred_at, actor_type, actor_id, actor_label, action, target_type, target_id, blog_id, ip, user_agent, payload)
             VALUES ($1, $2, $3::audit_actor_type, $4, $5, $6, $7, $8, $9, $10, $11, $12) ON CONFLICT (id) DO NOTHING",
        )
        .bind(uuid_of("id").ok_or_else(|| MaintenanceError::Archive("riga senza `id`".into()))?)
        .bind(at)
        .bind(text_of("actor_type").unwrap_or_default())
        .bind(uuid_of("actor_id"))
        .bind(text_of("actor_label"))
        .bind(text_of("action").unwrap_or_default())
        .bind(text_of("target_type"))
        .bind(uuid_of("target_id"))
        .bind(uuid_of("blog_id"))
        .bind(text_of("ip"))
        .bind(text_of("user_agent"))
        .bind(v["payload"].clone())
        .execute(&mut *tx)
        .await?
        .rows_affected() as usize;
    }
    tx.commit().await?;
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_weeks_start_on_monday_and_follow_the_iso_year() {
        let at = |y, m, d, h| Utc.with_ymd_and_hms(y, m, d, h, 30, 0).unwrap();
        // mercoledì 2026-09-02 → settimana 36 da lunedì 2026-08-31
        let (s, e, l) = iso_week_bounds(at(2026, 9, 2, 14));
        assert_eq!(
            (s, e, l.as_str()),
            (at(2026, 8, 31, 0) - Duration::minutes(30), at(2026, 9, 7, 0) - Duration::minutes(30), "2026w36")
        );
        // domenica sera e lunedì mattina stanno in settimane diverse
        assert_eq!(iso_week_bounds(at(2026, 9, 6, 23)).2, "2026w36");
        assert_eq!(iso_week_bounds(at(2026, 9, 7, 0)).2, "2026w37");
        // l'anno ISO non coincide con quello civile: 1 gennaio 2027 è venerdì della settimana 53 del 2026
        assert_eq!(iso_week_bounds(at(2027, 1, 1, 12)).2, "2026w53");
    }
}
