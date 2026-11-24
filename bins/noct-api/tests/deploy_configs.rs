//! I file di deploy non devono divergere né puntare a cose che non esistono più.
//! Non richiedono servizi: leggono i file del repository.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("radice del repository")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// Le copie in k8s/ dei file condivisi con compose.yaml (kustomize non legge fuori dalla propria
/// directory) devono restare identiche agli originali.
#[test]
fn k8s_copies_match_their_sources() {
    for (source, copy) in
        [("garage/garage.toml", "k8s/garage.toml"), ("s3-public/default.conf.template", "k8s/s3-public.conf.template")]
    {
        assert_eq!(read(copy), read(source), "{copy} diverge da {source}");
    }
}

#[test]
fn every_kustomize_resource_exists() {
    let kustomization = read("k8s/kustomization.yaml");
    let mut in_resources = false;
    let mut count = 0;
    for line in kustomization.lines() {
        if line.starts_with("resources:") {
            in_resources = true;
            continue;
        }
        if in_resources {
            let Some(item) = line.strip_prefix("  - ") else {
                if !line.trim_start().starts_with('#') && !line.trim().is_empty() {
                    in_resources = false;
                }
                continue;
            };
            // secret.yaml è locale (ignorato da git): si parte da secret.example.yaml
            if item.trim() == "secret.yaml" {
                continue;
            }
            assert!(root().join("k8s").join(item.trim()).exists(), "k8s/{item} non esiste");
            count += 1;
        }
    }
    assert!(count > 10, "elenco delle risorse non letto");
}

/// Il backend è Rust: nessun manifest o compose deve più lanciare Python, alembic o RabbitMQ per il backend.
#[test]
fn deploy_files_no_longer_reference_the_python_backend() {
    let files = std::fs::read_dir(root().join("k8s")).unwrap().filter_map(|e| e.ok()).map(|e| e.path());
    let mut contents: Vec<(String, String)> = files
        .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
        .map(|p| (p.display().to_string(), std::fs::read_to_string(&p).unwrap()))
        .collect();
    contents.push(("compose.yaml".into(), read("compose.yaml")));
    for (name, text) in contents {
        // il servizio di moderazione resta Python: è lui a poter lanciare `python`
        if name.ends_with("moderation.yaml") {
            continue;
        }
        for needle in ["app.workers", "scripts.garage_bootstrap", "alembic", "rabbitmq", "legacy/backend"] {
            let hits: Vec<&str> = text
                .lines()
                .filter(|l| !l.trim_start().starts_with('#') && l.to_lowercase().contains(needle))
                .collect();
            assert!(hits.is_empty(), "{name} cita `{needle}`: {hits:?}");
        }
        for line in text.lines().filter(|l| !l.trim_start().starts_with('#')) {
            assert!(
                !(line.contains("\"python\"") && !name.ends_with("moderation.yaml") && !line.contains("urllib")),
                "{name}: {line}"
            );
        }
    }
}

#[test]
fn the_dockerfile_builds_the_three_binaries_for_any_architecture() {
    let dockerfile = read("Dockerfile");
    for bin in ["noct-api", "noct-worker", "noct-ctl"] {
        assert!(dockerfile.contains(bin), "{bin} non è nella Dockerfile");
    }
    assert!(dockerfile.contains("cargo chef"), "cache dei layer con cargo-chef");
    assert!(dockerfile.contains("postgresql-client"), "pg_dump per il backup infrastrutturale");
    assert!(
        !dockerfile.contains("arm64v8") && !dockerfile.contains("amd64/"),
        "niente immagini di base legate a un'architettura"
    );
    assert!(dockerfile.contains("USER 10001"), "non gira come root, con UID numerico (runAsNonRoot)");
}

// ---- .env.example ----------------------------------------------------------------------------------------------------

/// `NOME=valore` dalle righe non commentate.
fn env_example() -> std::collections::BTreeMap<String, String> {
    read(".env.example")
        .lines()
        .filter(|l| !l.trim_start().starts_with('#') && l.contains('='))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

/// Il file d'esempio, così com'è, deve produrre una configurazione valida per il core e per ogni modulo.
#[test]
fn env_example_is_a_valid_configuration() {
    let mut vars = env_example();
    // il segreto d'esempio viene rifiutato di proposito (vedi sotto): qui lo si sostituisce come farebbe l'operatore
    let placeholder = vars.insert("NOCT_JWT_SECRET".into(), "x".repeat(48)).unwrap();
    assert!(noct_core::is_placeholder_secret(&placeholder), "il segreto d'esempio deve essere un segnaposto");
    let lookup = |k: &str| vars.get(k).cloned().filter(|v| !v.is_empty());
    noct_core::Config::from_env(&lookup).unwrap_or_else(|e| panic!(".env.example: {e}"));
    let registry = noct_core::Registry::new(noct_bundle::all()).unwrap();
    let enabled = registry.resolve_enabled(None).unwrap();
    registry.module_configs(&enabled, &lookup).unwrap_or_else(|e| panic!(".env.example: {e}"));
}

/// Ogni variabile che il backend legge è documentata in `.env.example`, e quel file non cita variabili
/// che nessuno legge più (la v1 ne aveva diverse, rimaste dopo il porting).
#[test]
fn env_example_documents_exactly_the_variables_the_backend_reads() {
    let documented: std::collections::BTreeSet<String> = {
        let mut set: std::collections::BTreeSet<String> = env_example().into_keys().collect();
        // le variabili commentate ma presenti come esempio
        for line in read(".env.example").lines().filter(|l| l.starts_with("# NOCT_")) {
            if let Some((k, _)) = line.trim_start_matches("# ").split_once('=') {
                set.insert(k.to_string());
            }
        }
        set
    };
    let name = regex::Regex::new(r"NOCT_[A-Z0-9_]+").unwrap();
    let mut read_by_backend: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for file in [
        "crates/noct-core/src/config.rs",
        "bins/noct-api/src/lib.rs",
        "bins/noct-ctl/src/main.rs",
        "crates/noct-audit/module.toml",
    ] {
        read_by_backend.extend(name.find_iter(&read(file)).map(|m| m.as_str().trim_end_matches('_').to_string()));
    }
    for dir in ["crates", "modules"] {
        for entry in std::fs::read_dir(root().join(dir)).unwrap().flatten() {
            if let Ok(text) = std::fs::read_to_string(entry.path().join("module.toml")) {
                read_by_backend.extend(name.find_iter(&text).map(|m| m.as_str().to_string()));
            }
        }
    }
    // variabili lette dal frontend Next.js a runtime (non dal backend): le si cerca dove il frontend le legge
    for file in ["frontend/src/lib/site.ts", "frontend/src/proxy.ts"] {
        read_by_backend.extend(name.find_iter(&read(file)).map(|m| m.as_str().trim_end_matches('_').to_string()));
    }
    // solo compose.yaml le legge (container diversi dal backend)
    let compose_only =
        ["NOCT_SMTP_RELAY_HOST", "NOCT_SMTP_RELAY_USERNAME", "NOCT_SMTP_RELAY_PASSWORD", "NOCT_GARAGE_RPC_SECRET"];
    // variabili di modulo di sviluppo (sandbox) e prefissi parziali citati nei messaggi
    let ignore =
        ["NOCT_SANDBOX_GREETING", "NOCT_SANDBOX_MAX_TITLE", "NOCT_POSTGRES", "NOCT_WORKER_INLINE", "NOCT_TEST"];
    let undocumented: Vec<&String> = read_by_backend
        .iter()
        .filter(|v| !documented.contains(*v) && !ignore.iter().any(|i| v.starts_with(i)) && v.as_str() != "NOCT_")
        .collect();
    assert!(undocumented.is_empty(), "variabili lette dal backend e assenti da .env.example: {undocumented:?}");
    let stale: Vec<&String> = documented
        .iter()
        .filter(|v| {
            !read_by_backend.contains(*v) && !compose_only.contains(&v.as_str()) && !v.starts_with("NOCT_POSTGRES")
        })
        .collect();
    assert!(stale.is_empty(), ".env.example cita variabili che il backend non legge: {stale:?}");
}

/// Il file d'esempio, copiato in `.env`, avvia lo stack minimo "solo": niente Redis, NATS, S3, moderazione.
#[test]
fn env_example_defaults_to_the_lean_solo_stack() {
    let mut vars = env_example();
    assert_eq!(vars.get("NOCT_DEPLOYMENT_MODE").map(String::as_str), Some("solo"));
    assert_eq!(vars.get("NOCT_STORAGE_BACKEND").map(String::as_str), Some("localstorage"));
    assert_eq!(vars.get("NOCT_SUBDOMAINS_ENABLED").map(String::as_str), Some("false"));
    for key in ["NOCT_REDIS_HOST", "NOCT_REDIS_PORT", "NOCT_REDIS_DB", "NOCT_REDIS_URL"] {
        assert!(!vars.contains_key(key), "{key} non deve essere attivo in solo");
    }
    for key in ["NOCT_NATS_URL", "NOCT_MODERATION_SERVICE_URL"] {
        assert_eq!(vars.get(key).map(String::as_str), Some(""), "{key} deve essere presente e vuoto");
    }
    assert!(!vars.keys().any(|k| k.starts_with("NOCT_S3_")), "nessuna variabile S3 attiva");
    let modules = vars.get("NOCT_MODULES").cloned().unwrap_or_default();
    assert!(!modules.is_empty() && !modules.split(',').any(|m| m == "image_moderation"), "NOCT_MODULES: {modules}");
    vars.insert("NOCT_JWT_SECRET".into(), "x".repeat(48));
    let lookup = |k: &str| vars.get(k).cloned().filter(|v| !v.is_empty());
    let cfg = noct_core::Config::from_env(&lookup).unwrap_or_else(|e| panic!(".env.example: {e}"));
    assert!(cfg.worker_inline, "senza NATS i consumer girano nel processo API");
    assert!(cfg.nats_url.is_none() && cfg.redis_url.is_none() && cfg.moderation_service_url.is_none());
    assert!(!cfg.subdomains_enabled);
    assert_eq!(cfg.storage.backend, noct_core::StorageBackend::Local);
    let registry = noct_core::Registry::new(noct_bundle::all()).unwrap();
    registry.resolve_enabled(cfg.modules.as_deref()).unwrap_or_else(|e| panic!("NOCT_MODULES: {e:?}"));
}

/// Il ConfigMap Kustomize è l'installazione "platform": le due variabili di modalità sono esplicite
/// (le leggono backend, worker e frontend) e il frontend carica il ConfigMap per intero.
#[test]
fn k8s_configmap_sets_mode_and_subdomain_flags() {
    let configmap = read("k8s/configmap.yaml");
    let value = |key: &str| {
        configmap
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .find_map(|l| l.trim().strip_prefix(&format!("{key}:")))
            .map(|v| v.trim().trim_matches('"').to_string())
    };
    assert_eq!(value("NOCT_DEPLOYMENT_MODE").as_deref(), Some("platform"));
    assert_eq!(value("NOCT_SUBDOMAINS_ENABLED").as_deref(), Some("true"));
    assert!(
        read("k8s/frontend.yaml").contains("configMapRef: {name: notturni-config}"),
        "il frontend legge il ConfigMap"
    );
}

/// Blocco di un servizio di compose (da `  nome:` alla riga successiva con la stessa indentazione).
fn compose_service(text: &str, name: &str) -> String {
    let header = format!("  {name}:");
    let mut lines = text.lines().skip_while(|l| *l != header);
    assert!(lines.next().is_some(), "servizio `{name}` assente");
    let mut block = vec![header.as_str()];
    block.extend(lines.take_while(|l| l.is_empty() || l.starts_with("   ") || l.trim_start().starts_with('#')));
    block.join("\n")
}

/// `podman compose up` avvia lo stack minimo: i servizi platform stanno nei profili, il backend non li
/// forza né li aspetta (podman-compose 1.6 non ammette `depends_on` con condition verso profili inattivi).
#[test]
fn compose_starts_the_lean_solo_stack_by_default() {
    let compose = read("compose.yaml");
    let active = |l: &&str| !l.trim_start().starts_with('#');
    for (svc, profile) in [
        ("redis", "platform"),
        ("nats", "platform"),
        ("garage", "platform"),
        ("garage-bootstrap", "platform"),
        ("s3-public", "platform"),
        ("moderation", "platform"),
        ("worker", "platform"),
        ("smtp-relay", "mail-relay"),
        ("mailhog", "dev-mail"),
    ] {
        let block = compose_service(&compose, svc);
        assert!(block.contains(&format!("profiles: [\"{profile}\"]")), "{svc} deve stare nel profilo {profile}");
    }
    for svc in ["postgres", "backend", "frontend"] {
        assert!(!compose_service(&compose, svc).contains("profiles:"), "{svc} fa parte dello stack solo");
    }
    let backend = compose_service(&compose, "backend");
    let lines: Vec<&str> = backend.lines().filter(active).collect();
    for key in ["NOCT_REDIS_HOST", "NOCT_NATS_URL", "NOCT_MODERATION_SERVICE_URL"] {
        let line = lines.iter().find(|l| l.contains(&format!("{key}:"))).unwrap_or_else(|| panic!("{key} assente"));
        assert!(line.contains(":-}"), "{key} deve essere un'interpolazione con default vuoto: {line}");
    }
    for svc in ["redis", "nats", "garage-bootstrap", "moderation", "mailhog"] {
        let deps: Vec<&&str> = lines.iter().filter(|l| l.trim() == format!("{svc}:")).collect();
        assert!(deps.is_empty(), "il backend di base non dipende da {svc} (sta in compose.platform.yaml)");
    }
    assert!(backend.contains("storage_data:/data/storage"), "storage locale su volume nominato");
    // l'override platform collega backend e worker ai servizi dei profili
    let platform = read("compose.platform.yaml");
    for needle in
        ["NOCT_NATS_URL: nats://nats:4222", "NOCT_REDIS_HOST: redis", "NOCT_STORAGE_BACKEND: s3", "garage-bootstrap:"]
    {
        assert!(platform.contains(needle), "compose.platform.yaml: manca `{needle}`");
    }
    assert!(!compose.contains("arm64v8") && !platform.contains("arm64v8"));
}
