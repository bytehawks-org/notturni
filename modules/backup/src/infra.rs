//! Backup infrastrutturale: `pg_dump` in formato custom più copia di tutti i bucket applicativi verso
//! lo storage esterno configurato (`NOCT_BACKUP_S3_*`).

use chrono::Utc;
use noct_core::AppState;
use noct_storage::{S3Config, S3Storage, Storage};
use tokio::process::Command;

#[derive(Debug)]
pub struct Report {
    pub dump_key: String,
    pub mirrored_objects: usize,
}

struct Destination {
    bucket: String,
    config: S3Config,
}

fn destination(st: &AppState) -> Option<Destination> {
    let get = |k: &str| st.module_config_str("backup", k).map(str::to_string);
    Some(Destination {
        bucket: get("s3_bucket")?,
        config: S3Config {
            endpoint_url: get("s3_endpoint_url"),
            region: get("s3_region"),
            access_key_id: get("s3_access_key_id"),
            secret_access_key: get("s3_secret_access_key"),
            public_url: None,
        },
    })
}

/// Parametri di connessione di `pg_dump` dall'URL del database; la password passa da `PGPASSWORD`,
/// non dalla riga di comando.
struct PgTarget {
    host: String,
    port: u16,
    user: String,
    password: String,
    database: String,
}

fn pg_target(database_url: &str) -> Result<PgTarget, String> {
    let url = url::Url::parse(database_url).map_err(|e| format!("URL del database non valido: {e}"))?;
    let decode = |s: &str| percent_encoding::percent_decode_str(s).decode_utf8_lossy().into_owned();
    Ok(PgTarget {
        host: url.host_str().unwrap_or("localhost").to_string(),
        port: url.port().unwrap_or(5432),
        user: decode(url.username()),
        password: decode(url.password().unwrap_or_default()),
        database: url.path().trim_start_matches('/').to_string(),
    })
}

async fn dump_postgres(st: &AppState) -> Result<(String, Vec<u8>), String> {
    let t = pg_target(&st.config.database_url)?;
    let out = Command::new("pg_dump")
        .args([
            "--format",
            "custom",
            "--host",
            &t.host,
            "--port",
            &t.port.to_string(),
            "--username",
            &t.user,
            &t.database,
        ])
        .env("PGPASSWORD", &t.password)
        .output()
        .await
        .map_err(|e| format!("pg_dump non eseguibile: {e}"))?;
    if !out.status.success() {
        return Err(format!("pg_dump fallito: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok((t.database, out.stdout))
}

async fn mirror_bucket(st: &AppState, dest: &S3Storage, dest_bucket: &str, bucket: &str) -> Result<usize, String> {
    let keys = st.services.storage.list(bucket, "").await.map_err(|e| e.to_string())?;
    let mut count = 0;
    for key in keys {
        let Some(object) = st.services.storage.get(bucket, &key).await.map_err(|e| e.to_string())? else { continue };
        let content_type = object.content_type.as_deref().unwrap_or("application/octet-stream");
        dest.put(dest_bucket, &format!("s3/{bucket}/{key}"), object.data, content_type)
            .await
            .map_err(|e| e.to_string())?;
        count += 1;
    }
    Ok(count)
}

/// Un giro di backup. `None` se la destinazione non è configurata.
pub async fn run_once(st: &AppState) -> Result<Option<Report>, String> {
    let Some(d) = destination(st) else {
        tracing::info!("NOCT_BACKUP_S3_BUCKET non configurato: backup infrastrutturale disattivato");
        return Ok(None);
    };
    let dest = S3Storage::new(&d.config).await;
    // alcuni provider non concedono CreateBucket a credenziali con privilegi minimi: se il bucket
    // esiste davvero l'errore salterà fuori più avanti, con un messaggio più chiaro
    if let Err(e) = dest.ensure_bucket(&d.bucket).await {
        tracing::warn!(error = %e, bucket = %d.bucket, "bucket di backup non verificato: proseguo");
    }
    let (database, dump) = dump_postgres(st).await?;
    let dump_key = format!("postgres/{database}-{}.dump", Utc::now().format("%Y%m%dT%H%M%SZ"));
    let size = dump.len();
    dest.put(&d.bucket, &dump_key, dump.into(), "application/octet-stream").await.map_err(|e| e.to_string())?;
    tracing::info!(key = %dump_key, bytes = size, "dump di Postgres scritto");
    let storage = &st.config.storage;
    let mut mirrored = 0;
    for bucket in [&storage.bucket_content, &storage.bucket_avatars, &storage.bucket_audit] {
        mirrored += mirror_bucket(st, &dest, &d.bucket, bucket).await?;
    }
    Ok(Some(Report { dump_key, mirrored_objects: mirrored }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_url_is_split_without_leaking_the_password_in_arguments() {
        let t = pg_target("postgres://noct:p%40ss@db.example.com:6543/notturni").unwrap();
        assert_eq!(
            (t.host.as_str(), t.port, t.user.as_str(), t.database.as_str()),
            ("db.example.com", 6543, "noct", "notturni")
        );
        assert_eq!(t.password, "p@ss");
        let t = pg_target("postgres://postgres@localhost/db").unwrap();
        assert_eq!((t.port, t.password.as_str()), (5432, ""));
        assert!(pg_target("non un url").is_err());
    }
}
