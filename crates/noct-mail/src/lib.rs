//! Invio email. Il trasporto è dietro il trait [`Mailer`]; i moduli (OTP,
//! newsletter, reset password) non conoscono SMTP.

use async_trait::async_trait;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, MultiPart, SinglePart, header::ContentType},
    transport::smtp::authentication::Credentials,
};
use std::sync::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("SMTP non configurato (impostare NOCT_SMTP_HOST)")]
    NotConfigured,
    #[error("indirizzo non valido `{0}`")]
    InvalidAddress(String),
    #[error("messaggio non valido: {0}")]
    Build(String),
    #[error("invio fallito: {0}")]
    Transport(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email {
    pub to: Vec<String>,
    pub subject: String,
    pub text: String,
    pub html: Option<String>,
    /// Nome mostrato del mittente; l'indirizzo resta sempre quello configurato.
    pub from_name: Option<String>,
}

impl Email {
    pub fn text(to: impl Into<String>, subject: impl Into<String>, body: impl Into<String>) -> Self {
        Self { to: vec![to.into()], subject: subject.into(), text: body.into(), html: None, from_name: None }
    }

    /// Messaggio `multipart/alternative`: testo semplice di ripiego e versione HTML.
    pub fn alternative(
        to: impl Into<String>,
        subject: impl Into<String>,
        text: impl Into<String>,
        html: impl Into<String>,
    ) -> Self {
        Self { html: Some(html.into()), ..Self::text(to, subject, text) }
    }

    pub fn with_from_name(mut self, name: impl Into<String>) -> Self {
        self.from_name = Some(name.into()).filter(|n| !n.trim().is_empty());
        self
    }
}

#[async_trait]
pub trait Mailer: Send + Sync {
    async fn send(&self, email: &Email) -> Result<(), MailError>;
    /// Verifica che il trasporto sia raggiungibile (per `/health/ready`).
    async fn health(&self) -> Result<(), MailError> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
    pub password: Option<String>,
    /// `true`: STARTTLS (porta 587, provider reali). `false`: testo in chiaro
    /// (solo catcher di sviluppo come Mailpit/MailHog).
    pub use_tls: bool,
    pub from: String,
}

pub struct SmtpMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl SmtpMailer {
    pub fn new(cfg: &SmtpConfig) -> Result<Self, MailError> {
        let from: Mailbox = cfg.from.parse().map_err(|_| MailError::InvalidAddress(cfg.from.clone()))?;
        let builder = if cfg.use_tls {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host)
                .map_err(|e| MailError::Transport(e.to_string()))?
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.host)
        };
        let mut builder = builder.port(cfg.port);
        if let (Some(user), Some(password)) = (&cfg.user, &cfg.password) {
            builder = builder.credentials(Credentials::new(user.clone(), password.clone()));
        }
        Ok(Self { transport: builder.build(), from })
    }
}

fn build_message(from: &Mailbox, email: &Email) -> Result<Message, MailError> {
    if email.to.is_empty() {
        return Err(MailError::Build("nessun destinatario".into()));
    }
    // il nome del mittente è testo libero dell'utente: lo si ripulisce da ritorni a capo e virgolette
    let from = match email.from_name.as_deref() {
        Some(name) => {
            let clean: String =
                name.chars().filter(|c| !c.is_control() && *c != '"' && *c != '<' && *c != '>').collect();
            Mailbox::new(Some(clean.trim().to_string()).filter(|n| !n.is_empty()), from.email.clone())
        }
        None => from.clone(),
    };
    let mut b = Message::builder().from(from).subject(email.subject.clone());
    for to in &email.to {
        b = b.to(to.parse().map_err(|_| MailError::InvalidAddress(to.clone()))?);
    }
    let msg = match &email.html {
        Some(html) => b.multipart(
            MultiPart::alternative()
                .singlepart(SinglePart::builder().header(ContentType::TEXT_PLAIN).body(email.text.clone()))
                .singlepart(SinglePart::builder().header(ContentType::TEXT_HTML).body(html.clone())),
        ),
        None => b.header(ContentType::TEXT_PLAIN).body(email.text.clone()),
    };
    msg.map_err(|e| MailError::Build(e.to_string()))
}

#[async_trait]
impl Mailer for SmtpMailer {
    async fn send(&self, email: &Email) -> Result<(), MailError> {
        let msg = build_message(&self.from, email)?;
        self.transport.send(msg).await.map(|_| ()).map_err(|e| MailError::Transport(e.to_string()))
    }

    async fn health(&self) -> Result<(), MailError> {
        match self.transport.test_connection().await {
            Ok(true) => Ok(()),
            Ok(false) => Err(MailError::Transport("connessione SMTP rifiutata".into())),
            Err(e) => Err(MailError::Transport(e.to_string())),
        }
    }
}

/// Nessun SMTP configurato: ogni invio fallisce in modo esplicito (mai una
/// perdita silenziosa di OTP o newsletter).
pub struct NotConfiguredMailer;

#[async_trait]
impl Mailer for NotConfiguredMailer {
    async fn send(&self, _email: &Email) -> Result<(), MailError> {
        Err(MailError::NotConfigured)
    }
}

/// Raccoglie i messaggi in memoria (test e sviluppo).
#[derive(Default)]
pub struct MemoryMailer {
    sent: Mutex<Vec<Email>>,
}

impl MemoryMailer {
    pub fn sent(&self) -> Vec<Email> {
        self.sent.lock().expect("lock avvelenato").clone()
    }
}

#[async_trait]
impl Mailer for MemoryMailer {
    async fn send(&self, email: &Email) -> Result<(), MailError> {
        build_message(&"test@example.com".parse().expect("indirizzo statico"), email)?;
        self.sent.lock().expect("lock avvelenato").push(email.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::TcpListener,
    };

    /// Server SMTP minimale: accetta una sessione e restituisce i dati ricevuti.
    async fn fake_smtp() -> (u16, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let (r, mut w) = sock.into_split();
            let mut r = BufReader::new(r);
            w.write_all(b"220 fake ESMTP\r\n").await.unwrap();
            let mut transcript = String::new();
            let mut in_data = false;
            let mut line = String::new();
            loop {
                line.clear();
                if r.read_line(&mut line).await.unwrap() == 0 {
                    break;
                }
                transcript.push_str(&line);
                if in_data {
                    if line == ".\r\n" {
                        in_data = false;
                        w.write_all(b"250 queued\r\n").await.unwrap();
                    }
                    continue;
                }
                let cmd = line.trim_end().to_ascii_uppercase();
                if cmd.starts_with("EHLO") || cmd.starts_with("HELO") {
                    w.write_all(b"250 fake\r\n").await.unwrap();
                } else if cmd == "DATA" {
                    in_data = true;
                    w.write_all(b"354 go\r\n").await.unwrap();
                } else if cmd == "QUIT" {
                    w.write_all(b"221 bye\r\n").await.unwrap();
                    break;
                } else {
                    w.write_all(b"250 ok\r\n").await.unwrap();
                }
            }
            transcript
        });
        (port, handle)
    }

    #[tokio::test]
    async fn smtp_mailer_delivers_through_a_real_socket() {
        let (port, server) = fake_smtp().await;
        let mailer = SmtpMailer::new(&SmtpConfig {
            host: "127.0.0.1".into(),
            port,
            user: None,
            password: None,
            use_tls: false,
            from: "Notturni <no-reply@example.com>".into(),
        })
        .unwrap();
        let mut email = Email::text("lettore@example.com", "Il tuo codice", "123456");
        email.html = Some("<p>123456</p>".into());
        mailer.send(&email).await.unwrap();
        drop(mailer);
        let transcript = server.await.unwrap();
        assert!(transcript.contains("MAIL FROM:<no-reply@example.com>"), "{transcript}");
        assert!(transcript.contains("RCPT TO:<lettore@example.com>"));
        assert!(transcript.contains("Subject: Il tuo codice"));
        assert!(transcript.contains("123456") && transcript.contains("multipart/alternative"));
    }

    #[tokio::test]
    async fn sender_name_is_sanitized_and_the_address_never_changes() {
        let (port, server) = fake_smtp().await;
        let mailer = SmtpMailer::new(&SmtpConfig {
            host: "127.0.0.1".into(),
            port,
            user: None,
            password: None,
            use_tls: false,
            from: "Notturni <no-reply@example.com>".into(),
        })
        .unwrap();
        // un nome con ritorni a capo e un indirizzo finto non può iniettare intestazioni né cambiare il mittente
        let email = Email::alternative("lettore@example.com", "Ciao", "testo", "<p>testo</p>")
            .with_from_name("Redazione\r\nBcc: x@example.com <evil@example.net>");
        mailer.send(&email).await.unwrap();
        drop(mailer);
        let transcript = server.await.unwrap();
        assert!(transcript.contains("MAIL FROM:<no-reply@example.com>"), "{transcript}");
        assert!(!transcript.lines().any(|l| l.starts_with("Bcc:")), "{transcript}");
        assert!(
            transcript.contains("RCPT TO:<lettore@example.com>") && !transcript.contains("RCPT TO:<x@"),
            "{transcript}"
        );
        // il nome è codificato (RFC 2047) e l'indirizzo dopo di esso è sempre quello configurato
        assert!(transcript.contains("=?utf-8?b?") && transcript.contains("<no-reply@example.com>"), "{transcript}");
        assert_eq!(Email::text("a@example.com", "s", "b").with_from_name("  ").from_name, None);
    }

    #[tokio::test]
    async fn invalid_addresses_and_empty_recipients_are_rejected() {
        let mem = MemoryMailer::default();
        assert!(matches!(
            mem.send(&Email::text("non-un-indirizzo", "s", "b")).await,
            Err(MailError::InvalidAddress(_))
        ));
        let none = Email { to: vec![], ..Email::text("a@example.com", "s", "b") };
        assert!(matches!(mem.send(&none).await, Err(MailError::Build(_))));
        assert!(mem.sent().is_empty());
        mem.send(&Email::text("a@example.com", "s", "b")).await.unwrap();
        assert_eq!(mem.sent().len(), 1);
    }

    #[tokio::test]
    async fn not_configured_fails_loudly() {
        assert!(matches!(
            NotConfiguredMailer.send(&Email::text("a@example.com", "s", "b")).await,
            Err(MailError::NotConfigured)
        ));
    }
}
