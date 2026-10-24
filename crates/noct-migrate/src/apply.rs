use crate::files::checksum;
use noct_manifest::{ColType, Schema};
use sqlx::{PgPool, Row};
use std::collections::BTreeMap;

/// Chiave dell'advisory lock che serializza le migrazioni tra repliche.
const LOCK_KEY: i64 = 0x6e6f_6374_6d69_6772; // "noctmigr"

const LEDGER_DDL: &str = "CREATE TABLE IF NOT EXISTS noct_migrations (
    module text NOT NULL,
    version integer NOT NULL,
    name text NOT NULL,
    checksum text NOT NULL,
    applied_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (module, version)
)";

#[derive(Debug, Clone, Copy)]
pub struct Migration<'a> {
    pub version: u32,
    pub name: &'a str,
    pub sql: &'a str,
}

/// Migrazioni incorporate di un modulo, in ordine di versione.
#[derive(Debug, Clone, Copy)]
pub struct ModuleMigrations<'a> {
    pub module: &'a str,
    pub migrations: &'a [Migration<'a>],
}

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("modulo `{module}`: migrazioni non valide: {reason}")]
    InvalidSet { module: String, reason: String },
    #[error(
        "modulo `{module}` versione {version}: il checksum non coincide con quello registrato (file modificato dopo l'applicazione)"
    )]
    Drift { module: String, version: u32 },
    #[error(
        "modulo `{module}`: il database ha la versione {db_version} ma il binario arriva alla {binary_version} (binario più vecchio del database)"
    )]
    DatabaseAhead { module: String, db_version: u32, binary_version: u32 },
    #[error("modulo `{module}` versione {version} ({name}): {source}")]
    Sql { module: String, version: u32, name: String, source: sqlx::Error },
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMigration {
    pub module: String,
    pub version: u32,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Applied,
    Pending,
    /// Registrata con un checksum diverso da quello del file incorporato.
    Drifted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusRow {
    pub module: String,
    pub version: u32,
    pub name: String,
    pub status: Status,
}

fn check_set(set: &ModuleMigrations<'_>) -> Result<(), MigrationError> {
    for (i, m) in set.migrations.iter().enumerate() {
        if m.version != i as u32 + 1 {
            return Err(MigrationError::InvalidSet {
                module: set.module.to_string(),
                reason: format!(
                    "le versioni devono essere consecutive da 1 (trovata la {} in posizione {})",
                    m.version,
                    i + 1
                ),
            });
        }
    }
    Ok(())
}

async fn applied(pool: &PgPool, module: &str) -> Result<BTreeMap<u32, String>, MigrationError> {
    let rows = sqlx::query("SELECT version, checksum FROM noct_migrations WHERE module = $1")
        .bind(module)
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(|r| (r.get::<i32, _>("version") as u32, r.get::<String, _>("checksum"))).collect())
}

/// Applica le migrazioni mancanti, modulo per modulo nell'ordine ricevuto
/// (il chiamante passa l'ordine topologico), sotto advisory lock. Ogni file è una
/// transazione. Rifiuta di procedere se un file già applicato è stato modificato
/// o se il database è più avanti del binario.
pub async fn apply(pool: &PgPool, sets: &[ModuleMigrations<'_>]) -> Result<Vec<AppliedMigration>, MigrationError> {
    for set in sets {
        check_set(set)?;
    }
    // il lock vive sulla connessione: se cade, Postgres lo rilascia da solo
    let mut lock_conn = pool.acquire().await?;
    sqlx::query("SELECT pg_advisory_lock($1)").bind(LOCK_KEY).execute(&mut *lock_conn).await?;
    let result = apply_locked(pool, sets).await;
    let _ = sqlx::query("SELECT pg_advisory_unlock($1)").bind(LOCK_KEY).execute(&mut *lock_conn).await;
    result
}

async fn apply_locked(pool: &PgPool, sets: &[ModuleMigrations<'_>]) -> Result<Vec<AppliedMigration>, MigrationError> {
    sqlx::query(LEDGER_DDL).execute(pool).await?;
    let mut done = Vec::new();
    for set in sets {
        let known = applied(pool, set.module).await?;
        let binary_version = set.migrations.len() as u32;
        if let Some(&db_version) = known.keys().max()
            && db_version > binary_version
        {
            return Err(MigrationError::DatabaseAhead { module: set.module.into(), db_version, binary_version });
        }
        for m in set.migrations {
            let sum = checksum(m.sql);
            if let Some(recorded) = known.get(&m.version) {
                if *recorded != sum {
                    return Err(MigrationError::Drift { module: set.module.into(), version: m.version });
                }
                continue;
            }
            let sql_err = |source| MigrationError::Sql {
                module: set.module.into(),
                version: m.version,
                name: m.name.into(),
                source,
            };
            let mut tx = pool.begin().await?;
            sqlx::raw_sql(m.sql).execute(&mut *tx).await.map_err(sql_err)?;
            sqlx::query("INSERT INTO noct_migrations (module, version, name, checksum) VALUES ($1, $2, $3, $4)")
                .bind(set.module)
                .bind(m.version as i32)
                .bind(m.name)
                .bind(&sum)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            done.push(AppliedMigration { module: set.module.into(), version: m.version, name: m.name.into() });
        }
    }
    Ok(done)
}

/// Stato di ogni migrazione incorporata (non modifica nulla).
pub async fn status(pool: &PgPool, sets: &[ModuleMigrations<'_>]) -> Result<Vec<StatusRow>, MigrationError> {
    let ledger_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('noct_migrations') IS NOT NULL").fetch_one(pool).await?;
    let mut rows = Vec::new();
    for set in sets {
        let known = if ledger_exists { applied(pool, set.module).await? } else { BTreeMap::new() };
        for m in set.migrations {
            let status = match known.get(&m.version) {
                None => Status::Pending,
                Some(sum) if *sum == checksum(m.sql) => Status::Applied,
                Some(_) => Status::Drifted,
            };
            rows.push(StatusRow { module: set.module.into(), version: m.version, name: m.name.into(), status });
        }
    }
    Ok(rows)
}

/// Confronta lo schema desiderato con il database reale (tabelle, colonne,
/// nullabilità, enum). Restituisce le differenze trovate; vuoto = allineato.
pub async fn verify_live(pool: &PgPool, schema: &Schema) -> Result<Vec<String>, MigrationError> {
    let mut problems = Vec::new();
    for e in &schema.enums {
        let rows = sqlx::query(
            "SELECT e.enumlabel FROM pg_type t JOIN pg_enum e ON e.enumtypid = t.oid WHERE t.typname = $1 ORDER BY e.enumsortorder",
        )
        .bind(&e.name)
        .fetch_all(pool)
        .await?;
        let live: Vec<String> = rows.iter().map(|r| r.get::<String, _>(0)).collect();
        if live.is_empty() {
            problems.push(format!("enum `{}` assente nel database", e.name));
        } else if let Some(missing) = e.values.iter().find(|v| !live.contains(v)) {
            problems.push(format!("enum `{}`: valore `{missing}` assente nel database", e.name));
        }
    }
    for t in &schema.tables {
        let rows = sqlx::query(
            "SELECT column_name, is_nullable, data_type, udt_name FROM information_schema.columns
             WHERE table_schema = current_schema() AND table_name = $1",
        )
        .bind(&t.name)
        .fetch_all(pool)
        .await?;
        if rows.is_empty() {
            problems.push(format!("tabella `{}` assente nel database", t.name));
            continue;
        }
        let live: BTreeMap<String, (bool, String, String)> = rows
            .iter()
            .map(|r| {
                (
                    r.get("column_name"),
                    (r.get::<String, _>("is_nullable") == "YES", r.get("data_type"), r.get("udt_name")),
                )
            })
            .collect();
        for c in &t.columns {
            match live.get(&c.name) {
                None => problems.push(format!("colonna `{}.{}` assente nel database", t.name, c.name)),
                Some((nullable, data_type, udt)) => {
                    if *nullable != c.null {
                        problems.push(format!("colonna `{}.{}`: nullabilità diversa dal manifest", t.name, c.name));
                    }
                    let type_ok = match &c.ty {
                        ColType::Text => data_type == "text",
                        ColType::Varchar(_) => data_type == "character varying",
                        ColType::Uuid => data_type == "uuid",
                        ColType::Integer => data_type == "integer",
                        ColType::BigInt => data_type == "bigint",
                        ColType::Boolean => data_type == "boolean",
                        ColType::Timestamptz => data_type == "timestamp with time zone",
                        ColType::Jsonb => data_type == "jsonb",
                        ColType::Enum(name) => data_type == "USER-DEFINED" && udt == name,
                    };
                    if !type_ok {
                        problems.push(format!(
                            "colonna `{}.{}`: tipo reale `{data_type}` diverso da {}",
                            t.name, c.name, c.ty
                        ));
                    }
                }
            }
        }
    }
    Ok(problems)
}
