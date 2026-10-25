//! Strumenti OpenSSH per la modalità SSH: generazione della chiave di deploy, rilevamento e impronte della
//! chiave dell'host. Tutto passa da binari esterni (`ssh-keygen`, `ssh-keyscan`) in un ambiente ripulito.

use crate::scratch::Scratch;
use std::{process::Stdio, time::Duration};
use tokio::process::Command;

const TOOL_TIMEOUT: Duration = Duration::from_secs(25);

pub struct KeyPair {
    pub private: String,
    pub public: String,
}

async fn run_tool(bin: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new(bin);
    cmd.args(args).env_clear().stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    if let Some(path) = std::env::var_os("PATH") {
        cmd.env("PATH", path);
    }
    let out = tokio::time::timeout(TOOL_TIMEOUT, cmd.output())
        .await
        .map_err(|_| format!("{bin}: tempo scaduto"))?
        .map_err(|e| format!("impossibile eseguire {bin}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{bin} non riuscito: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Nuova coppia ed25519 senza passphrase (la privata viene poi cifrata a riposo). `comment` finisce nella
/// chiave pubblica e aiuta a riconoscerla nel pannello del provider.
pub async fn generate_keypair(comment: &str) -> Result<KeyPair, String> {
    let scratch = Scratch::create().map_err(|e| format!("directory temporanea non creata: {e}"))?;
    let key = scratch.root().join("key");
    let key_str = key.to_string_lossy().into_owned();
    run_tool("ssh-keygen", &["-q", "-t", "ed25519", "-N", "", "-C", comment, "-f", &key_str]).await?;
    let private = std::fs::read_to_string(&key).map_err(|e| format!("chiave non leggibile: {e}"))?;
    let public = std::fs::read_to_string(format!("{key_str}.pub")).map_err(|e| format!("chiave non leggibile: {e}"))?;
    Ok(KeyPair { private, public: public.trim().to_string() })
}

/// Le righe `known_hosts` valide in un testo: tre campi, tipo di chiave riconosciuto, niente commenti.
pub fn parse_known_hosts(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            f.len() >= 3 && (f[1].starts_with("ssh-") || f[1].starts_with("ecdsa-")) && !f[0].starts_with('|')
        })
        .map(str::to_string)
        .collect()
}

/// Chiavi pubbliche dell'host, così come le presenta in questo momento (da fissare alla configurazione).
pub async fn scan_host(keyscan_bin: &str, host: &str, port: u16) -> Result<String, String> {
    let port_s = port.to_string();
    let out = run_tool(keyscan_bin, &["-T", "10", "-t", "ed25519,ecdsa,rsa", "-p", &port_s, "--", host]).await?;
    let lines = parse_known_hosts(&out);
    if lines.is_empty() {
        return Err(format!(
            "Nessuna chiave SSH ricevuta da {host}:{port}: l'host non è raggiungibile o non parla SSH."
        ));
    }
    Ok(lines.join("\n") + "\n")
}

/// Impronte `SHA256:...` delle chiavi di un testo `known_hosts` (o di una chiave pubblica).
pub async fn fingerprints(known_hosts: &str) -> Result<Vec<String>, String> {
    let scratch = Scratch::create().map_err(|e| format!("directory temporanea non creata: {e}"))?;
    let file = scratch.write_private("keys", known_hosts).map_err(|e| format!("scrittura non riuscita: {e}"))?;
    let out = run_tool("ssh-keygen", &["-l", "-E", "sha256", "-f", &file.to_string_lossy()]).await?;
    Ok(out
        .lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .filter(|f| f.starts_with("SHA256:"))
        .map(str::to_string)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_wellformed_host_keys_are_kept() {
        let text = "# github.com:22 SSH-2.0-x\ngithub.com ssh-ed25519 AAAAC3Nza\n\n[h]:2222 ecdsa-sha2-nistp256 AAAA\n|1|hashed ssh-rsa AAAA\nbad line\nhost ssh-dss\n";
        assert_eq!(parse_known_hosts(text), ["github.com ssh-ed25519 AAAAC3Nza", "[h]:2222 ecdsa-sha2-nistp256 AAAA"]);
    }

    #[tokio::test]
    async fn generated_keys_are_ed25519_with_a_stable_fingerprint() {
        let pair = generate_keypair("notturni-test").await.unwrap();
        assert!(pair.private.starts_with("-----BEGIN OPENSSH PRIVATE KEY-----"));
        assert!(pair.public.starts_with("ssh-ed25519 ") && pair.public.ends_with(" notturni-test"));
        let fp = fingerprints(&format!("example.com {}", pair.public.rsplit_once(' ').unwrap().0)).await.unwrap();
        assert_eq!(fp.len(), 1);
        assert!(fp[0].starts_with("SHA256:"));
        let other = generate_keypair("x").await.unwrap();
        assert_ne!(pair.private, other.private, "ogni chiave è nuova");
    }
}
