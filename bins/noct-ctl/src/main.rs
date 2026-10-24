//! Strumento di amministrazione e sviluppo: validazione dei manifest, ciclo di
//! vita delle migrazioni, OpenAPI.

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use noct_core::{Config, ModuleDef, Registry};
use noct_manifest::{Manifest, Schema, Sources, topological_order};
use noct_migrate::{Migration, ModuleMigrations, Status, diff, parse_file_name, render_file};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{fs, path::Path};

#[derive(Parser)]
#[command(name = "noct-ctl", about = "Strumenti di sviluppo e amministrazione di Notturni")]
struct Cli {
    /// Moduli attivi (separati da virgola); default: NOCT_MODULES o i default dei manifest.
    #[arg(long, global = true, env = "NOCT_MODULES")]
    modules: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Valida tutti i manifest e le relazioni tra moduli (drift incluso). Per la CI.
    Validate,
    /// Elenca i moduli compilati e il loro stato.
    Modules,
    /// Stampa il documento OpenAPI dei moduli attivi.
    Openapi,
    /// Gestione delle migrazioni di schema.
    #[command(subcommand)]
    Migrate(Migrate),
    /// Stato dell'outbox transazionale (eventi e job in attesa di pubblicazione).
    Outbox,
    /// Messaggi finiti in dead letter sul bus.
    DeadLetters,
    /// API token del motore core (machine-to-machine).
    #[command(subcommand)]
    Token(TokenCmd),
    /// Manutenzione del registro di audit.
    #[command(subcommand)]
    Audit(AuditCmd),
    /// Porta Garage allo stato atteso (layout del nodo, chiave applicativa, bucket); idempotente.
    GarageBootstrap,
}

#[derive(Subcommand)]
enum AuditCmd {
    /// Un giro di manutenzione adesso: archivia le settimane concluse e cancella oltre la conservazione.
    Maintain,
    /// Rimette a database gli eventi di una settimana archiviata (`2026w36`).
    Restore { week: String },
}

#[derive(Subcommand)]
enum TokenCmd {
    /// Emette un token core e lo stampa **una sola volta** (in DB resta solo l'hash).
    Create {
        #[arg(long)]
        name: String,
    },
}

#[derive(Subcommand)]
enum Migrate {
    /// Mostra cosa genererebbe `generate`, senza scrivere nulla.
    Plan {
        #[arg(long)]
        module: Option<String>,
    },
    /// Scrive la prossima migrazione SQL e aggiorna schema.lock.json.
    Generate {
        #[arg(long)]
        module: Option<String>,
        /// Suffisso del file (default: `init` alla prima migrazione, poi `schema_update`).
        #[arg(long)]
        name: Option<String>,
    },
    /// Aggiorna solo schema.lock.json (dopo aver scritto a mano una migrazione).
    Lock {
        #[arg(long)]
        module: Option<String>,
    },
    /// Applica al database le migrazioni incorporate nel binario mancanti.
    Apply,
    /// Stato di ogni migrazione (applicata / da applicare / modificata).
    Status,
    /// Confronta i manifest con il database reale.
    Verify,
    /// Sviluppo: `generate` per tutti i moduli, poi applica i file letti da disco
    /// (nessuna ricompilazione necessaria per provare la migrazione).
    Dev,
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok()
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.cmd {
        // i comandi che riparano il drift non possono dipendere da un registry valido
        Cmd::Migrate(Migrate::Plan { module }) => plan(module.as_deref()),
        Cmd::Migrate(Migrate::Generate { module, name }) => generate(module.as_deref(), name.as_deref()),
        Cmd::Migrate(Migrate::Lock { module }) => lock(module.as_deref()),
        Cmd::Migrate(Migrate::Dev) => {
            generate(None, None)?;
            apply_from_disk().await
        }
        _ => {
            let registry = Registry::new(noct_bundle::all()).map_err(|e| anyhow!("manifest non validi:\n{e}"))?;
            let requested: Option<Vec<String>> = cli
                .modules
                .as_deref()
                .map(|m| m.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect());
            let enabled =
                registry.resolve_enabled(requested.as_deref()).map_err(|e| anyhow!("moduli non attivabili:\n{e}"))?;
            match cli.cmd {
                Cmd::Token(TokenCmd::Create { name }) => create_core_token(&name).await,
                Cmd::Audit(cmd) => audit(cmd).await,
                Cmd::GarageBootstrap => garage_bootstrap().await,
                Cmd::Outbox => outbox().await,
                Cmd::DeadLetters => dead_letters().await,
                Cmd::Validate => validate(&registry, &enabled),
                Cmd::Modules => modules(&registry, &enabled),
                Cmd::Openapi => {
                    println!("{}", serde_json::to_string_pretty(&registry.openapi(&enabled))?);
                    Ok(())
                }
                Cmd::Migrate(Migrate::Apply) => {
                    let pool = connect().await?;
                    let done = registry.migrate(&pool, &enabled).await?;
                    if done.is_empty() {
                        println!("nessuna migrazione da applicare");
                    }
                    for m in done {
                        println!("applicata: {} {:04}_{}", m.module, m.version, m.name);
                    }
                    Ok(())
                }
                Cmd::Migrate(Migrate::Status) => {
                    let pool = connect().await?;
                    for row in noct_migrate::status(&pool, &registry.migration_sets(&enabled)).await? {
                        let st = match row.status {
                            Status::Applied => "applicata",
                            Status::Pending => "DA APPLICARE",
                            Status::Drifted => "MODIFICATA DOPO L'APPLICAZIONE",
                        };
                        println!("{:<12} {:04}_{:<24} {st}", row.module, row.version, row.name);
                    }
                    Ok(())
                }
                Cmd::Migrate(Migrate::Verify) => {
                    let pool = connect().await?;
                    let mut bad = 0;
                    for (i, m) in registry.manifests().into_iter().enumerate() {
                        if !enabled.contains(&i) {
                            continue;
                        }
                        for p in noct_migrate::verify_live(&pool, &m.schema).await? {
                            println!("{}: {p}", m.name());
                            bad += 1;
                        }
                    }
                    if bad > 0 {
                        bail!("{bad} differenze tra manifest e database");
                    }
                    println!("database allineato ai manifest");
                    Ok(())
                }
                _ => unreachable!(),
            }
        }
    }
}

async fn connect() -> Result<PgPool> {
    let url = Config::database_url_from_env(&env).map_err(anyhow::Error::msg)?;
    PgPoolOptions::new().max_connections(2).connect(&url).await.context("connessione a Postgres")
}

fn validate(registry: &Registry, enabled: &[usize]) -> Result<()> {
    let routes: usize = registry.manifests().iter().map(|m| m.api.routes.len()).sum();
    let tables: usize = registry.manifests().iter().map(|m| m.schema.tables.len()).sum();
    println!(
        "OK: {} moduli ({} attivi), {routes} rotte, {tables} tabelle; nessun drift di schema",
        registry.manifests().len(),
        enabled.len()
    );
    Ok(())
}

fn modules(registry: &Registry, enabled: &[usize]) -> Result<()> {
    println!("{:<10} {:<9} {:<8} {:<7} {:<7} {:<7} requires", "modulo", "kind", "attivo", "rotte", "tabelle", "migr.");
    for (i, (m, def)) in registry.manifests().into_iter().zip(registry.defs()).enumerate() {
        println!(
            "{:<10} {:<9} {:<8} {:<7} {:<7} {:<7} {}",
            m.name(),
            format!("{:?}", m.kind()).to_lowercase(),
            if enabled.contains(&i) { "sì" } else { "no" },
            m.api.routes.len(),
            m.schema.tables.len(),
            def.migrations.len(),
            m.module.module.requires.join(", ")
        );
    }
    Ok(())
}

// ---- migrazioni, lette da disco ------------------------------------------------------

fn read_opt(dir: &Path, file: &str) -> Result<Option<String>> {
    match fs::read_to_string(dir.join(file)) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("lettura di {file}")),
    }
}

/// Manifest di un modulo letti dai sorgenti (non dalla copia incorporata nel binario).
fn disk_manifest(def: &ModuleDef) -> Result<Manifest> {
    let dir = Path::new(def.source_dir);
    let module = read_opt(dir, "module.toml")?.context("manca module.toml")?;
    let api = read_opt(dir, "api.toml")?;
    let db = read_opt(dir, "db.toml")?;
    let lock = read_opt(dir, "schema.lock.json")?;
    Manifest::parse(Sources { module: &module, api: api.as_deref(), db: db.as_deref(), lock: lock.as_deref() })
        .map_err(|e| anyhow!("{}:\n{e}", def.name))
}

fn selected(module: Option<&str>) -> Result<Vec<ModuleDef>> {
    let all = noct_bundle::all();
    match module {
        None => Ok(all),
        Some(name) => {
            let found: Vec<_> = all.into_iter().filter(|d| d.name == name).collect();
            if found.is_empty() {
                bail!("modulo sconosciuto: {name}");
            }
            Ok(found)
        }
    }
}

fn next_version(dir: &Path) -> u32 {
    fs::read_dir(dir.join("migrations"))
        .map(|rd| rd.flatten().filter(|e| parse_file_name(&e.file_name().to_string_lossy()).is_some()).count() as u32)
        .unwrap_or(0)
        + 1
}

fn write_lock(dir: &Path, schema: &Schema) -> Result<()> {
    let json = serde_json::to_string_pretty(&schema.canonical())? + "\n";
    fs::write(dir.join("schema.lock.json"), json).context("scrittura di schema.lock.json")
}

fn plan(module: Option<&str>) -> Result<()> {
    for def in selected(module)? {
        let m = disk_manifest(&def)?;
        let p = diff(&m.lock.clone().unwrap_or_default(), &m.schema);
        println!("== {} ==", def.name);
        if p.is_empty() {
            println!("  nessuna modifica di schema");
        }
        for s in &p.steps {
            println!("  + {}", s.description);
        }
        for w in &p.warnings {
            println!("  ! {w}");
        }
        for b in &p.blocked {
            println!("  ✗ BLOCCATO: {b}");
        }
    }
    Ok(())
}

fn generate(module: Option<&str>, name: Option<&str>) -> Result<()> {
    let mut failed = false;
    for def in selected(module)? {
        let dir = Path::new(def.source_dir);
        let m = disk_manifest(&def)?;
        let old = m.lock.clone().unwrap_or_default();
        let p = diff(&old, &m.schema);
        if !p.blocked.is_empty() {
            failed = true;
            eprintln!("{}: modifiche che richiedono una migrazione scritta a mano:", def.name);
            for b in &p.blocked {
                eprintln!("  ✗ {b}");
            }
            eprintln!("  → scrivere migrations/NNNN_nome.sql, poi `noct-ctl migrate lock --module {}`", def.name);
            continue;
        }
        if p.steps.is_empty() {
            if m.lock.is_none() || m.drift().is_some() {
                write_lock(dir, &m.schema)?;
                println!("{}: nessuna modifica SQL, schema.lock.json aggiornato", def.name);
            } else {
                println!("{}: già allineato", def.name);
            }
            continue;
        }
        let version = next_version(dir);
        let suffix = name.unwrap_or(if version == 1 { "init" } else { "schema_update" });
        let file = format!("{version:04}_{suffix}.sql");
        if parse_file_name(&file).is_none() {
            bail!("nome migrazione non valido: {file} (minuscole, cifre, `_`)");
        }
        fs::create_dir_all(dir.join("migrations"))?;
        fs::write(dir.join("migrations").join(&file), render_file(def.name, version, &p))?;
        write_lock(dir, &m.schema)?;
        println!("{}: scritta migrations/{file}", def.name);
        for w in &p.warnings {
            println!("  ! {w}");
        }
    }
    if failed {
        bail!("generazione interrotta");
    }
    Ok(())
}

fn lock(module: Option<&str>) -> Result<()> {
    for def in selected(module)? {
        let m = disk_manifest(&def)?;
        write_lock(Path::new(def.source_dir), &m.schema)?;
        println!("{}: schema.lock.json aggiornato", def.name);
    }
    Ok(())
}

async fn apply_from_disk() -> Result<()> {
    let defs = noct_bundle::all();
    let manifests: Vec<Manifest> = defs.iter().map(disk_manifest).collect::<Result<_>>()?;
    let refs: Vec<&Manifest> = manifests.iter().collect();
    let order = topological_order(&refs).map_err(|c| anyhow!("ciclo di dipendenze: {}", c.join(", ")))?;

    // contenuto dei file, tenuto vivo per tutta l'applicazione
    type ModuleFiles = (String, Vec<(u32, String, String)>);
    let mut files: Vec<ModuleFiles> = Vec::new();
    for &i in &order {
        let dir = Path::new(defs[i].source_dir).join("migrations");
        let mut list = Vec::new();
        if let Ok(rd) = fs::read_dir(&dir) {
            for e in rd.flatten() {
                let fname = e.file_name().to_string_lossy().to_string();
                if let Some((v, n)) = parse_file_name(&fname) {
                    list.push((v, n, fs::read_to_string(e.path())?));
                }
            }
        }
        list.sort();
        files.push((defs[i].name.to_string(), list));
    }
    let migrations: Vec<Vec<Migration<'_>>> = files
        .iter()
        .map(|(_, l)| l.iter().map(|(v, n, sql)| Migration { version: *v, name: n, sql }).collect())
        .collect();
    let sets: Vec<ModuleMigrations<'_>> =
        files.iter().zip(&migrations).map(|((name, _), m)| ModuleMigrations { module: name, migrations: m }).collect();

    let pool = connect().await?;
    let done = noct_migrate::apply(&pool, &sets).await?;
    if done.is_empty() {
        println!("nessuna migrazione da applicare");
    }
    for m in done {
        println!("applicata: {} {:04}_{}", m.module, m.version, m.name);
    }
    Ok(())
}

async fn outbox() -> Result<()> {
    let pool = connect().await?;
    let rows = sqlx::query_as::<_, (String, String, i64, i64)>(
        "SELECT kind::text, subject, count(*) FILTER (WHERE published_at IS NULL), count(*) FILTER (WHERE published_at IS NOT NULL)
         FROM noct_outbox GROUP BY 1, 2 ORDER BY 1, 2",
    )
    .fetch_all(&pool)
    .await
    .context("lettura di noct_outbox (migrazioni applicate?)")?;
    println!("{:<6} {:<40} {:>10} {:>10}", "tipo", "subject", "in attesa", "pubblicati");
    for (kind, subject, pending, published) in rows {
        println!("{kind:<6} {subject:<40} {pending:>10} {published:>10}");
    }
    let stuck: Option<(String, String)> = sqlx::query_as(
        "SELECT subject, last_error FROM noct_outbox WHERE published_at IS NULL AND last_error IS NOT NULL ORDER BY created_at LIMIT 1",
    )
    .fetch_optional(&pool)
    .await?;
    if let Some((subject, error)) = stuck {
        println!("\nbloccato su `{subject}`: {error}");
    }
    Ok(())
}

async fn dead_letters() -> Result<()> {
    let config = Config::from_env(&env).map_err(anyhow::Error::msg)?;
    let services = noct_core::Services::connect(&config).await.map_err(anyhow::Error::msg)?;
    let dead = services.bus.dead_letters().await?;
    if dead.is_empty() {
        println!("nessuna dead letter");
    }
    for d in dead {
        println!(
            "{} {} id={} consumer={} consegne={}\n  motivo: {}\n  payload: {}",
            d.kind.as_str(),
            d.subject,
            d.id,
            d.consumer,
            d.delivered,
            d.reason,
            d.payload
        );
    }
    Ok(())
}

async fn create_core_token(name: &str) -> Result<()> {
    if name.trim().is_empty() || name.len() > 255 {
        bail!("il nome deve essere non vuoto (max 255 caratteri)");
    }
    let pool = connect().await?;
    let (plaintext, prefix, hash) = noct_core::security::token::generate_api_token();
    let mut tx = pool.begin().await?;
    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO api_tokens (name, owner_type, token_prefix, token_hash) VALUES ($1, 'core', $2, $3) RETURNING id",
    )
    .bind(name)
    .bind(&prefix)
    .bind(&hash)
    .fetch_one(&mut *tx)
    .await
    .context("scrittura di api_tokens (migrazioni applicate?)")?;
    noct_audit::record(
        &mut tx,
        noct_audit::Entry {
            action: "api_token.created",
            actor: noct_audit::AuditActor::System,
            target: Some(("api_token", id)),
            blog_id: None,
            meta: &noct_core::RequestMeta::default(),
            payload: serde_json::json!({ "name": name, "owner_type": "core", "via": "noct-ctl" }),
        },
    )
    .await?;
    tx.commit().await?;
    println!("{plaintext}");
    eprintln!("Token core `{name}` creato (id {id}). Conservalo ora: non sarà più mostrato.");
    Ok(())
}

// ---- audit ---------------------------------------------------------------------------------------------------------

/// Stato minimo per i comandi che usano solo database, storage e configurazione.
async fn light_state() -> Result<noct_core::AppState> {
    let config = Config::from_env(&|k| env(k)).map_err(|e| anyhow!(e))?;
    let pool = connect().await?;
    let services = noct_core::Services::connect(&config).await.map_err(|e| anyhow!(e))?;
    Ok(noct_core::AppState::new(pool, config, services, Default::default()))
}

async fn audit(cmd: AuditCmd) -> Result<()> {
    let st = light_state().await?;
    match cmd {
        AuditCmd::Maintain => {
            let archive = env("NOCT_AUDIT_ARCHIVE_ENABLED").is_none_or(|v| !matches!(v.as_str(), "0" | "false" | "no"));
            let report = noct_audit::maintenance::run_once(&st, archive).await?;
            println!("settimane archiviate: {}", report.archived_weeks.len());
            println!("eventi cancellati: {}", report.deleted_rows);
        }
        AuditCmd::Restore { week } => {
            let n = noct_audit::maintenance::restore_week(&st, &week).await?;
            println!("{week}: {n} eventi reinseriti");
        }
    }
    Ok(())
}

// ---- Garage ----------------------------------------------------------------------------------------------------------

const GARAGE_ZONE: &str = "dc1";
const GARAGE_READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

fn required(name: &str) -> Result<String> {
    env(name).filter(|v| !v.is_empty()).ok_or_else(|| anyhow!("{name} obbligatoria"))
}

/// Un Garage appena avviato non è utilizzabile: nessun ruolo nel layout, nessuna chiave, e i bucket
/// creati via S3 avrebbero solo un alias *locale* alla chiave, che il web endpoint non risolve.
/// Questo comando porta Garage allo stato atteso ed è sicuro da rieseguire (anche in parallelo:
/// initContainer di più repliche).
async fn garage_bootstrap() -> Result<()> {
    use serde_json::{Value, json};
    let base = env("NOCT_GARAGE_ADMIN_URL").unwrap_or_else(|| "http://garage:3903".into());
    let token = required("NOCT_GARAGE_ADMIN_TOKEN")?;
    let key_id = required("NOCT_S3_ACCESS_KEY_ID")?;
    let secret = required("NOCT_S3_SECRET_ACCESS_KEY")?;
    let capacity: u64 = env("NOCT_GARAGE_NODE_CAPACITY_BYTES").and_then(|v| v.parse().ok()).unwrap_or(1_073_741_824);
    let site = env("NOCT_SITE_SLUG").unwrap_or_else(|| "notturni".into());
    let bucket = |var: &str, default: &str| env(var).unwrap_or_else(|| default.into());
    let mut plan = vec![
        (bucket("NOCT_S3_BUCKET_AVATARS", "avatars"), true),
        (bucket("NOCT_S3_BUCKET_CONTENT", "notturni-content"), true),
        (bucket("NOCT_S3_BUCKET_AUDIT", "notturni-audit"), false),
    ];
    // destinazione di backup sullo stesso Garage (sviluppo locale): in produzione è un provider esterno
    if let (Some(b), Some(e)) = (env("NOCT_BACKUP_S3_BUCKET"), env("NOCT_BACKUP_S3_ENDPOINT_URL"))
        && Some(e) == env("NOCT_S3_ENDPOINT_URL")
    {
        plan.push((b, false));
    }

    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).build()?;
    let call =
        |method: reqwest::Method, path: &str| client.request(method, format!("{base}{path}")).bearer_auth(&token);

    let deadline = std::time::Instant::now() + GARAGE_READY_TIMEOUT;
    let status: Value = loop {
        match call(reqwest::Method::GET, "/v2/GetClusterStatus").send().await {
            Ok(r) if r.status().is_success() => break r.json().await?,
            Ok(r) if matches!(r.status().as_u16(), 401 | 403) => bail!("Garage admin API: token rifiutato"),
            _ => {}
        }
        if std::time::Instant::now() > deadline {
            bail!("Garage admin API non raggiungibile su {base} dopo {}s", GARAGE_READY_TIMEOUT.as_secs());
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    };

    // layout
    let node = status["nodes"]
        .as_array()
        .and_then(|n| n.iter().find(|n| n["isUp"].as_bool() == Some(true)))
        .ok_or_else(|| anyhow!("Garage: nessun nodo attivo nel cluster"))?;
    let node_id = node["id"].as_str().unwrap_or_default();
    if node["role"].is_null() {
        call(reqwest::Method::POST, "/v2/UpdateClusterLayout")
            .json(&json!({"roles": [{"id": node_id, "zone": GARAGE_ZONE, "capacity": capacity, "tags": []}]}))
            .send()
            .await?
            .error_for_status()?;
        let version = status["layoutVersion"].as_i64().unwrap_or(0) + 1;
        call(reqwest::Method::POST, "/v2/ApplyClusterLayout")
            .json(&json!({"version": version}))
            .send()
            .await?
            .error_for_status()?;
        println!("layout: nodo {} assegnato ({capacity} byte)", &node_id[..node_id.len().min(16)]);
    } else {
        println!("layout: nodo {} già assegnato", &node_id[..node_id.len().min(16)]);
    }

    // chiave applicativa
    let has_key = || async {
        call(reqwest::Method::GET, "/v2/GetKeyInfo")
            .query(&[("id", &key_id)])
            .send()
            .await
            .map(|r| r.status().is_success())
    };
    if has_key().await? {
        println!("chiave {key_id} già presente");
    } else {
        let r = call(reqwest::Method::POST, "/v2/ImportKey")
            .json(&json!({"accessKeyId": key_id, "secretAccessKey": secret, "name": site}))
            .send()
            .await?;
        // più repliche possono eseguire il bootstrap insieme: se un'altra l'ha importata, va bene
        if !r.status().is_success() && !has_key().await? {
            bail!(
                "import della chiave rifiutato da Garage ({}): formato richiesto NOCT_S3_ACCESS_KEY_ID = 'GK' + 24 hex, NOCT_S3_SECRET_ACCESS_KEY = 64 hex",
                r.status()
            );
        }
        println!("chiave {key_id} importata");
    }

    // bucket con alias globale
    for (name, public) in plan {
        let mut r = call(reqwest::Method::GET, "/v2/GetBucketInfo").query(&[("globalAlias", &name)]).send().await?;
        if r.status().as_u16() == 404 {
            r = call(reqwest::Method::POST, "/v2/CreateBucket").json(&json!({"globalAlias": name})).send().await?;
            if !r.status().is_success() {
                // creato nel frattempo da un'altra istanza
                r = call(reqwest::Method::GET, "/v2/GetBucketInfo").query(&[("globalAlias", &name)]).send().await?;
            }
        }
        let info: Value = r.error_for_status()?.json().await?;
        let id = info["id"].as_str().ok_or_else(|| anyhow!("Garage: bucket senza id"))?.to_string();
        call(reqwest::Method::POST, "/v2/AllowBucketKey")
            .json(&json!({"bucketId": id, "accessKeyId": key_id, "permissions": {"read": true, "write": true, "owner": true}}))
            .send()
            .await?
            .error_for_status()?;
        let website =
            if public { json!({"enabled": true, "indexDocument": "index.html"}) } else { json!({"enabled": false}) };
        call(reqwest::Method::POST, "/v2/UpdateBucket")
            .query(&[("id", &id)])
            .json(&json!({"websiteAccess": website}))
            .send()
            .await?
            .error_for_status()?;
        println!("bucket {name} pronto ({})", if public { "pubblico" } else { "privato" });
    }
    Ok(())
}
