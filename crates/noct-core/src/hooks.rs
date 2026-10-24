//! Punti di aggancio del login per i moduli opzionali.
//!
//! Dopo la verifica delle credenziali (password o provider esterno) ogni [`LoginGate`] può
//! fermare l'accesso e rispondere con una sfida da completare (es. `auth_mfa` chiede il codice
//! a due fattori). Senza gate attivi il login emette subito la sessione.

use crate::{ApiError, AppState};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait LoginGate: Send + Sync {
    fn module(&self) -> &'static str;

    /// `Some(corpo)`: il login si ferma e la risposta 200 è `corpo` (nessuna sessione emessa).
    /// `extra` porta ciò che il chiamante vuole far sopravvivere alla sfida (es. il profilo
    /// esterno di un collegamento SSO da completare dopo la verifica).
    async fn challenge(&self, st: &AppState, user_id: Uuid, extra: Option<Value>) -> Result<Option<Value>, ApiError>;

    /// La sfida è stata superata e la sessione sta per essere emessa: ogni gate può completare
    /// ciò che aspettava (es. `auth_sso` collega l'identità esterna dopo la verifica MFA).
    /// `extra` è quello passato a [`LoginGate::challenge`].
    async fn completed(&self, _st: &AppState, _user_id: Uuid, _extra: &Value) -> Result<(), ApiError> {
        Ok(())
    }
}

/// I gate dei moduli attivi.
#[derive(Clone, Default)]
pub struct LoginGates {
    list: Arc<Vec<Arc<dyn LoginGate>>>,
}

impl LoginGates {
    pub fn new(list: Vec<Arc<dyn LoginGate>>) -> Self {
        Self { list: Arc::new(list) }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// La prima sfida richiesta da un gate, se c'è.
    pub async fn challenge(
        &self,
        st: &AppState,
        user_id: Uuid,
        extra: Option<Value>,
    ) -> Result<Option<Value>, ApiError> {
        for gate in self.list.iter() {
            if let Some(body) = gate.challenge(st, user_id, extra.clone()).await? {
                return Ok(Some(body));
            }
        }
        Ok(None)
    }

    /// Notifica a tutti i gate che la sfida è stata superata.
    pub async fn completed(&self, st: &AppState, user_id: Uuid, extra: &Value) -> Result<(), ApiError> {
        for gate in self.list.iter() {
            gate.completed(st, user_id, extra).await?;
        }
        Ok(())
    }
}
