//! Chiamate HTTP in uscita verso servizi ausiliari. Entrambe sono *fail open*: il
//! database resta la fonte di verità, un servizio ausiliario spento non deve mai
//! far fallire la richiesta che ha già modificato i dati.
//!
//! * [`Revalidator`]: invalida la cache dei Server Component del frontend.
//! * [`Moderation`]: classificazione automatica delle immagini (servizio `moderation/`).

use bytes::Bytes;
use std::time::Duration;

const REVALIDATE_TIMEOUT: Duration = Duration::from_secs(2);
const CLASSIFY_TIMEOUT: Duration = Duration::from_secs(15);

fn client(timeout: Duration) -> reqwest::Client {
    // la costruzione fallisce solo se manca un backend TLS: in tal caso nessuna chiamata
    // parte (e viene loggato), non si interrompe l'avvio
    reqwest::Client::builder().timeout(timeout).build().unwrap_or_default()
}

/// Invalidazione on-demand della cache del frontend (Next.js): `POST url {"tags": [...]}`
/// con `Authorization: Bearer <secret>`. Le stringhe dei tag devono restare identiche
/// a quelle di `frontend/src/lib/revalidate.ts`.
#[derive(Clone)]
pub struct Revalidator {
    client: reqwest::Client,
    target: Option<(String, String)>,
}

impl Revalidator {
    pub fn new(url: Option<String>, secret: Option<String>) -> Self {
        let target = match (url, secret) {
            (Some(u), Some(s)) if !u.is_empty() && !s.is_empty() => Some((u, s)),
            _ => None,
        };
        Self { client: client(REVALIDATE_TIMEOUT), target }
    }

    pub fn disabled() -> Self {
        Self::new(None, None)
    }

    pub fn is_enabled(&self) -> bool {
        self.target.is_some()
    }

    /// Invia i tag (deduplicati, senza vuoti) e attende l'esito; non fallisce mai.
    pub async fn tags(&self, tags: &[String]) {
        let Some((url, secret)) = &self.target else { return };
        let mut seen: Vec<&str> = Vec::new();
        for t in tags {
            if !t.is_empty() && !seen.contains(&t.as_str()) {
                seen.push(t);
            }
        }
        if seen.is_empty() {
            return;
        }
        match self.client.post(url).bearer_auth(secret).json(&serde_json::json!({ "tags": seen })).send().await {
            Ok(res) if res.status().is_client_error() || res.status().is_server_error() => {
                tracing::warn!(status = %res.status(), ?seen, "revalidate del frontend rifiutato");
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, ?seen, "revalidate del frontend non riuscito"),
        }
    }

    /// Come [`Revalidator::tags`] ma in background: la richiesta HTTP non aspetta il frontend.
    pub fn spawn(&self, tags: Vec<String>) {
        if self.target.is_none() {
            return;
        }
        let me = self.clone();
        tokio::spawn(async move { me.tags(&tags).await });
    }
}

/// Client del servizio di moderazione immagini (`POST {url}/classify`, multipart).
#[derive(Clone)]
pub struct Moderation {
    client: reqwest::Client,
    url: Option<String>,
}

impl Moderation {
    pub fn new(url: Option<String>) -> Self {
        Self {
            client: client(CLASSIFY_TIMEOUT),
            url: url.map(|u| u.trim_end_matches('/').to_string()).filter(|u| !u.is_empty()),
        }
    }

    pub fn disabled() -> Self {
        Self::new(None)
    }

    /// `true` se l'immagine è segnalata come possibile contenuto sensibile. Servizio non
    /// configurato, irraggiungibile o in errore → `false` (mai un blocco).
    pub async fn classify(&self, data: Bytes, filename: &str, content_type: &str, threshold: Option<f64>) -> bool {
        let Some(url) = &self.url else { return false };
        let part = match reqwest::multipart::Part::stream(data)
            .file_name(filename.to_string())
            .mime_str(if content_type.is_empty() { "application/octet-stream" } else { content_type })
        {
            Ok(p) => p,
            Err(_) => return false,
        };
        let mut form = reqwest::multipart::Form::new().part("file", part);
        if let Some(t) = threshold {
            form = form.text("threshold", t.to_string());
        }
        let res = match self.client.post(format!("{url}/classify")).multipart(form).send().await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                tracing::warn!(status = %r.status(), "servizio di moderazione in errore: nessun blocco");
                return false;
            }
            Err(e) => {
                tracing::warn!(error = %e, "servizio di moderazione non raggiungibile: nessun blocco");
                return false;
            }
        };
        match res.json::<serde_json::Value>().await {
            Ok(v) => v.get("is_sensitive").and_then(|b| b.as_bool()).unwrap_or(false),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    /// Server HTTP minimale: risponde una volta con `status`/`body` e restituisce la richiesta ricevuta.
    async fn one_shot(status: &'static str, body: &'static str) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 16 * 1024];
            let mut got = Vec::new();
            // legge finché gli header e il corpo dichiarato sono arrivati
            loop {
                let n = sock.read(&mut buf).await.unwrap();
                got.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&got).to_string();
                if let Some(pos) = text.find("\r\n\r\n") {
                    let len = text[..pos]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if got.len() >= pos + 4 + len || n == 0 {
                        break;
                    }
                } else if n == 0 {
                    break;
                }
            }
            let reply = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(reply.as_bytes()).await.unwrap();
            String::from_utf8_lossy(&got).to_string()
        });
        (format!("http://{addr}"), handle)
    }

    #[tokio::test]
    async fn revalidate_posts_deduplicated_tags_with_the_secret() {
        let (url, req) = one_shot("200 OK", "{}").await;
        let r = Revalidator::new(Some(format!("{url}/revalidate")), Some("s3cret".into()));
        r.tags(&["feed".into(), "blog:a".into(), "feed".into(), String::new()]).await;
        let req = req.await.unwrap();
        assert!(req.starts_with("POST /revalidate"), "{req}");
        assert!(req.to_ascii_lowercase().contains("authorization: bearer s3cret"));
        assert!(req.contains(r#"{"tags":["feed","blog:a"]}"#), "{req}");
    }

    #[tokio::test]
    async fn revalidate_never_fails() {
        // niente in ascolto: l'errore è solo loggato
        Revalidator::new(Some("http://127.0.0.1:1/x".into()), Some("s".into())).tags(&["a".into()]).await;
        assert!(!Revalidator::disabled().is_enabled());
        Revalidator::disabled().tags(&["a".into()]).await;
    }

    #[tokio::test]
    async fn moderation_reads_the_verdict_and_fails_open() {
        let (url, req) = one_shot("200 OK", r#"{"is_sensitive": true}"#).await;
        let m = Moderation::new(Some(url));
        assert!(m.classify(Bytes::from_static(b"\x89PNG"), "a.png", "image/png", Some(0.8)).await);
        let req = req.await.unwrap();
        assert!(req.starts_with("POST /classify"), "{req}");
        assert!(req.contains("name=\"threshold\""));

        let (url, _) = one_shot("500 Internal Server Error", "{}").await;
        assert!(!Moderation::new(Some(url)).classify(Bytes::new(), "a.png", "image/png", None).await);
        assert!(!Moderation::new(Some("http://127.0.0.1:1".into())).classify(Bytes::new(), "a", "", None).await);
        assert!(!Moderation::disabled().classify(Bytes::new(), "a", "", None).await);
    }
}
