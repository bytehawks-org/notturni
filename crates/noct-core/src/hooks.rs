//! Punti di aggancio del login per i moduli opzionali.
//!
//! Dopo la verifica delle credenziali (password o provider esterno) ogni [`LoginGate`] può
//! fermare l'accesso e rispondere con una sfida da completare (es. `auth_mfa` chiede il codice
//! a due fattori). Senza gate attivi il login emette subito la sessione.
//!
//! [`PersonalData`] è l'altro punto di aggancio: ogni modulo che tiene dati personali di un utente
//! li espone per l'esportazione e li cancella quando l'account viene eliminato (modulo `gdpr`).

use crate::{ApiError, AppState, JsonMap};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::PgConnection;
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

/// Un oggetto su storage da cancellare dopo il commit (la riga è già andata: un orfano non deve far
/// fallire la richiesta).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredObject {
    pub bucket: String,
    pub key: String,
}

/// Dati personali di un modulo: portabilità (GDPR art. 20) e cancellazione (art. 17).
#[async_trait]
pub trait PersonalData: Send + Sync {
    fn module(&self) -> &'static str;

    /// Sezioni JSON dell'esportazione (chiave → valore). Due moduli possono contribuire alla stessa
    /// sezione se entrambi i valori sono oggetti: vengono uniti.
    async fn export(&self, st: &AppState, user_id: Uuid) -> Result<JsonMap, ApiError>;

    /// Cancella i dati puramente personali dell'utente nella transazione `conn`. Blog, post e
    /// commenti non si toccano (diritti di terzi, art. 17.3): restano con l'autore anonimizzato.
    /// Restituisce gli oggetti su storage da eliminare dopo il commit.
    async fn erase(&self, conn: &mut PgConnection, st: &AppState, user_id: Uuid)
    -> Result<Vec<StoredObject>, ApiError>;
}

/// I fornitori di dati personali dei moduli attivi.
#[derive(Clone, Default)]
pub struct PersonalDataProviders {
    list: Arc<Vec<Arc<dyn PersonalData>>>,
}

impl PersonalDataProviders {
    pub fn new(list: Vec<Arc<dyn PersonalData>>) -> Self {
        Self { list: Arc::new(list) }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Le sezioni di tutti i moduli, unite per chiave.
    pub async fn export(&self, st: &AppState, user_id: Uuid) -> Result<JsonMap, ApiError> {
        let mut out = JsonMap::new();
        for provider in self.list.iter() {
            for (key, value) in provider.export(st, user_id).await? {
                match (out.get_mut(&key), value) {
                    (Some(Value::Object(have)), Value::Object(more)) => have.extend(more),
                    (Some(_), _) => {
                        return Err(ApiError::internal(format!(
                            "la sezione `{key}` dell'esportazione è dichiarata due volte e non è un oggetto (modulo `{}`)",
                            provider.module()
                        )));
                    }
                    (None, value) => {
                        out.insert(key, value);
                    }
                }
            }
        }
        Ok(out)
    }

    /// Cancella i dati personali in tutti i moduli, nella stessa transazione.
    pub async fn erase(
        &self,
        conn: &mut PgConnection,
        st: &AppState,
        user_id: Uuid,
    ) -> Result<Vec<StoredObject>, ApiError> {
        let mut objects = Vec::new();
        for provider in self.list.iter() {
            objects.extend(provider.erase(&mut *conn, st, user_id).await?);
        }
        Ok(objects)
    }
}
