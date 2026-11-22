//! Punti di estensione di blog e post per i moduli opzionali.
//!
//! Il core possiede le colonne essenziali di `blogs` e `posts`; ciò che appartiene a
//! un modulo (categoria → `taxonomy`, modalità commenti → `comments`, note a piè di
//! pagina → `notes`, ...) vive in tabelle del modulo. Per mantenere il contratto
//! `/api/v1` (un solo `PATCH /posts/{id}` con `category_id`, un solo `PostOut`)
//! il modulo registra un [`EntityExtension`] che:
//!
//! * **legge** i propri campi dal corpo di create/update e li scrive nella *stessa
//!   transazione* del core ([`EntityExtension::apply`]);
//! * **aggiunge** i propri campi alle risposte, in blocco per un insieme di entità
//!   ([`EntityExtension::enrich`], una query per elenco e non una per riga).
//!
//! Un modulo disattivato non registra nulla: i suoi campi non compaiono né vengono
//! accettati. I campi dichiarati da due estensioni della stessa entità non possono
//! sovrapporsi (verificato all'avvio).

use crate::{Actor, ApiError, AppState, BlogRef};
use async_trait::async_trait;
use serde_json::{Map, Value};
use sqlx::{PgConnection, PgPool};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

pub type JsonMap = Map<String, Value>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Entity {
    Blog,
    Post,
    /// Profilo utente: solo campi in uscita ([`EntityExtension::enrich`]); `apply` non viene chiamato.
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Create,
    Update,
    /// Creazione di un post come traduzione di un altro (`source_id`).
    Translate,
}

/// Contesto di una scrittura, passato a [`EntityExtension::apply`].
pub struct ExtCtx<'a> {
    /// Stato dell'applicazione (configurazione dei moduli, servizi).
    pub state: &'a AppState,
    pub op: Op,
    /// Id dell'entità appena scritta (già presente nella transazione).
    pub entity_id: Uuid,
    /// Per [`Op::Translate`], il post originale.
    pub source_id: Option<Uuid>,
    pub blog: &'a BlogRef,
    pub actor: &'a Actor,
}

impl ExtCtx<'_> {
    /// Valore stringa della configurazione di `module` (vuoto se non impostato).
    pub fn config_str(&self, module: &str, key: &str) -> Option<&str> {
        match self.state.module_config.get(module)?.get(key)? {
            noct_manifest::ConfigValue::String(s) if !s.is_empty() => Some(s),
            _ => None,
        }
    }
}

#[async_trait]
pub trait EntityExtension: Send + Sync {
    fn module(&self) -> &'static str;
    fn entity(&self) -> Entity;
    /// Chiavi JSON del corpo di create/update gestite da questa estensione, e
    /// anche chiavi che aggiunge alla risposta.
    fn fields(&self) -> &'static [&'static str];

    /// Scrive nella transazione `conn`, dopo il salvataggio del core. `input` contiene
    /// solo le chiavi di [`EntityExtension::fields`] presenti nel corpo (può essere vuoto:
    /// alla creazione serve comunque a inizializzare i default). Un errore annulla l'intera
    /// richiesta.
    async fn apply(&self, conn: &mut PgConnection, ctx: &ExtCtx<'_>, input: &JsonMap) -> Result<(), ApiError>;

    /// Campi aggiuntivi in uscita per `ids`; ogni id deve avere una voce (con i default
    /// del modulo se non ha dati propri).
    async fn enrich(&self, db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, JsonMap>, ApiError>;
}

/// Estensioni dei moduli attivi.
#[derive(Clone, Default)]
pub struct Extensions {
    list: Arc<Vec<Arc<dyn EntityExtension>>>,
}

impl Extensions {
    pub fn new(list: Vec<Arc<dyn EntityExtension>>) -> Result<Extensions, String> {
        let mut owner: HashMap<(Entity, &'static str), &'static str> = HashMap::new();
        for ext in &list {
            for field in ext.fields() {
                if let Some(prev) = owner.insert((ext.entity(), field), ext.module()) {
                    return Err(format!(
                        "il campo `{field}` di {:?} è dichiarato sia da `{prev}` sia da `{}`",
                        ext.entity(),
                        ext.module()
                    ));
                }
            }
        }
        Ok(Extensions { list: Arc::new(list) })
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    fn of(&self, entity: Entity) -> impl Iterator<Item = &Arc<dyn EntityExtension>> {
        self.list.iter().filter(move |e| e.entity() == entity)
    }

    /// Chiavi extra accettate per `entity`.
    pub fn fields(&self, entity: Entity) -> Vec<&'static str> {
        self.of(entity).flat_map(|e| e.fields().iter().copied()).collect()
    }

    /// Passa a ogni estensione le sue chiavi del corpo e ne esegue la scrittura.
    pub async fn apply(
        &self,
        conn: &mut PgConnection,
        entity: Entity,
        ctx: &ExtCtx<'_>,
        body: &JsonMap,
    ) -> Result<(), ApiError> {
        for ext in self.of(entity) {
            let input: JsonMap =
                ext.fields().iter().filter_map(|k| body.get(*k).map(|v| ((*k).to_string(), v.clone()))).collect();
            ext.apply(conn, ctx, &input).await?;
        }
        Ok(())
    }

    /// Campi extra per ogni id, unendo i contributi di tutte le estensioni.
    pub async fn enrich(&self, db: &PgPool, entity: Entity, ids: &[Uuid]) -> Result<HashMap<Uuid, JsonMap>, ApiError> {
        let mut out: HashMap<Uuid, JsonMap> = HashMap::new();
        if ids.is_empty() {
            return Ok(out);
        }
        for ext in self.of(entity) {
            for (id, fields) in ext.enrich(db, ids).await? {
                out.entry(id).or_default().extend(fields);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(&'static str, Entity, &'static [&'static str]);

    #[async_trait]
    impl EntityExtension for Fake {
        fn module(&self) -> &'static str {
            self.0
        }
        fn entity(&self) -> Entity {
            self.1
        }
        fn fields(&self) -> &'static [&'static str] {
            self.2
        }
        async fn apply(&self, _: &mut PgConnection, _: &ExtCtx<'_>, _: &JsonMap) -> Result<(), ApiError> {
            Ok(())
        }
        async fn enrich(&self, _: &PgPool, _: &[Uuid]) -> Result<HashMap<Uuid, JsonMap>, ApiError> {
            Ok(HashMap::new())
        }
    }

    #[test]
    fn overlapping_fields_are_rejected_per_entity() {
        let a: Arc<dyn EntityExtension> = Arc::new(Fake("a", Entity::Post, &["category_id"]));
        let b: Arc<dyn EntityExtension> = Arc::new(Fake("b", Entity::Post, &["category_id"]));
        let c: Arc<dyn EntityExtension> = Arc::new(Fake("c", Entity::Blog, &["category_id"]));
        assert!(Extensions::new(vec![a.clone(), c.clone()]).is_ok(), "stessa chiave su entità diverse: ok");
        let err = Extensions::new(vec![a, b]).err().unwrap();
        assert!(err.contains("category_id") && err.contains('a') && err.contains('b'), "{err}");
    }

    #[test]
    fn fields_are_collected_per_entity() {
        let list: Vec<Arc<dyn EntityExtension>> =
            vec![Arc::new(Fake("a", Entity::Post, &["x", "y"])), Arc::new(Fake("b", Entity::Blog, &["z"]))];
        let ext = Extensions::new(list).unwrap();
        assert_eq!(ext.fields(Entity::Post), vec!["x", "y"]);
        assert_eq!(ext.fields(Entity::Blog), vec!["z"]);
    }
}
