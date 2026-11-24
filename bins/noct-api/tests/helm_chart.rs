//! Il chart Helm (`helm/notturni/`) deve produrre manifest coerenti per i profili solo e platform.
//! Non servono cluster né rete: i test di rendering lanciano `helm template` (saltati con un avviso se `helm`
//! non è nel PATH; la CI lo installa, quindi lì non si saltano mai), quelli sui file leggono solo il repository.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("radice del repository")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn chart() -> PathBuf {
    root().join("helm/notturni")
}

fn helm_bin() -> String {
    std::env::var("NOCT_HELM_BIN").unwrap_or_else(|_| "helm".into())
}

fn helm_available() -> bool {
    Command::new(helm_bin()).arg("version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// `helm template` del chart con un file di valori (in `ci/`) più eventuali `--set`. `Err` con lo stderr se fallisce.
fn render(values: &str, sets: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new(helm_bin());
    cmd.arg("template").arg("notturni").arg(chart());
    if !values.is_empty() {
        cmd.arg("-f").arg(chart().join("ci").join(values));
    }
    for s in sets {
        cmd.arg("--set").arg(s);
    }
    let out = cmd.output().expect("helm eseguibile");
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

struct Doc {
    kind: String,
    name: String,
    text: String,
}

/// Documenti del multi-YAML renderizzato: tipo, nome (`metadata.name`) e testo.
fn docs(rendered: &str) -> Vec<Doc> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut flush = |text: &mut String| {
        if let Some(kind) = text.lines().find_map(|l| l.strip_prefix("kind: ")) {
            let after_meta = text.split_once("\nmetadata:\n").map(|(_, r)| r).unwrap_or("");
            let name = after_meta
                .lines()
                .find_map(|l| l.strip_prefix("  name: "))
                .unwrap_or_else(|| panic!("documento {kind} senza nome"))
                .trim_matches('"')
                .to_string();
            out.push(Doc { kind: kind.trim().to_string(), name, text: std::mem::take(text) });
        }
        text.clear();
    };
    for line in rendered.lines() {
        if line == "---" {
            flush(&mut current);
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }
    flush(&mut current);
    out
}

fn find<'a>(docs: &'a [Doc], kind: &str, suffix: &str) -> Option<&'a Doc> {
    docs.iter().find(|d| d.kind == kind && d.name.ends_with(suffix))
}

fn names(docs: &[Doc], kind: &str) -> Vec<String> {
    docs.iter().filter(|d| d.kind == kind).map(|d| d.name.clone()).collect()
}

/// Chiavi `NOCT_*` della ConfigMap `*-config` (indentate di due spazi sotto `data:`).
fn config_keys(docs: &[Doc]) -> BTreeSet<String> {
    let cm = find(docs, "ConfigMap", "-config").expect("ConfigMap di configurazione");
    cm.text
        .lines()
        .filter_map(|l| l.strip_prefix("  NOCT_"))
        .filter_map(|l| l.split_once(':').map(|(k, _)| format!("NOCT_{k}")))
        .collect()
}

fn config_value(docs: &[Doc], key: &str) -> Option<String> {
    let cm = find(docs, "ConfigMap", "-config")?;
    cm.text
        .lines()
        .find_map(|l| l.trim_start().strip_prefix(&format!("{key}: ")))
        .map(|v| v.trim().trim_matches('"').to_string())
}

macro_rules! need_helm {
    () => {
        if !helm_available() {
            eprintln!("SKIP: `helm` non è nel PATH (NOCT_HELM_BIN): test di rendering del chart saltato");
            return;
        }
    };
}

// ---- file e valori (senza helm) -------------------------------------------------------------------------------

/// I file di configurazione del chart sono copie identiche di quelli di k8s/ (e di compose.yaml).
#[test]
fn chart_files_match_their_k8s_sources() {
    for (copy, source) in [
        ("helm/notturni/files/garage.toml", "k8s/garage.toml"),
        ("helm/notturni/files/s3-public.conf.template", "k8s/s3-public.conf.template"),
    ] {
        assert_eq!(read(copy), read(source), "{copy} diverge da {source}");
    }
}

#[test]
fn values_schema_pins_mode_and_storage_type() {
    let schema: serde_json::Value = serde_json::from_str(&read("helm/notturni/values.schema.json")).unwrap();
    assert_eq!(schema["properties"]["mode"]["enum"], serde_json::json!(["solo", "platform"]));
    assert_eq!(schema["properties"]["storage"]["properties"]["type"]["enum"], serde_json::json!(["local", "s3"]));
    assert_eq!(schema["properties"]["ingress"]["required"], serde_json::json!(["host"]));
}

/// Nessun segreto nei default, immagini fissate e senza tag legati a un'architettura.
#[test]
fn default_values_hold_no_secrets_and_pin_images() {
    let values = read("helm/notturni/values.yaml");
    assert!(values.contains("existingSecret: \"\"") && values.contains("extra: {}"));
    let uncommented: Vec<&str> = values.lines().filter(|l| !l.trim_start().starts_with('#')).collect();
    for line in &uncommented {
        let l = line.to_lowercase();
        assert!(
            !l.contains("changeme") && !l.contains("password:") && !l.contains("secret:")
                || l.contains("existingsecret"),
            "{line}"
        );
        assert!(!l.contains("arm64v8") && !l.contains("amd64/"), "{line}");
        assert!(!l.trim_end().ends_with("tag: latest"), "tag non fissato: {line}");
    }
    assert!(values.contains("mode: solo") && values.contains("type: local"));
}

#[test]
fn notes_never_print_secret_values() {
    let notes = read("helm/notturni/templates/NOTES.txt");
    assert!(!notes.contains("secrets.extra") && !notes.contains("lookup") && !notes.contains("b64dec("));
    // l'unico riferimento al segreto è il comando per leggerlo, che non lo stampa nel rendering
    assert!(!notes.contains(".Values.secrets.extra"));
}

// ---- rendering --------------------------------------------------------------------------------------------------

#[test]
fn solo_render_is_lean() {
    need_helm!();
    let out = render("solo-values.yaml", &[]).unwrap_or_else(|e| panic!("helm template: {e}"));
    let d = docs(&out);
    let workloads: Vec<String> = names(&d, "Deployment").into_iter().chain(names(&d, "StatefulSet")).collect();
    for needed in ["backend", "frontend", "postgres"] {
        assert!(workloads.iter().any(|n| n.ends_with(needed)), "manca {needed}: {workloads:?}");
    }
    assert_eq!(workloads.len(), 3, "solo = postgres + backend + frontend: {workloads:?}");
    for absent in ["redis", "nats", "garage", "s3-public", "moderation", "worker", "mailhog", "smtp-relay"] {
        assert!(!d.iter().any(|x| x.name.contains(absent)), "{absent} non deve esistere in solo");
    }
    assert!(!d.iter().any(|x| x.kind == "IngressRoute" || x.kind == "Certificate"), "nessun wildcard in solo");

    let keys = config_keys(&d);
    for forbidden in
        ["NOCT_NATS_URL", "NOCT_REDIS_HOST", "NOCT_REDIS_PORT", "NOCT_MODERATION_SERVICE_URL", "NOCT_SMTP_HOST"]
    {
        assert!(!keys.contains(forbidden), "{forbidden} deve essere assente (non vuoto) in solo");
    }
    assert!(
        !out.contains("NOCT_REDIS") && !out.contains("NOCT_NATS_URL") && !out.contains("NOCT_MODERATION_SERVICE_URL")
    );
    assert_eq!(config_value(&d, "NOCT_STORAGE_BACKEND").as_deref(), Some("localstorage"));
    assert_eq!(config_value(&d, "NOCT_DEPLOYMENT_MODE").as_deref(), Some("solo"));
    assert_eq!(config_value(&d, "NOCT_SUBDOMAINS_ENABLED").as_deref(), Some("false"));
    assert_eq!(config_value(&d, "NOCT_LOCAL_STORAGE_PUBLIC_URL").as_deref(), Some("https://example.com/storage"));
    assert!(!config_value(&d, "NOCT_MODULES").unwrap().split(',').any(|m| m == "image_moderation"));

    let backend = find(&d, "Deployment", "-backend").unwrap();
    assert!(backend.text.contains("replicas: 1") && backend.text.contains("type: Recreate"));
    assert!(backend.text.contains("persistentVolumeClaim") && backend.text.contains("mountPath: /data/storage"));
    assert!(!backend.text.contains("initContainers"), "nessun garage-bootstrap in solo");
    assert!(backend.text.contains("runAsNonRoot: true") && backend.text.contains("drop: [\"ALL\"]"));
    // il frontend non riceve l'intero Secret
    let frontend = find(&d, "Deployment", "-frontend").unwrap();
    assert!(!frontend.text.contains("secretRef"));

    let ingress = find(&d, "Ingress", "").unwrap();
    for path in ["path: /api", "path: /storage", "path: /"] {
        assert!(ingress.text.contains(path), "Ingress senza {path}");
    }
    let secret = find(&d, "Secret", "-secrets").expect("Secret generato");
    for key in ["NOCT_POSTGRES_PASSWORD", "NOCT_JWT_SECRET", "NOCT_REVALIDATE_SECRET"] {
        assert!(secret.text.contains(key), "Secret senza {key}");
    }
    assert!(!out.contains("arm64v8"));
}

#[test]
fn platform_render_has_every_component() {
    need_helm!();
    let out = render("platform-values.yaml", &[]).unwrap_or_else(|e| panic!("helm template: {e}"));
    let d = docs(&out);
    let deployments = names(&d, "Deployment");
    for needed in [
        "backend",
        "frontend",
        "postgres",
        "worker",
        "nats",
        "redis",
        "moderation",
        "garage",
        "s3-public",
        "smtp-relay",
    ] {
        assert!(deployments.iter().any(|n| n.ends_with(&format!("-{needed}"))), "manca {needed}: {deployments:?}");
    }
    assert_eq!(names(&d, "IngressRoute").len(), 2, "wildcard + proxy S3");
    assert!(find(&d, "Certificate", "-tls").unwrap().text.contains("*.example.com"));
    assert_eq!(names(&d, "NetworkPolicy").len(), 3);
    assert_eq!(config_value(&d, "NOCT_DEPLOYMENT_MODE").as_deref(), Some("platform"));
    assert_eq!(config_value(&d, "NOCT_SUBDOMAINS_ENABLED").as_deref(), Some("true"));
    assert_eq!(config_value(&d, "NOCT_STORAGE_BACKEND").as_deref(), Some("s3"));
    assert_eq!(config_value(&d, "NOCT_S3_PUBLIC_URL").as_deref(), Some("https://s3.example.com"));
    assert!(config_value(&d, "NOCT_NATS_URL").unwrap().starts_with("nats://"));
    assert!(config_value(&d, "NOCT_MODERATION_SERVICE_URL").is_some());
    assert!(
        config_value(&d, "NOCT_CORS_ORIGIN_REGEX").is_some()
            && config_value(&d, "NOCT_SESSION_COOKIE_DOMAIN").is_some()
    );
    assert!(config_value(&d, "NOCT_MODULES").is_none(), "modules: [] lascia il default del backend");
    assert_eq!(config_value(&d, "NOCT_SMTP_PORT").as_deref(), Some("587"));
    let backend = find(&d, "Deployment", "-backend").unwrap();
    assert!(backend.text.contains("garage-bootstrap") && !backend.text.contains("type: Recreate"));
    assert!(backend.text.contains("replicas: 2"));
    let worker = find(&d, "Deployment", "-worker").unwrap();
    assert!(worker.text.contains("noct-worker") && worker.text.contains("NOCT_AUTO_MIGRATE"));
    // i segreti di Garage e dello scheduler stanno solo nel Secret
    let secret = find(&d, "Secret", "-secrets").unwrap();
    for key in ["NOCT_GARAGE_RPC_SECRET", "NOCT_GARAGE_ADMIN_TOKEN", "NOCT_S3_SECRET_ACCESS_KEY"] {
        assert!(secret.text.contains(key), "Secret senza {key}");
        assert!(config_value(&d, key).is_none());
    }
    assert!(!out.contains("arm64v8"));
}

#[test]
fn full_render_covers_external_services_and_existing_secret() {
    need_helm!();
    let out = render("full-values.yaml", &[]).unwrap_or_else(|e| panic!("helm template: {e}"));
    let d = docs(&out);
    assert!(find(&d, "Secret", "-secrets").is_none(), "con existingSecret il chart non crea il Secret");
    assert!(out.contains("my-notturni-secrets") && out.contains("claimName: my-postgres"));
    assert!(!d.iter().any(|x| x.name.contains("garage") || x.name.contains("s3-public")));
    assert_eq!(config_value(&d, "NOCT_S3_ENDPOINT_URL").as_deref(), Some("https://s3.eu.example.com"));
    assert_eq!(config_value(&d, "NOCT_S3_PUBLIC_URL").as_deref(), Some("https://cdn.example.com"));
    assert_eq!(config_value(&d, "NOCT_SMTP_HOST").as_deref(), Some("smtp.example.com"));
    assert_eq!(config_value(&d, "NOCT_SMTP_USE_TLS").as_deref(), Some("true"));
    // image_moderation resta perché moderation.enabled=true
    assert!(config_value(&d, "NOCT_MODULES").unwrap().split(',').any(|m| m == "image_moderation"));
    assert!(find(&d, "Ingress", "-nct").unwrap().text.contains("example.com/annotation"));
    assert!(
        !d.iter().any(|x| x.kind == "PersistentVolumeClaim" && x.text.contains("resource-policy")),
        "keepOnUninstall=false"
    );
}

#[test]
fn moderation_flag_and_module_list_stay_consistent() {
    need_helm!();
    let off =
        render("platform-values.yaml", &["modules={pages,image_moderation}", "moderation.enabled=false"]).unwrap();
    let d = docs(&off);
    assert_eq!(config_value(&d, "NOCT_MODULES").as_deref(), Some("pages"));
    assert!(config_value(&d, "NOCT_MODERATION_SERVICE_URL").is_none());
    let on = render("platform-values.yaml", &["modules={pages}"]).unwrap();
    assert_eq!(config_value(&docs(&on), "NOCT_MODULES").as_deref(), Some("pages,image_moderation"));
}

#[test]
fn smtp_host_alone_configures_external_smtp() {
    need_helm!();
    let out = render("solo-values.yaml", &["smtp.host=smtp.example.com"]).unwrap();
    let d = docs(&out);
    assert_eq!(config_value(&d, "NOCT_SMTP_HOST").as_deref(), Some("smtp.example.com"));
    assert!(!d.iter().any(|x| x.name.contains("smtp-relay")));
}

#[test]
fn invalid_combinations_fail_to_render() {
    need_helm!();
    let cases: [(&str, &[&str], &str); 7] = [
        ("solo + subdomains", &["subdomains.enabled=true"], "mode=platform"),
        ("local storage + 2 replicas", &["backend.replicaCount=2"], "replicaCount must be 1"),
        ("s3 without endpoint", &["storage.type=s3"], "storage.s3.endpointUrl"),
        ("s3 without public url", &["storage.type=s3", "storage.s3.endpointUrl=https://s3.example.com"], "publicUrl"),
        ("garage with local storage", &["storage.s3.garage.enabled=true"], "storage.type=s3"),
        ("no host", &["ingress.host="], "ingress"),
        ("unknown mode", &["mode=cloud"], "mode"),
    ];
    for (label, sets, expected) in cases {
        let err = render("solo-values.yaml", sets).expect_err(label);
        assert!(err.contains(expected), "{label}: messaggio inatteso: {err}");
    }
    // senza NATS il bus è in processo: una sola replica anche con S3
    let err = render(
        "solo-values.yaml",
        &[
            "storage.type=s3",
            "storage.s3.endpointUrl=https://s3.example.com",
            "storage.s3.publicUrl=https://s3.example.com",
            "backend.replicaCount=2",
        ],
    )
    .expect_err("nats off + 2 repliche");
    assert!(err.contains("nats"), "{err}");
}

/// Ogni variabile `NOCT_*` emessa dal chart è letta dal backend, dal worker, dal frontend o da un servizio
/// ausiliario noto: nessun refuso nei nomi.
#[test]
fn every_chart_variable_is_read_by_the_stack() {
    need_helm!();
    let name = regex::Regex::new(r"NOCT_[A-Z0-9_]+").unwrap();
    let mut read_by_stack: BTreeSet<String> = BTreeSet::new();
    for file in [
        "crates/noct-core/src/config.rs",
        "bins/noct-api/src/lib.rs",
        "bins/noct-ctl/src/main.rs",
        "frontend/src/lib/site.ts",
        "frontend/src/lib/server-api.ts",
        "frontend/src/proxy.ts",
        "crates/noct-audit/module.toml",
    ] {
        read_by_stack.extend(name.find_iter(&read(file)).map(|m| m.as_str().trim_end_matches('_').to_string()));
    }
    for dir in ["crates", "modules"] {
        for entry in std::fs::read_dir(root().join(dir)).unwrap().flatten() {
            if let Ok(text) = std::fs::read_to_string(entry.path().join("module.toml")) {
                read_by_stack.extend(name.find_iter(&text).map(|m| m.as_str().to_string()));
            }
        }
    }
    // lette solo da servizi ausiliari: smtp-relay (postfix), nginx di s3-public (envsubst), cert-manager (token DNS-01)
    let auxiliary = [
        "NOCT_SMTP_RELAY_HOST",
        "NOCT_SMTP_RELAY_USERNAME",
        "NOCT_SMTP_RELAY_PASSWORD",
        "NOCT_GARAGE_RPC_SECRET",
        "NOCT_GARAGE_WEB_UPSTREAM",
        "NOCT_CLOUDFLARE_API_TOKEN",
    ];
    let mut emitted: BTreeSet<String> = BTreeSet::new();
    for values in ["solo-values.yaml", "platform-values.yaml", "full-values.yaml"] {
        emitted.extend(name.find_iter(&render(values, &[]).unwrap()).map(|m| m.as_str().to_string()));
    }
    assert!(emitted.len() > 25, "variabili non lette dal rendering: {emitted:?}");
    let unknown: Vec<&String> = emitted
        .iter()
        .filter(|v| {
            !read_by_stack.iter().any(|r| r == *v || (r.ends_with('_') && v.starts_with(r.as_str())))
                && !auxiliary.contains(&v.as_str())
                && !v.starts_with("NOCT_POSTGRES")
        })
        .collect();
    assert!(unknown.is_empty(), "variabili del chart che nessuno legge: {unknown:?}");
}

#[test]
fn chart_lints_for_every_values_set() {
    need_helm!();
    for values in ["solo-values.yaml", "platform-values.yaml", "full-values.yaml"] {
        let out = Command::new(helm_bin())
            .arg("lint")
            .arg(chart())
            .arg("-f")
            .arg(chart().join("ci").join(values))
            .output()
            .expect("helm eseguibile");
        assert!(out.status.success(), "helm lint {values}: {}", String::from_utf8_lossy(&out.stdout));
    }
}
