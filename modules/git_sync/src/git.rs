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

pub struct Git {
    bin: String,
    repo: PathBuf,
    home: PathBuf,
    env: Vec<(String, String)>,
    secrets: Vec<String>,
}

impl Git {
    /// `repo` e `home` devono già esistere. `credentials` è `(utente, token)`.
    pub fn new(bin: &str, repo: &Path, home: &Path, credentials: Option<(&str, &str)>, allow_file: bool) -> Git {
        let mut config: Vec<(&str, String)> = vec![
            ("http.followRedirects", "false".into()),
            ("core.hooksPath", "/dev/null".into()),
            ("credential.helper", String::new()),
            ("protocol.allow", "never".into()),
            ("protocol.https.allow", "always".into()),
        ];
        if allow_file {
            config.push(("protocol.file.allow", "always".into()));
        }
        let mut secrets = Vec::new();
        if let Some((user, token)) = credentials {
            let basic = STANDARD.encode(format!("{user}:{token}"));
            config.push(("http.extraHeader", format!("Authorization: Basic {basic}")));
            secrets.extend([token.to_string(), basic]);
        }
        let mut env: Vec<(String, String)> = vec![
            ("GIT_TERMINAL_PROMPT".into(), "0".into()),
            ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
            ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
            ("GIT_ALLOW_PROTOCOL".into(), if allow_file { "https:file".into() } else { "https".into() }),
            ("LC_ALL".into(), "C".into()),
            ("GIT_CONFIG_COUNT".into(), config.len().to_string()),
        ];
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
