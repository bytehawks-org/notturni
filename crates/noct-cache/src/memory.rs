use crate::{Cache, CacheError, LockToken, WindowHit};
use async_trait::async_trait;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

enum Entry {
    Counter { start: Instant, window: Duration, count: u64 },
    Value { value: String, expires: Option<Instant> },
}

#[derive(Default)]
pub struct MemoryCache {
    entries: Mutex<HashMap<String, Entry>>,
}

impl MemoryCache {
    fn with<R>(&self, f: impl FnOnce(&mut HashMap<String, Entry>, Instant) -> R) -> R {
        let now = Instant::now();
        let mut map = self.entries.lock().expect("lock avvelenato");
        if map.len() > 10_000 {
            map.retain(|_, e| match e {
                Entry::Counter { start, window, .. } => now.duration_since(*start) < *window,
                Entry::Value { expires, .. } => expires.is_none_or(|t| t > now),
            });
        }
        f(&mut map, now)
    }
}

#[async_trait]
impl Cache for MemoryCache {
    fn backend(&self) -> &'static str {
        "memory"
    }

    async fn incr_window(&self, key: &str, window: Duration) -> Result<WindowHit, CacheError> {
        Ok(self.with(|map, now| {
            let entry = map.entry(key.to_string()).or_insert(Entry::Counter { start: now, window, count: 0 });
            if !matches!(entry, Entry::Counter { start, window, .. } if now.duration_since(*start) < *window) {
                *entry = Entry::Counter { start: now, window, count: 0 };
            }
            let Entry::Counter { start, count, .. } = entry else { unreachable!() };
            *count += 1;
            WindowHit { count: *count, remaining: window.saturating_sub(now.duration_since(*start)) }
        }))
    }

    async fn get(&self, key: &str) -> Result<Option<String>, CacheError> {
        Ok(self.with(|map, now| match map.get(key) {
            Some(Entry::Value { value, expires }) if expires.is_none_or(|t| t > now) => Some(value.clone()),
            _ => None,
        }))
    }

    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<(), CacheError> {
        self.with(|map, now| {
            map.insert(key.to_string(), Entry::Value { value: value.to_string(), expires: ttl.map(|t| now + t) });
        });
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        self.with(|map, _| map.remove(key));
        Ok(())
    }

    async fn try_lock(&self, key: &str, ttl: Duration) -> Result<Option<LockToken>, CacheError> {
        let token = uuid::Uuid::new_v4().to_string();
        Ok(self.with(|map, now| {
            let key = format!("lock:{key}");
            let held = matches!(map.get(&key), Some(Entry::Value { expires, .. }) if expires.is_none_or(|t| t > now));
            if held {
                return None;
            }
            map.insert(key, Entry::Value { value: token.clone(), expires: Some(now + ttl) });
            Some(LockToken(token))
        }))
    }

    async fn unlock(&self, key: &str, token: &LockToken) -> Result<bool, CacheError> {
        Ok(self.with(|map, now| {
            let key = format!("lock:{key}");
            match map.get(&key) {
                Some(Entry::Value { value, expires }) if *value == token.0 && expires.is_none_or(|t| t > now) => {
                    map.remove(&key);
                    true
                }
                _ => false,
            }
        }))
    }

    async fn health(&self) -> Result<(), CacheError> {
        Ok(())
    }
}
