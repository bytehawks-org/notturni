use noct_cache::Cache;
use std::{sync::Arc, time::Duration};

/// Rate limit a finestra fissa sopra la [`Cache`]: con Redis è condiviso tra le
/// repliche, senza è per processo.
#[derive(Clone)]
pub struct RateLimiter {
    cache: Arc<dyn Cache>,
}

impl RateLimiter {
    pub fn new(cache: Arc<dyn Cache>) -> Self {
        Self { cache }
    }

    /// `Ok` se la richiesta rientra nel limite, `Err(secondi_da_attendere)` altrimenti.
    /// Se la cache non risponde la richiesta passa (fail open): un Redis giù non
    /// deve spegnere l'API.
    pub async fn check(&self, key: &str, max: u32, window: Duration) -> Result<(), u64> {
        match self.cache.incr_window(key, window).await {
            Ok(hit) if hit.count > u64::from(max) => Err(hit.remaining.as_secs_f64().ceil().max(1.0) as u64),
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::warn!(error = %e, "rate limit non verificabile, richiesta ammessa");
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noct_cache::MemoryCache;

    #[tokio::test]
    async fn blocks_after_max_and_resets_after_window() {
        let rl = RateLimiter::new(Arc::new(MemoryCache::default()));
        let w = Duration::from_millis(100);
        assert!(rl.check("k", 2, w).await.is_ok());
        assert!(rl.check("k", 2, w).await.is_ok());
        assert_eq!(rl.check("k", 2, w).await, Err(1));
        assert!(rl.check("other", 2, w).await.is_ok(), "chiavi indipendenti");
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(rl.check("k", 2, w).await.is_ok());
    }

    struct Broken;

    #[async_trait::async_trait]
    impl Cache for Broken {
        fn backend(&self) -> &'static str {
            "broken"
        }
        async fn incr_window(&self, _: &str, _: Duration) -> Result<noct_cache::WindowHit, noct_cache::CacheError> {
            Err(noct_cache::CacheError::Backend("giù".into()))
        }
        async fn get(&self, _: &str) -> Result<Option<String>, noct_cache::CacheError> {
            unimplemented!()
        }
        async fn set(&self, _: &str, _: &str, _: Option<Duration>) -> Result<(), noct_cache::CacheError> {
            unimplemented!()
        }
        async fn delete(&self, _: &str) -> Result<(), noct_cache::CacheError> {
            unimplemented!()
        }
        async fn try_lock(
            &self,
            _: &str,
            _: Duration,
        ) -> Result<Option<noct_cache::LockToken>, noct_cache::CacheError> {
            unimplemented!()
        }
        async fn unlock(&self, _: &str, _: &noct_cache::LockToken) -> Result<bool, noct_cache::CacheError> {
            unimplemented!()
        }
        async fn health(&self) -> Result<(), noct_cache::CacheError> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn fails_open_when_the_cache_is_down() {
        let rl = RateLimiter::new(Arc::new(Broken));
        for _ in 0..10 {
            assert!(rl.check("k", 1, Duration::from_secs(60)).await.is_ok());
        }
    }
}
