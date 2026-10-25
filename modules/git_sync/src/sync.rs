//! Un giro di sincronizzazione: raccoglie i contenuti dello Spazio, ricostruisce in una directory
//! temporanea il branch di destinazione, e se qualcosa è cambiato lo committa e lo pubblica con un push
//! normale (mai forzato) su un branch che non sia quello predefinito del repository.

use crate::{
    git::{Git, GitAuth},
    rules,
    scratch::Scratch,
};
use chrono::{DateTime, Utc};
use noct_audit::{AuditActor, Entry};
use noct_core::{AppState, RequestMeta};
use noct_security::{net, secrets};
use serde_json::json;
use sqlx::FromRow;
use std::{net::SocketAddr, path::Path};
use uuid::Uuid;

/// Chiave dei segreti a riposo: la stessa degli altri (`NOCT_SECRETS_KEY`, dichiarata da `auth_mfa`),
/// altrimenti derivata dal segreto JWT.
pub fn secrets_key(st: &AppState) -> [u8; 32] {
    secrets::derive_key(st.module_config_str("auth_mfa", "secrets_key"), &st.config.jwt_secret)
}

pub fn allow_private(st: &AppState) -> bool {
    st.module_flag("git_sync", "allow_private_hosts").unwrap_or(false)
}

/// L'host dell'URL deve risolvere e tutti i suoi indirizzi essere pubblici (protezione SSRF). Non
/// serve per `file://`, ammesso solo con `allow_private`.
pub async fn check_host(url: &reqwest::Url, allow_private: bool) -> Result<(), String> {
    if allow_private || url.scheme() == "file" {
        return Ok(());
    }
    let unreachable = || "L'host del repository non è raggiungibile o non è pubblico.".to_string();
    let host = url.host_str().ok_or_else(unreachable)?;
    let port = if url.scheme() == "ssh" {
        url.port().unwrap_or(22)
    } else {
        url.port_or_known_default().ok_or_else(unreachable)?
    };
    let addrs: Vec<SocketAddr> = match host.trim_matches(['[', ']']).parse::<std::net::IpAddr>() {
        Ok(ip) => vec![SocketAddr::new(ip, port)],
        Err(_) => tokio::net::lookup_host((host, port)).await.map_err(|_| unreachable())?.collect(),
    };
    if addrs.is_empty() || addrs.iter().any(|a| net::is_blocked(a.ip())) {
        return Err(unreachable());
    }
    Ok(())
}

#[derive(FromRow)]
struct Cfg {
    id: Uuid,
    blog_id: Uuid,
    remote_url: String,
    branch: String,
    username: String,
    auth_method: String,
    secret: String,
    ssh_host_key: Option<String>,
    include_drafts: bool,
    last_hash: Option<String>,
    blog_slug: String,
    kind: String,
}

#[derive(FromRow)]
struct PostRow {
    id: Uuid,
    slug: String,
    locale: String,
    title: String,
    content: String,
    status: String,
    published_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
    author_display_name: String,
    tags: Vec<String>,
    parent_id: Option<Uuid>,
    position: i32,
}

enum Outcome {
    Unchanged,
    Pushed { commit: String },
}

async fn collect(st: &AppState, cfg: &Cfg) -> Result<Vec<rules::ExportPost>, sqlx::Error> {
    // l'albero esiste solo se il modulo `wiki` ha creato le sue tabelle
    let has_outline: bool =
        sqlx::query_scalar("SELECT to_regclass('post_outline') IS NOT NULL").fetch_one(&st.db).await?;
    let (join, cols) = if has_outline {
        ("LEFT JOIN post_outline o ON o.post_id = p.id", "o.parent_id, COALESCE(o.position, 0) AS position")
    } else {
        ("", "NULL::uuid AS parent_id, 0 AS position")
    };
    let rows: Vec<PostRow> = sqlx::query_as(&format!(
        "SELECT p.id, p.slug, p.locale, p.title, p.content, p.status::text AS status, p.published_at, p.updated_at,
                p.author_display_name,
                COALESCE((SELECT array_agg(t.name ORDER BY t.name) FROM post_tags pt JOIN tags t ON t.id = pt.tag_id
                          WHERE pt.post_id = p.id), '{{}}') AS tags,
                {cols}
         FROM posts p {join}
         WHERE p.blog_id = $1 AND NOT p.is_hidden
           AND ($2 OR (p.status = 'published' AND (p.published_at IS NULL OR p.published_at <= now())))
         ORDER BY p.created_at, p.id"
    ))
    .bind(cfg.blog_id)
    .bind(cfg.include_drafts)
    .fetch_all(&st.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| rules::ExportPost {
            id: r.id,
            slug: r.slug,
            locale: r.locale,
            title: r.title,
            content: r.content,
            status: r.status,
            published_at: r.published_at,
            updated_at: r.updated_at,
            author: r.author_display_name,
            tags: r.tags,
            parent_id: r.parent_id,
            position: r.position,
        })
        .collect())
}

fn write_tree(repo: &Path, files: &[(String, String)]) -> std::io::Result<()> {
    for entry in std::fs::read_dir(repo)? {
        let path = entry?.path();
        if path.file_name().is_some_and(|n| n == ".git") {
            continue;
        }
        if path.is_dir() { std::fs::remove_dir_all(&path)? } else { std::fs::remove_file(&path)? }
    }
    for (rel, content) in files {
        let path = repo.join(rel);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, content)?;
    }
    Ok(())
}

async fn publish(st: &AppState, cfg: &Cfg, files: &[(String, String)]) -> Result<Outcome, String> {
    let method = rules::validate_method(&cfg.auth_method)?;
    let url = match method {
        "ssh" => rules::validate_ssh_remote(&cfg.remote_url)?,
        _ => rules::validate_remote(&cfg.remote_url, allow_private(st))?,
    };
    check_host(&url, allow_private(st)).await?;
    let secret =
        secrets::open(&secrets_key(st), &cfg.secret).map_err(|_| "credenziale illeggibile: reimpostala".to_string())?;
    let branch = cfg.branch.as_str();
    rules::validate_branch(branch)?;

    let scratch = Scratch::create().map_err(|e| format!("directory temporanea non creata: {e}"))?;
    let bin = st.module_config_str("git_sync", "git_bin").unwrap_or("git");
    let ssh_bin = st.module_config_str("git_sync", "ssh_bin").unwrap_or("ssh");
    // la chiave privata decifrata vive solo in questa directory, che sparisce a fine giro
    let ssh_files = if method == "ssh" {
        let known = cfg.ssh_host_key.as_deref().filter(|k| !k.trim().is_empty());
        let known = known.ok_or("Nessuna host key SSH fissata: salva di nuovo la configurazione.")?;
        let key =
            scratch.write_private("id", &secret).map_err(|e| format!("scrittura della chiave non riuscita: {e}"))?;
        let known_hosts = scratch
            .write_private("known_hosts", known)
            .map_err(|e| format!("scrittura di known_hosts non riuscita: {e}"))?;
        Some((key, known_hosts))
    } else {
        None
    };
    let auth = match &ssh_files {
        Some((key, known_hosts)) => GitAuth::Ssh { ssh_bin, key, known_hosts },
        None => GitAuth::Https { username: &cfg.username, token: &secret },
    };
    let git = Git::new(bin, &scratch.repo(), &scratch.home(), &auth, allow_private(st));
    let remote = url.as_str();

    git.run(&["init", "-q", "--template="]).await?;
    git.run(&["remote", "add", "origin", remote]).await?;

    // ls-remote verifica raggiungibilità e credenziali. In un repository senza alcun riferimento il
    // branch predefinito non è determinabile (e il primo push lo diventerebbe su alcuni host): si rifiuta.
    if git.run(&["ls-remote", "origin"]).await?.trim().is_empty() {
        return Err("Il repository è vuoto: crea prima un commit iniziale sul suo branch predefinito.".into());
    }
    // il branch predefinito non si tocca mai
    let head = git.run(&["ls-remote", "--symref", "origin", "HEAD"]).await?;
    let default_branch = head
        .lines()
        .find_map(|l| l.strip_prefix("ref: refs/heads/"))
        .and_then(|l| l.split('\t').next())
        .map(str::to_string);
    if default_branch.as_deref() == Some(branch) {
        return Err(format!("`{branch}` è il branch predefinito del repository: scegline un altro."));
    }
    let reference = format!("refs/heads/{branch}");
    let exists = !git.run(&["ls-remote", "--heads", "origin", &reference]).await?.trim().is_empty();
    if exists {
        git.run(&["fetch", "-q", "--depth", "1", "origin", &reference]).await?;
        git.run(&["checkout", "-q", "-B", branch, "FETCH_HEAD"]).await?;
        if !scratch.repo().join(rules::MARKER_FILE).is_file() {
            return Err(format!(
                "Il branch `{branch}` esiste già e non è stato creato da Notturni: non lo sovrascrivo."
            ));
        }
    } else {
        git.run(&["checkout", "-q", "--orphan", branch]).await?;
    }

    let (repo, owned) = (scratch.repo(), files.to_vec());
    tokio::task::spawn_blocking(move || write_tree(&repo, &owned))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("scrittura dei file non riuscita: {e}"))?;
    git.run(&["add", "-A"]).await?;
    if git.run(&["status", "--porcelain"]).await?.trim().is_empty() {
        return Ok(Outcome::Unchanged);
    }
    let email = format!("noreply@{}", st.config.instance_fqdn);
    let message = format!(
        "Notturni: sincronizzazione di {} ({} file)\n\nGenerato automaticamente: le modifiche manuali a questo branch verranno sovrascritte.",
        cfg.blog_slug,
        files.len() - 1
    );
    git.run(&[
        "-c",
        "user.name=Notturni",
        "-c",
        &format!("user.email={email}"),
        "commit",
        "-q",
        "--no-gpg-sign",
        "-m",
        &message,
    ])
    .await?;
    git.run(&["push", "-q", "origin", &format!("{branch}:{reference}")]).await?;
    let commit = git.run(&["rev-parse", "HEAD"]).await?.trim().to_string();
    Ok(Outcome::Pushed { commit })
}

/// Esegue la sincronizzazione della configurazione `config_id` e ne registra l'esito. Un errore di
/// sincronizzazione (rete, credenziali, branch) non è un `Err`: finisce in `last_error`. `Err` solo per
/// problemi del database, che vale la pena riprovare. Con `force` si ignora l'impronta dell'ultimo esito.
pub async fn run(st: &AppState, config_id: Uuid, force: bool) -> Result<(), sqlx::Error> {
    // prenota la configurazione; un giro rimasto appeso oltre 15 minuti si considera morto
    let claimed: Option<Cfg> = sqlx::query_as(
        "WITH c AS (
           UPDATE git_sync_configs SET last_status = 'running', last_run_at = now()
           WHERE id = $1 AND enabled AND (last_status IS DISTINCT FROM 'running' OR last_run_at < now() - interval '15 minutes')
           RETURNING id, blog_id, remote_url, branch, username, auth_method, secret, ssh_host_key, include_drafts, last_hash)
         SELECT c.id, c.blog_id, c.remote_url, c.branch, c.username, c.auth_method, c.secret, c.ssh_host_key, c.include_drafts, c.last_hash,
                b.slug AS blog_slug, b.kind::text AS kind
         FROM c JOIN blogs b ON b.id = c.blog_id",
    )
    .bind(config_id)
    .fetch_optional(&st.db)
    .await?;
    let Some(cfg) = claimed else { return Ok(()) };

    let posts = collect(st, &cfg).await?;
    let files = rules::build_files(&cfg.kind, &cfg.blog_slug, &posts);
    let hash = rules::fingerprint(&files);
    let pages = posts.len();

    let result = if !force && cfg.last_hash.as_deref() == Some(hash.as_str()) {
        Ok(Outcome::Unchanged)
    } else {
        publish(st, &cfg, &files).await
    };
    let (status, error, commit) = match &result {
        Ok(Outcome::Unchanged) => ("unchanged", None, None),
        Ok(Outcome::Pushed { commit }) => ("ok", None, Some(commit.as_str())),
        Err(e) => ("error", Some(e.as_str()), None),
    };
    sqlx::query(
        "UPDATE git_sync_configs SET last_status = $2, last_error = $3, last_commit = COALESCE($4, last_commit),
                last_pages = $5, last_hash = CASE WHEN $2 = 'error' THEN last_hash ELSE $6 END, last_run_at = now()
         WHERE id = $1",
    )
    .bind(cfg.id)
    .bind(status)
    .bind(error)
    .bind(commit)
    .bind(pages as i32)
    .bind(&hash)
    .execute(&st.db)
    .await?;
    // l'audit non deve far fallire (né ripetere) un giro già concluso
    let mut tx = st.db.begin().await?;
    let audited = noct_audit::record(
        &mut tx,
        Entry {
            action: "git_sync.run",
            actor: AuditActor::System,
            target: Some(("blog", cfg.blog_id)),
            blog_id: Some(cfg.blog_id),
            meta: &RequestMeta::default(),
            payload: json!({"status": status, "pages": pages, "branch": cfg.branch, "commit": commit, "error": error}),
        },
    )
    .await;
    match audited {
        Ok(()) => tx.commit().await?,
        Err(e) => tracing::warn!(error = %e, "audit della sincronizzazione Git non scritto"),
    }
    if let Some(e) = error {
        tracing::warn!(space = %cfg.blog_slug, error = %e, "sincronizzazione Git non riuscita");
    }
    Ok(())
}
