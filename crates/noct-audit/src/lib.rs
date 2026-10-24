//! Registro di audit. `record()` scrive nella **stessa transazione** dell'azione
//! tracciata, senza commit propri: la riga esiste se e solo se l'azione è andata a
//! buon fine (un log di sicurezza non deve poter divergere dallo stato reale).
//! Per gli eventi che devono restare anche se l'azione fallisce (es. login
//! fallito) il chiamante apre una transazione dedicata.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

use noct_core::{RequestMeta, TokenOwner, TokenPrincipal};
use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

/// Chi ha originato l'evento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditActor {
    User {
        id: Uuid,
        label: String,
    },
    CoreToken {
        id: Uuid,
    },
    UserToken {
        id: Uuid,
        user_id: Uuid,
    },
    /// Processo interno senza attore umano (avvio, job schedulati).
    System,
    /// Richiesta non autenticata (es. login fallito); `label` è ciò che ha dichiarato.
    Anonymous {
        label: Option<String>,
    },
}

impl AuditActor {
    /// Snapshot leggibile dell'utente, da salvare in `actor_label`.
    pub fn user(id: Uuid, username: &str, email: &str) -> Self {
        AuditActor::User { id, label: format!("{username} <{email}>") }
    }

    pub fn from_token(t: &TokenPrincipal) -> Self {
        match t.owner {
            TokenOwner::Core => AuditActor::CoreToken { id: t.id },
            TokenOwner::User(user_id) => AuditActor::UserToken { id: t.id, user_id },
        }
    }

    fn parts(&self) -> (&'static str, Option<Uuid>, Option<&str>) {
        match self {
            AuditActor::User { id, label } => ("user", Some(*id), Some(label)),
            AuditActor::CoreToken { id } => ("core_token", Some(*id), None),
            AuditActor::UserToken { id, .. } => ("user_token", Some(*id), None),
            AuditActor::System => ("system", None, None),
            AuditActor::Anonymous { label } => ("anonymous", None, label.as_deref()),
        }
    }
}

pub struct Entry<'a> {
    /// Verbo puntato: `auth.login`, `api_token.created`...
    pub action: &'a str,
    pub actor: AuditActor,
    pub target: Option<(&'a str, Uuid)>,
    pub blog_id: Option<Uuid>,
    pub meta: &'a RequestMeta,
    pub payload: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("azione di audit non valida: `{0}`")]
    InvalidAction(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

fn valid_action(action: &str) -> bool {
    action.len() <= 100
        && !action.is_empty()
        && action
            .split('.')
            .all(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
}

/// Aggiunge una riga al registro nella transazione `conn`.
pub async fn record(conn: &mut PgConnection, entry: Entry<'_>) -> Result<(), AuditError> {
    if !valid_action(entry.action) {
        return Err(AuditError::InvalidAction(entry.action.to_string()));
    }
    let (actor_type, actor_id, actor_label) = entry.actor.parts();
    sqlx::query(
        "INSERT INTO audit_log (actor_type, actor_id, actor_label, action, target_type, target_id, blog_id, ip, user_agent, payload)
         VALUES ($1::audit_actor_type, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(actor_type)
    .bind(actor_id)
    .bind(actor_label.map(|l| l.chars().take(255).collect::<String>()))
    .bind(entry.action)
    .bind(entry.target.map(|(t, _)| t))
    .bind(entry.target.map(|(_, id)| id))
    .bind(entry.blog_id)
    .bind(entry.meta.ip.as_deref())
    .bind(entry.meta.user_agent.as_deref())
    .bind(&entry.payload)
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_names_are_dotted_lowercase_verbs() {
        for ok in ["auth.login", "user.role_change", "api_token.created", "a"] {
            assert!(valid_action(ok), "{ok}");
        }
        for bad in ["", "Auth.login", "auth..login", ".auth", "auth login", &"x".repeat(101)] {
            assert!(!valid_action(bad), "{bad}");
        }
    }

    #[test]
    fn actors_map_to_audit_columns() {
        let id = Uuid::new_v4();
        assert_eq!(
            AuditActor::user(id, "anna", "a@example.com").parts(),
            ("user", Some(id), Some("anna <a@example.com>"))
        );
        assert_eq!(AuditActor::System.parts(), ("system", None, None));
        assert_eq!(AuditActor::Anonymous { label: Some("x@y.z".into()) }.parts().2, Some("x@y.z"));
        let t = TokenPrincipal { id, owner: TokenOwner::Core };
        assert_eq!(AuditActor::from_token(&t).parts().0, "core_token");
        let t = TokenPrincipal { id, owner: TokenOwner::User(Uuid::new_v4()) };
        assert_eq!(AuditActor::from_token(&t).parts().0, "user_token");
    }
}
