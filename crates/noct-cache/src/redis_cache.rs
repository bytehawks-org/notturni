use crate::{Cache, CacheError, LockToken, NAMESPACE, WindowHit};
use async_trait::async_trait;
use redis::{Script, aio::ConnectionManager};
use std::time::Duration;

fn err(e: redis::RedisError) -> CacheError {
    CacheError::Backend(e.to_string())
}

/// INCR atomico che imposta la scadenza al primo incremento; restituisce {conteggio, ttl_ms}.
const INCR_WINDOW: &str = r#"
local c = redis.call('INCR', KEYS[1])
if c == 1 then redis.call('PEXPIRE', KEYS[1], ARGV[1]) end
local ttl = redis.call('PTTL', KEYS[1])
if ttl < 0 then redis.call('PEXPIRE', KEYS[1], ARGV[1]); ttl = tonumber(ARGV[1]) end
return {c, ttl}
"#;

/// Rilascio sicuro: cancella solo se il valore è ancora il nostro token.
const UNLOCK: &str = r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then return redis.call('DEL', KEYS[1]) else return 0 end
"#;

#[derive(Clone)]
pub struct RedisCache {
    conn: ConnectionManager,
}

impl RedisCache {
    /// `url`: es. `redis://localhost:6379/0`. La connessione si ristabilisce da sola.
    pub async fn connect(url: &str) -> Result<Self, CacheError> {
        let client = redis::Client::open(url).map_err(err)?;
        let conn = tokio::time::timeout(Duration::from_secs(3), ConnectionManager::new(client))
            .await
            .map_err(|_| CacheError::Backend("timeout di connessione a Redis".into()))?
            .map_err(err)?;
        Ok(Self { conn })
    }
}

fn ms(d: Duration) -> u64 {
    d.as_millis().clamp(1, u128::from(u64::MAX)) as u64
}

#[async_trait]
impl Cache for RedisCache {
    fn backend(&self) -> &'static str {
        "redis"
    }

    async fn incr_window(&self, key: &str, window: Duration) -> Result<WindowHit, CacheError> {
        let mut conn = self.conn.clone();
        let (count, ttl): (u64, i64) = Script::new(INCR_WINDOW)
            .key(format!("{NAMESPACE}rl:{key}"))
            .arg(ms(window))
            .invoke_async(&mut conn)
            .await
            .map_err(err)?;
        Ok(WindowHit { count, remaining: Duration::from_millis(ttl.max(0) as u64) })
    }

    async fn get(&self, key: &str) -> Result<Option<String>, CacheError> {
        let mut conn = self.conn.clone();
        redis::cmd("GET").arg(format!("{NAMESPACE}kv:{key}")).query_async(&mut conn).await.map_err(err)
    }

    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<(), CacheError> {
        let mut conn = self.conn.clone();
        let mut cmd = redis::cmd("SET");
        cmd.arg(format!("{NAMESPACE}kv:{key}")).arg(value);
        if let Some(ttl) = ttl {
            cmd.arg("PX").arg(ms(ttl));
        }
        cmd.query_async::<()>(&mut conn).await.map_err(err)
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        let mut conn = self.conn.clone();
        redis::cmd("DEL").arg(format!("{NAMESPACE}kv:{key}")).query_async::<()>(&mut conn).await.map_err(err)
    }

    async fn try_lock(&self, key: &str, ttl: Duration) -> Result<Option<LockToken>, CacheError> {
        let mut conn = self.conn.clone();
        let token = uuid::Uuid::new_v4().to_string();
        let set: Option<String> = redis::cmd("SET")
            .arg(format!("{NAMESPACE}lock:{key}"))
            .arg(&token)
            .arg("NX")
            .arg("PX")
            .arg(ms(ttl))
            .query_async(&mut conn)
            .await
            .map_err(err)?;
        Ok(set.map(|_| LockToken(token)))
    }

    async fn unlock(&self, key: &str, token: &LockToken) -> Result<bool, CacheError> {
        let mut conn = self.conn.clone();
        let deleted: i64 = Script::new(UNLOCK)
            .key(format!("{NAMESPACE}lock:{key}"))
            .arg(&token.0)
            .invoke_async(&mut conn)
            .await
            .map_err(err)?;
        Ok(deleted == 1)
    }

    async fn health(&self) -> Result<(), CacheError> {
        let mut conn = self.conn.clone();
        redis::cmd("PING").query_async::<String>(&mut conn).await.map(|_| ()).map_err(err)
    }
}
