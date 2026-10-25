//! Modulo `git_sync`: configurazione (proprietario, validazione, protezione SSRF, token mai restituito)
//! e un giro di sincronizzazione vero contro un repository bare locale (richiede `git` nel PATH).

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use std::{path::Path, process::Command};
use uuid::Uuid;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git installato");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Un repository bare con `main` come branch predefinito e un primo commit; `seed = false` lo lascia vuoto.
fn bare_repo(seed: bool) -> (std::path::PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("noct-gs-remote-{}", Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "--bare", "-q", "-b", "main"]);
    let url = format!("file://{}", dir.display());
    if seed {
        let work = std::env::temp_dir().join(format!("noct-gs-seed-{}", Uuid::new_v4().simple()));
        std::fs::create_dir_all(&work).unwrap();
        git(&work, &["init", "-q", "-b", "main"]);
        std::fs::write(work.join("README.md"), "# testi\n").unwrap();
        git(&work, &["add", "-A"]);
        git(&work, &["-c", "user.name=x", "-c", "user.email=x@example.com", "commit", "-q", "-m", "init"]);
        git(&work, &["push", "-q", &url, "main:main"]);
        let _ = std::fs::remove_dir_all(work);
    }
    (dir, url)
}

/// Finti `ssh` e `ssh-keyscan` per provare l'intero giro senza un server SSH: lo scanner stampa una host key
/// vera generata al volo; `ssh` registra argomenti, permessi e contenuto dei file ricevuti, poi esegue il
/// comando remoto in locale (il "server" è un repository bare sul disco). Restituisce
/// `(ssh, keyscan, log_ssh, log_scan, host_key_line)`.
fn fake_ssh_tools() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf, std::path::PathBuf, String) {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("noct-gs-fake-{}", Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["--version"]);
    assert!(
        Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-f", "hostkey"])
            .current_dir(&dir)
            .status()
            .unwrap()
            .success()
    );
    let pubkey = std::fs::read_to_string(dir.join("hostkey.pub")).unwrap();
    let line = format!("127.0.0.1 {}", pubkey.split_whitespace().take(2).collect::<Vec<_>>().join(" "));
    let (log_ssh, log_scan) = (dir.join("ssh.log"), dir.join("scan.log"));
    let write = |name: &str, body: String| {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    };
    let scan = write(
        "keyscan",
        format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\necho '# 127.0.0.1:22 SSH-2.0-fake'\necho '{line}'\n",
            log_scan.display()
        ),
    );
    let ssh = write(
        "ssh",
        format!(
            r#"#!/bin/sh
for a in "$@"; do last="$a"; done
prev=""
for a in "$@"; do
  [ "$prev" = "-i" ] && key="$a"
  case "$a" in UserKnownHostsFile=*) kh="${{a#UserKnownHostsFile=}}";; esac
  prev="$a"
done
{{ echo "ARGS: $*"; echo "KEYMODE: $(stat -c %a "$key")"; echo "KEY: $(head -1 "$key")"; echo "KNOWN: $(cat "$kh")"; }} >> '{}'
exec sh -c "$last"
"#,
            log_ssh.display()
        ),
    );
    (ssh, scan, log_ssh, log_scan, line)
}

async fn space(env: &Env, owner: Uuid, slug: &str, kind: &str) -> Uuid {
    let id = env.blog(slug, owner, "public").await;
    sqlx::query("UPDATE blogs SET kind = $2::space_kind WHERE id = $1")
        .bind(id)
        .bind(kind)
        .execute(&env.pool)
        .await
        .unwrap();
    id
}

async fn page(env: &Env, who: Uuid, blog: &str, slug: &str, extra: Value, publish: bool) -> Value {
    let mut body = json!({"slug": slug, "title": format!("Pagina {slug}"), "content": format!("testo di {slug}")});
    body.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    let (s, p) = env.post(&format!("/blogs/{blog}/posts"), Some(who), body).await;
    assert_eq!(s, StatusCode::CREATED, "{p}");
    if publish {
        let (s, b) = env.post(&format!("/posts/{}/publish", p["id"].as_str().unwrap()), Some(who), json!({})).await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    p
}

async fn config_id(env: &Env, blog: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT id FROM git_sync_configs WHERE blog_id = $1")
        .bind(blog)
        .fetch_one(&env.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn configuration_is_owner_only_validated_and_never_leaks_the_token() {
    let env = setup(&["git_sync"], &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    let blog = space(&env, anna, "guida", "wiki").await;
    env.member(blog, bruno, "co_autore").await;
    let url = "/blogs/guida/git-sync";

    assert_eq!(env.get(url, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.get(url, Some(bruno)).await.0, StatusCode::FORBIDDEN, "nemmeno un co-autore");
    let (s, c) = env.get(url, Some(anna)).await;
    assert_eq!((s, c["configured"].clone()), (StatusCode::OK, json!(false)));

    // protezione SSRF e URL non ammessi (qui `allow_private_hosts` è spento)
    for bad in [
        "https://localhost/a/b.git",
        "https://127.0.0.1/a/b.git",
        "https://169.254.169.254/latest",
        "https://[::1]/a/b.git",
        "http://github.com/a/b.git",
        "ssh://git@github.com/a/b.git",
        "file:///tmp/repo",
        "https://user:pw@github.com/a/b.git",
    ] {
        let body = json!({"remote_url": bad, "secret": "tok"});
        let (s, b) = env.put(url, Some(anna), body).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad}: {b}");
    }
    let (s, _) = env
        .put(url, Some(anna), json!({"remote_url": "https://8.8.8.8/a/b.git", "secret": "tok", "branch": "a..b"}))
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "branch non valido");
    let (s, b) = env.put(url, Some(anna), json!({"remote_url": "https://8.8.8.8/a/b.git"})).await;
    assert_eq!((s, detail(&b).contains("obbligatorio")), (StatusCode::BAD_REQUEST, true), "{b}");
    let (s, _) =
        env.put(url, Some(anna), json!({"remote_url": "https://8.8.8.8/a/b.git", "secret": " con spazi "})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(
        env.put(url, Some(bruno), json!({"remote_url": "https://8.8.8.8/a/b.git", "secret": "t"})).await.0,
        StatusCode::FORBIDDEN
    );

    let ok = json!({"remote_url": "https://8.8.8.8/anna/testi.git", "secret": "ghp_supersegreto"});
    let (s, c) = env.put(url, Some(anna), ok).await;
    assert_eq!(s, StatusCode::OK, "{c}");
    assert_eq!(
        (c["configured"].clone(), c["branch"].clone(), c["username"].clone()),
        (json!(true), json!("notturni-backup"), json!("git"))
    );
    assert_eq!(c["has_secret"], true);
    assert!(!c.to_string().contains("ghp_supersegreto") && c.get("secret").is_none());
    let stored: String = sqlx::query_scalar("SELECT secret FROM git_sync_configs").fetch_one(&env.pool).await.unwrap();
    assert!(stored.starts_with("enc:v1:") && !stored.contains("ghp_"), "il token è cifrato a riposo");

    // il token resta se non viene reinviato; cambiare il branch azzera l'esito
    let (s, c) =
        env.put(url, Some(anna), json!({"remote_url": "https://8.8.8.8/anna/testi.git", "branch": "altro"})).await;
    assert_eq!((s, c["branch"].clone(), c["has_secret"].clone()), (StatusCode::OK, json!("altro"), json!(true)), "{c}");

    // audit, esportazione e rimozione
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'git_sync.configured'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(n, 2);
    assert_eq!(env.delete(url, Some(bruno)).await, StatusCode::FORBIDDEN);
    assert_eq!(env.delete(url, Some(anna)).await, StatusCode::NO_CONTENT);
    assert_eq!(env.delete(url, Some(anna)).await, StatusCode::NOT_FOUND);
    assert_eq!(env.get(url, Some(anna)).await.1["configured"], false);
    env.teardown().await;
}

#[tokio::test]
async fn a_sync_exports_the_tree_to_a_dedicated_branch_and_skips_unchanged_runs() {
    let env = setup(&["wiki", "git_sync"], &[("NOCT_GIT_SYNC_ALLOW_PRIVATE", "true")]).await;
    let anna = env.user("anna", "utente").await;
    let blog = space(&env, anna, "guida", "wiki").await;
    let intro = page(&env, anna, "guida", "intro", json!({}), true).await;
    page(&env, anna, "guida", "install", json!({"parent_id": intro["id"]}), true).await;
    page(&env, anna, "guida", "bozza", json!({}), false).await;

    let (remote, remote_url) = bare_repo(true);
    let url = "/blogs/guida/git-sync";
    let (s, c) = env.put(url, Some(anna), json!({"remote_url": remote_url, "secret": "tok", "branch": "backup"})).await;
    assert_eq!(s, StatusCode::OK, "{c}");
    let id = config_id(&env, blog).await;

    // la richiesta di avvio accoda un job (l'esecuzione è del worker)
    let queued = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM noct_outbox WHERE subject = 'git_sync.run'")
            .fetch_one(&env.pool)
            .await
            .unwrap()
    };
    assert_eq!(env.post(&format!("{url}/run"), Some(anna), json!({})).await.0, StatusCode::ACCEPTED);
    assert_eq!(queued().await, 1);

    noct_git_sync::sync::run(&env.state, id, false).await.unwrap();
    let (_, c) = env.get(url, Some(anna)).await;
    assert_eq!(
        (c["last_status"].clone(), c["last_pages"].clone(), c["last_error"].clone()),
        (json!("ok"), json!(2), Value::Null),
        "{c}"
    );
    let commit = c["last_commit"].as_str().unwrap().to_string();

    // il branch contiene l'albero (senza la bozza) e il marcatore; il branch predefinito non è stato toccato
    let files = git(&remote, &["ls-tree", "-r", "--name-only", "backup"]);
    assert_eq!(files.lines().collect::<Vec<_>>(), [".notturni.json", "it/intro/index.md", "it/intro/install.md"]);
    assert_eq!(git(&remote, &["rev-parse", "backup"]).trim(), commit);
    assert_eq!(git(&remote, &["rev-list", "--count", "main"]).trim(), "1", "main non è stato toccato");
    let page = git(&remote, &["show", "backup:it/intro/install.md"]);
    assert!(page.starts_with("---\ntitle: \"Pagina install\"\n") && page.contains("parent: \"intro\"\n"), "{page}");
    assert!(page.ends_with("---\n\ntesto di install\n"), "{page}");
    assert!(!page.contains("tok"), "il token non finisce nei file");

    // niente di nuovo: nessun accesso alla rete, nessun nuovo commit
    noct_git_sync::sync::run(&env.state, id, false).await.unwrap();
    let (_, c) = env.get(url, Some(anna)).await;
    assert_eq!((c["last_status"].clone(), c["last_commit"].clone()), (json!("unchanged"), json!(commit)));
    // forzando, il repository è già allineato: ancora nessun commit
    noct_git_sync::sync::run(&env.state, id, true).await.unwrap();
    assert_eq!(git(&remote, &["rev-list", "--count", "backup"]).trim(), "1");

    // una modifica produce un secondo commit; una pagina eliminata sparisce dal branch
    let (s, _) = env
        .patch(&format!("/posts/{}", intro["id"].as_str().unwrap()), Some(anna), json!({"content": "nuovo testo"}))
        .await;
    assert_eq!(s, StatusCode::OK);
    noct_git_sync::sync::run(&env.state, id, false).await.unwrap();
    assert_eq!(git(&remote, &["rev-list", "--count", "backup"]).trim(), "2");
    assert!(git(&remote, &["show", "backup:it/intro/index.md"]).contains("nuovo testo"));

    // le bozze entrano solo se richiesto
    let (s, _) = env.put(url, Some(anna), json!({"remote_url": remote_url, "include_drafts": true})).await;
    assert_eq!(s, StatusCode::OK);
    noct_git_sync::sync::run(&env.state, id, false).await.unwrap();
    let (_, c) = env.get(url, Some(anna)).await;
    assert_eq!((c["last_status"].clone(), c["last_error"].clone()), (json!("ok"), Value::Null), "{c}");
    let tree = git(&remote, &["ls-tree", "-r", "--name-only", "backup"]);
    assert!(tree.contains("it/bozza.md"), "{tree}");

    let audited: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'git_sync.run'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(audited, 5);
    let _ = std::fs::remove_dir_all(remote);
    env.teardown().await;
}

#[tokio::test]
async fn unsafe_targets_are_refused_and_recorded() {
    let env = setup(&["git_sync"], &[("NOCT_GIT_SYNC_ALLOW_PRIVATE", "true")]).await;
    let anna = env.user("anna", "utente").await;
    let blog = space(&env, anna, "diario", "blog").await;
    page(&env, anna, "diario", "primo", json!({}), true).await;
    let url = "/blogs/diario/git-sync";

    // un repository vuoto non si usa: il suo branch predefinito non è determinabile
    let (empty, empty_url) = bare_repo(false);
    env.put(url, Some(anna), json!({"remote_url": empty_url, "secret": "tok", "branch": "backup"})).await;
    let id = config_id(&env, blog).await;
    noct_git_sync::sync::run(&env.state, id, false).await.unwrap();
    let (_, c) = env.get(url, Some(anna)).await;
    assert!(c["last_error"].as_str().unwrap().contains("vuoto"), "{c}");
    let _ = std::fs::remove_dir_all(empty);

    let (remote, remote_url) = bare_repo(true);

    // il branch predefinito del repository non si tocca
    env.put(url, Some(anna), json!({"remote_url": remote_url, "branch": "main"})).await;
    noct_git_sync::sync::run(&env.state, id, false).await.unwrap();
    let (_, c) = env.get(url, Some(anna)).await;
    assert_eq!(c["last_status"], "error");
    assert!(c["last_error"].as_str().unwrap().contains("predefinito"), "{c}");
    assert_eq!(git(&remote, &["rev-list", "--count", "main"]).trim(), "1", "nulla è stato scritto");

    // un branch esistente non creato da Notturni non si sovrascrive
    let work = std::env::temp_dir().join(format!("noct-gs-work-{}", Uuid::new_v4().simple()));
    std::fs::create_dir_all(&work).unwrap();
    git(&work, &["init", "-q", "-b", "altrui"]);
    std::fs::write(work.join("codice.rs"), "fn main() {}").unwrap();
    git(&work, &["add", "-A"]);
    git(&work, &["-c", "user.name=x", "-c", "user.email=x@example.com", "commit", "-q", "-m", "x"]);
    git(&work, &["push", "-q", &remote_url, "altrui:altrui"]);
    env.put(url, Some(anna), json!({"remote_url": remote_url, "branch": "altrui"})).await;
    noct_git_sync::sync::run(&env.state, id, false).await.unwrap();
    let (_, c) = env.get(url, Some(anna)).await;
    assert!(c["last_error"].as_str().unwrap().contains("non è stato creato da Notturni"), "{c}");
    assert_eq!(git(&remote, &["ls-tree", "-r", "--name-only", "altrui"]).trim(), "codice.rs");

    // disattivata: nessun giro
    env.put(url, Some(anna), json!({"remote_url": remote_url, "branch": "backup", "enabled": false})).await;
    assert_eq!(env.post(&format!("{url}/run"), Some(anna), json!({})).await.0, StatusCode::BAD_REQUEST);
    noct_git_sync::sync::run(&env.state, id, false).await.unwrap();
    assert!(git(&remote, &["branch", "--list", "backup"]).trim().is_empty());
    let _ = std::fs::remove_dir_all(remote);
    let _ = std::fs::remove_dir_all(work);
    env.teardown().await;
}

#[tokio::test]
async fn ssh_mode_generates_a_deploy_key_pins_the_host_key_and_pushes_over_ssh() {
    let (ssh, keyscan, ssh_log, scan_log, host_line) = fake_ssh_tools();
    let (ssh_s, scan_s) = (ssh.display().to_string(), keyscan.display().to_string());
    let env = setup(
        &["git_sync"],
        &[
            ("NOCT_GIT_SYNC_ALLOW_PRIVATE", "true"),
            ("NOCT_GIT_SYNC_SSH_BIN", &ssh_s),
            ("NOCT_GIT_SYNC_KEYSCAN_BIN", &scan_s),
        ],
    )
    .await;
    let anna = env.user("anna", "utente").await;
    let blog = space(&env, anna, "guida", "blog").await;
    page(&env, anna, "guida", "primo", json!({}), true).await;
    let (remote, _) = bare_repo(true);
    let ssh_url = format!("ssh://git@127.0.0.1{}", remote.display());
    let url = "/blogs/guida/git-sync";
    let scans = || std::fs::read_to_string(&scan_log).map(|t| t.lines().count()).unwrap_or(0);

    // il token non c'entra con SSH; l'impronta attesa deve corrispondere a quella ricevuta
    let (s, b) =
        env.put(url, Some(anna), json!({"remote_url": ssh_url, "auth_method": "ssh", "secret": "ghp_x"})).await;
    assert_eq!((s, detail(&b).contains("genera la piattaforma")), (StatusCode::BAD_REQUEST, true), "{b}");
    let (s, b) = env
        .put(
            url,
            Some(anna),
            json!({"remote_url": ssh_url, "auth_method": "ssh", "host_fingerprint": "SHA256:sbagliata"}),
        )
        .await;
    assert_eq!((s, detail(&b).contains("non corrisponde")), (StatusCode::BAD_REQUEST, true), "{b}");
    let (s, _) =
        env.put(url, Some(anna), json!({"remote_url": "https://github.com/a/b.git", "auth_method": "ssh"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "un indirizzo https non è un indirizzo SSH");
    assert_eq!(env.get(url, Some(anna)).await.1["configured"], false, "nulla è stato salvato");
    let base = scans();

    let (s, c) = env
        .put(
            url,
            Some(anna),
            json!({"remote_url": ssh_url, "auth_method": "ssh", "branch": "backup", "username": "git"}),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{c}");
    let public = c["ssh_public_key"].as_str().unwrap().to_string();
    assert!(public.starts_with("ssh-ed25519 ") && public.ends_with(" notturni-guida"), "{public}");
    assert_eq!((c["auth_method"].clone(), c["has_secret"].clone()), (json!("ssh"), json!(true)));
    let prints = c["ssh_host_fingerprints"].as_array().unwrap();
    assert_eq!(prints.len(), 1);
    let fingerprint = prints[0].as_str().unwrap().to_string();
    assert!(fingerprint.starts_with("SHA256:"));
    assert!(!c.to_string().contains("PRIVATE KEY"), "la chiave privata non esce mai");
    let stored: String = sqlx::query_scalar("SELECT secret FROM git_sync_configs").fetch_one(&env.pool).await.unwrap();
    assert!(stored.starts_with("enc:v1:") && !stored.contains("OPENSSH"), "la chiave privata è cifrata a riposo");
    assert_eq!(scans(), base + 1);

    // salvare di nuovo lo stesso host non rigenera la chiave né rilegge l'host; con l'impronta giusta è accettato
    let (s, c2) = env.put(url, Some(anna), json!({"remote_url": ssh_url, "include_drafts": false})).await;
    assert_eq!((s, c2["ssh_public_key"].as_str()), (StatusCode::OK, Some(public.as_str())), "{c2}");
    assert_eq!(scans(), base + 1, "host invariato: nessuna nuova scansione");
    let (s, _) = env.put(url, Some(anna), json!({"remote_url": ssh_url, "host_fingerprint": fingerprint})).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(scans(), base + 2, "un'impronta attesa forza la verifica");

    // il giro: chiave 0600 decifrata solo per il processo, host key pinnata, push normale sul branch dedicato
    let id = config_id(&env, blog).await;
    noct_git_sync::sync::run(&env.state, id, false).await.unwrap();
    let (_, c) = env.get(url, Some(anna)).await;
    assert_eq!((c["last_status"].clone(), c["last_error"].clone()), (json!("ok"), Value::Null), "{c}");
    assert_eq!(
        git(&remote, &["ls-tree", "-r", "--name-only", "backup"]).lines().collect::<Vec<_>>(),
        [".notturni.json", "it/primo.md"]
    );
    assert_eq!(git(&remote, &["rev-list", "--count", "main"]).trim(), "1", "main non è stato toccato");
    let log = std::fs::read_to_string(&ssh_log).unwrap();
    for needle in [
        "StrictHostKeyChecking=yes",
        "IdentitiesOnly=yes",
        "BatchMode=yes",
        "-F /dev/null",
        "KEYMODE: 600",
        "KEY: -----BEGIN OPENSSH PRIVATE KEY-----",
        &format!("KNOWN: {host_line}"),
    ] {
        assert!(log.contains(needle), "manca `{needle}` in:\n{log}");
    }
    assert!(!log.contains("accept-new") && !log.contains("StrictHostKeyChecking=no"));
    let key_path =
        log.split(" -i ").nth(1).and_then(|t| t.split_whitespace().next()).expect("percorso della chiave nel log");
    assert!(
        key_path.contains("noct-git-sync-") && !Path::new(key_path).exists(),
        "la chiave decifrata è stata rimossa: {key_path}"
    );

    // rigenerare cambia la chiave e fa rinviare; tornare a HTTPS chiede un token e dimentica la chiave
    let (s, c) = env.put(url, Some(anna), json!({"remote_url": ssh_url, "regenerate_key": true})).await;
    assert_eq!(s, StatusCode::OK);
    assert_ne!(c["ssh_public_key"].as_str().unwrap(), public);
    assert_eq!(c["last_status"], Value::Null, "esito azzerato: va rinviato");
    let (s, b) =
        env.put(url, Some(anna), json!({"remote_url": "https://8.8.8.8/a/b.git", "auth_method": "https"})).await;
    assert_eq!((s, detail(&b).contains("obbligatorio")), (StatusCode::BAD_REQUEST, true), "{b}");
    let (s, c) = env
        .put(
            url,
            Some(anna),
            json!({"remote_url": "https://8.8.8.8/a/b.git", "auth_method": "https", "secret": "ghp_x"}),
        )
        .await;
    assert_eq!(
        (s, c["ssh_public_key"].clone(), c["ssh_host_fingerprints"].clone()),
        (StatusCode::OK, Value::Null, json!([])),
        "{c}"
    );
    let _ = std::fs::remove_dir_all(remote);
    env.teardown().await;
}

#[tokio::test]
async fn ssh_remotes_are_normalized_and_blocked_on_private_hosts_in_production() {
    let (ssh, keyscan, _, _, _) = fake_ssh_tools();
    let (ssh_s, scan_s) = (ssh.display().to_string(), keyscan.display().to_string());
    let env = setup(&["git_sync"], &[("NOCT_GIT_SYNC_SSH_BIN", &ssh_s), ("NOCT_GIT_SYNC_KEYSCAN_BIN", &scan_s)]).await;
    let anna = env.user("anna", "utente").await;
    space(&env, anna, "guida", "blog").await;
    let url = "/blogs/guida/git-sync";
    for bad in [
        "git@127.0.0.1:a/b.git",
        "ssh://git@localhost/a/b.git",
        "ssh://git@169.254.169.254/a/b.git",
        "ssh://git@[::1]/a/b",
    ] {
        let (s, b) = env.put(url, Some(anna), json!({"remote_url": bad, "auth_method": "ssh"})).await;
        assert_eq!((s, detail(&b).contains("raggiungibile")), (StatusCode::BAD_REQUEST, true), "{bad}: {b}");
    }
    let (s, c) =
        env.put(url, Some(anna), json!({"remote_url": "git@8.8.8.8:anna/testi.git", "auth_method": "ssh"})).await;
    assert_eq!((s, c["remote_url"].clone()), (StatusCode::OK, json!("ssh://git@8.8.8.8/anna/testi.git")), "{c}");
    env.teardown().await;
}
