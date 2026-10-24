use crate::{Config, config::StorageBackend};
use noct_bus::{Bus, MemoryBus, NatsBus};
use noct_cache::{Cache, MemoryCache, RedisCache};
use noct_http::{Moderation, Revalidator};
use noct_mail::{Mailer, MemoryMailer, NotConfiguredMailer, SmtpMailer};
use noct_storage::{LocalStorage, S3Storage, Storage};
use std::{path::PathBuf, sync::Arc};

/// I servizi di infrastruttura, scelti dalla configurazione. I moduli li vedono
/// solo attraverso i trait: cambiare backend non tocca il loro codice.
#[derive(Clone)]
pub struct Services {
    pub storage: Arc<dyn Storage>,
    pub cache: Arc<dyn Cache>,
    pub bus: Arc<dyn Bus>,
    pub mailer: Arc<dyn Mailer>,
    /// Invalidazione della cache del frontend (fail open).
    pub revalidate: Revalidator,
    /// Classificazione automatica delle immagini (fail open).
    pub moderation: Moderation,
}

impl Services {
    pub async fn connect(cfg: &Config) -> Result<Services, String> {
        let storage: Arc<dyn Storage> = match cfg.storage.backend {
            StorageBackend::Local => {
                Arc::new(LocalStorage::new(&cfg.storage.local_base_path, &cfg.storage.local_public_url))
            }
            StorageBackend::S3 => Arc::new(S3Storage::new(&cfg.storage.s3).await),
        };
        for bucket in [&cfg.storage.bucket_avatars, &cfg.storage.bucket_content, &cfg.storage.bucket_audit] {
            storage.ensure_bucket(bucket).await.map_err(|e| format!("storage ({}): {e}", storage.backend()))?;
        }
        let cache: Arc<dyn Cache> = match &cfg.redis_url {
            Some(url) => Arc::new(RedisCache::connect(url).await.map_err(|e| format!("Redis: {e}"))?),
            None => {
                tracing::warn!(
                    "NOCT_REDIS_URL/NOCT_REDIS_HOST non impostato: rate limit e lock in memoria (un solo processo)"
                );
                Arc::new(MemoryCache::default())
            }
        };
        let bus: Arc<dyn Bus> = match &cfg.nats_url {
            Some(url) => Arc::new(NatsBus::connect(url, &cfg.nats_prefix).await.map_err(|e| format!("NATS: {e}"))?),
            None => {
                tracing::warn!("NOCT_NATS_URL non impostato: bus in memoria (un solo processo, nessuna persistenza)");
                Arc::new(MemoryBus::default())
            }
        };
        let mailer: Arc<dyn Mailer> = match &cfg.smtp {
            Some(smtp) => Arc::new(SmtpMailer::new(smtp).map_err(|e| format!("SMTP: {e}"))?),
            None => Arc::new(NotConfiguredMailer),
        };
        Ok(Services {
            storage,
            cache,
            bus,
            mailer,
            revalidate: Revalidator::new(cfg.frontend_revalidate_url.clone(), cfg.revalidate_secret.clone()),
            moderation: Moderation::new(cfg.moderation_service_url.clone()),
        })
    }

    /// Tutto in memoria o su una directory locale, con un mailer ispezionabile: per i test.
    pub fn in_memory(storage_dir: impl Into<PathBuf>) -> (Services, Arc<MemoryMailer>) {
        let mailer = Arc::new(MemoryMailer::default());
        let services = Services {
            storage: Arc::new(LocalStorage::new(storage_dir, "http://localhost:8000/storage")),
            cache: Arc::new(MemoryCache::default()),
            bus: Arc::new(MemoryBus::default()),
            mailer: mailer.clone(),
            revalidate: Revalidator::disabled(),
            moderation: Moderation::disabled(),
        };
        (services, mailer)
    }

    /// Stato di ogni servizio, per `/health/ready`: (nome, backend, esito).
    pub async fn check(&self) -> Vec<(&'static str, &'static str, Result<(), String>)> {
        vec![
            ("storage", self.storage.backend(), self.storage.health().await.map_err(|e| e.to_string())),
            ("cache", self.cache.backend(), self.cache.health().await.map_err(|e| e.to_string())),
            ("bus", self.bus.backend(), self.bus.health().await.map_err(|e| e.to_string())),
        ]
    }
}
