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
    let config = Config::from_env(&env).map_err(anyhow::Error::msg)?;
    PgPoolOptions::new().max_connections(2).connect(&config.database_url).await.context("connessione a Postgres")
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
