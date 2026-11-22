//! Esecuzione del binario `git` in un ambiente isolato: niente configurazione di sistema o utente,
//! nessun prompt, nessun hook, solo i protocolli ammessi, nessun redirect seguito. Il token viaggia
//! in una variabile d'ambiente del processo figlio (`http.extraHeader`), mai negli argomenti.

use base64::{Engine, engine::general_purpose::STANDARD};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::process::Command;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_ERROR_CHARS: usize = 600;

/// Come git si autentica presso il remoto.
pub enum GitAuth<'a> {
    /// `(utente, token)` via HTTPS.
    Https { username: &'a str, token: &'a str },
    /// Chiave privata e `known_hosts` (già scritti in file riservati) più il client SSH da usare.
    Ssh { ssh_bin: &'a str, key: &'a Path, known_hosts: &'a Path },
}

/// Comando che git usa come client SSH: solo la chiave indicata, nessun agente né configurazione
/// dell'utente, nessuna password, e soprattutto host key **verificata** contro il file pinnato
/// (`StrictHostKeyChecking=yes`: una chiave diversa o sconosciuta fa fallire la connessione).
pub fn ssh_command(ssh_bin: &str, key: &Path, known_hosts: &Path) -> String {
    let q = |p: &str| format!("\'{}\'", p.replace('\'', "'\\''"));
    format!(
        "{} -F /dev/null -i {} -o IdentitiesOnly=yes -o IdentityAgent=none -o BatchMode=yes -o PasswordAuthentication=no \
         -o KbdInteractiveAuthentication=no -o StrictHostKeyChecking=yes -o UserKnownHostsFile={} -o GlobalKnownHostsFile=/dev/null \
         -o ForwardAgent=no -o ForwardX11=no -o ConnectTimeout=15",
        q(ssh_bin),
        q(&key.to_string_lossy()),
        q(&known_hosts.to_string_lossy()),
    )
}

pub struct Git {
    bin: String,
    repo: PathBuf,
    home: PathBuf,
    env: Vec<(String, String)>,
    secrets: Vec<String>,
}

impl Git {
    /// `repo` e `home` devono già esistere. `allow_file`: consente anche il protocollo `file` (solo test).
    pub fn new(bin: &str, repo: &Path, home: &Path, auth: &GitAuth, allow_file: bool) -> Git {
        let mut config: Vec<(&str, String)> = vec![
            ("http.followRedirects", "false".into()),
            ("core.hooksPath", "/dev/null".into()),
            ("credential.helper", String::new()),
            ("protocol.allow", "never".into()),
        ];
        let mut allowed: Vec<&str> = Vec::new();
        let mut secrets = Vec::new();
        let mut env: Vec<(String, String)> = Vec::new();
        match auth {
            GitAuth::Https { username, token } => {
                allowed.push("https");
                config.push(("protocol.https.allow", "always".into()));
                let basic = STANDARD.encode(format!("{username}:{token}"));
                config.push(("http.extraHeader", format!("Authorization: Basic {basic}")));
                secrets.extend([token.to_string(), basic]);
            }
            GitAuth::Ssh { ssh_bin, key, known_hosts } => {
                allowed.push("ssh");
                config.push(("protocol.ssh.allow", "always".into()));
                env.push(("GIT_SSH_COMMAND".into(), ssh_command(ssh_bin, key, known_hosts)));
                env.push(("GIT_SSH_VARIANT".into(), "ssh".into()));
            }
        }
        if allow_file {
            allowed.push("file");
            config.push(("protocol.file.allow", "always".into()));
        }
        env.extend([
            ("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()),
            ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
            ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
            ("GIT_ALLOW_PROTOCOL".into(), allowed.join(":")),
            ("LC_ALL".into(), "C".into()),
            ("GIT_CONFIG_COUNT".into(), config.len().to_string()),
        ]);
        for (i, (k, v)) in config.into_iter().enumerate() {
            env.push((format!("GIT_CONFIG_KEY_{i}"), k.to_string()));
            env.push((format!("GIT_CONFIG_VALUE_{i}"), v));
        }
        Git { bin: bin.to_string(), repo: repo.to_path_buf(), home: home.to_path_buf(), env, secrets }
    }

    fn redact(&self, text: &str) -> String {
        let mut out = text.to_string();
        for s in self.secrets.iter().filter(|s| !s.is_empty()) {
            out = out.replace(s.as_str(), "***");
        }
        out.chars().take(MAX_ERROR_CHARS).collect()
    }

    /// Esegue `git <args>` nel repository; restituisce stdout. L'errore riporta uno stderr ripulito dei segreti.
    pub async fn run(&self, args: &[&str]) -> Result<String, String> {
        let mut cmd = Command::new(&self.bin);
        cmd.args(args)
            .current_dir(&self.repo)
            .env_clear()
            .env("HOME", &self.home)
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(path) = std::env::var_os("PATH") {
            cmd.env("PATH", path);
        }
        let out = tokio::time::timeout(COMMAND_TIMEOUT, cmd.output())
            .await
            .map_err(|_| format!("git {}: tempo scaduto", args.first().unwrap_or(&"")))?
            .map_err(|e| format!("impossibile eseguire git: {e}"))?;
        if out.status.success() {
            return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(format!("git {} non riuscito: {}", args.first().unwrap_or(&""), self.redact(stderr.trim())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_client_is_locked_to_the_pinned_key_and_host() {
        let cmd = ssh_command("ssh", Path::new("/tmp/x/key"), Path::new("/tmp/x/known"));
        for needle in [
            "-F /dev/null",
            "-i '/tmp/x/key'",
            "IdentitiesOnly=yes",
            "IdentityAgent=none",
            "BatchMode=yes",
            "PasswordAuthentication=no",
            "StrictHostKeyChecking=yes",
            "UserKnownHostsFile='/tmp/x/known'",
            "GlobalKnownHostsFile=/dev/null",
            "ForwardAgent=no",
        ] {
            assert!(cmd.contains(needle), "manca {needle}: {cmd}");
        }
        assert!(!cmd.contains("accept-new") && !cmd.contains("StrictHostKeyChecking=no"));
        let quoted = ssh_command("ssh", Path::new("/tmp/it's"), Path::new("/k"));
        assert!(quoted.contains(r"'/tmp/it'\''s'"), "{quoted}");
    }
}
