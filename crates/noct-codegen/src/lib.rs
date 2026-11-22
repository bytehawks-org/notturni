//! Generatore di codice a build-time per i moduli.
//!
//! Da `module.toml`, `api.toml`, `db.toml` (+ `schema.lock.json` e
//! `migrations/*.sql`) emette in `$OUT_DIR/module_gen.rs`: DTO tipizzati con
//! validazione, handler e router axum per le rotte CRUD, glue verso gli handler
//! `custom` scritti a mano, e la funzione `module()` con tutti i sorgenti
//! incorporati. Il modulo la include con:
//!
//! ```ignore
//! include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));
//! ```
//!
//! Gli errori dei manifest interrompono la compilazione con un messaggio per riga.

mod emit;
mod types;

use noct_manifest::{Manifest, Sources};
use std::{fs, path::Path};

/// Da chiamare dal `build.rs` del modulo.
pub fn build() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    for f in ["module.toml", "api.toml", "db.toml", "schema.lock.json", "migrations"] {
        println!("cargo:rerun-if-changed={f}");
    }
    match generate(Path::new(&dir)) {
        Ok(src) => fs::write(Path::new(&out).join("module_gen.rs"), src).expect("scrittura module_gen.rs"),
        Err(msg) => {
            eprintln!("errore nei manifest del modulo in {dir}:\n{msg}");
            std::process::exit(1);
        }
    }
}

/// Genera il sorgente Rust per la directory di un modulo.
pub fn generate(dir: &Path) -> Result<String, String> {
    let read = |name: &str| -> Result<Option<String>, String> {
        match fs::read_to_string(dir.join(name)) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{name}: {e}")),
        }
    };
    let module = read("module.toml")?.ok_or("manca module.toml")?;
    let api = read("api.toml")?;
    let db = read("db.toml")?;
    let lock = read("schema.lock.json")?;
    let manifest =
        Manifest::parse(Sources { module: &module, api: api.as_deref(), db: db.as_deref(), lock: lock.as_deref() })
            .map_err(|issues| issues.to_string())?;

    let mut migrations = Vec::new();
    if let Ok(rd) = fs::read_dir(dir.join("migrations")) {
        for entry in rd.flatten() {
            let file = entry.file_name().to_string_lossy().to_string();
            if file == ".gitkeep" {
                continue;
            }
            let (version, name) = noct_migrate::parse_file_name(&file).ok_or_else(|| {
                format!("migrations/{file}: nome non valido (atteso NNNN_nome.sql, es. 0001_init.sql)")
            })?;
            migrations.push((version, name, file));
        }
    }
    migrations.sort();
    for (i, (v, _, file)) in migrations.iter().enumerate() {
        if *v != i as u32 + 1 {
            return Err(format!("migrations/{file}: le versioni devono essere consecutive da 0001"));
        }
    }

    emit::emit(&manifest, api.is_some(), db.is_some(), lock.is_some(), &migrations)
}
