//! Contratto comune a MemoryCache e RedisCache (`NOCT_TEST_REDIS_URL`).

use noct_cache::{Cache, MemoryCache, RedisCache};
use std::time::Duration;

async fn contract(c: &dyn Cache, ns: &str) {
    c.health().await.unwrap();
    let k = |s: &str| format!("{ns}:{s}");

    // contatore a finestra: cresce, poi riparte dopo la scadenza
    let win = Duration::from_millis(300);
    for expected in 1..=3 {
        assert_eq!(c.incr_window(&k("rl"), win).await.unwrap().count, expected);
    }
    let hit = c.incr_window(&k("rl"), win).await.unwrap();
    assert!(hit.remaining <= win && hit.remaining > Duration::ZERO);
    assert_eq!(c.incr_window(&k("altro"), win).await.unwrap().count, 1, "chiavi indipendenti");
    tokio::time::sleep(Duration::from_millis(350)).await;
    assert_eq!(c.incr_window(&k("rl"), win).await.unwrap().count, 1, "finestra azzerata");

    // chiavi con scadenza
    assert_eq!(c.get(&k("v")).await.unwrap(), None);
    c.set(&k("v"), "uno", None).await.unwrap();
    assert_eq!(c.get(&k("v")).await.unwrap().as_deref(), Some("uno"));
    c.set(&k("t"), "breve", Some(Duration::from_millis(100))).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(c.get(&k("t")).await.unwrap(), None, "scaduta");
    c.delete(&k("v")).await.unwrap();
    assert_eq!(c.get(&k("v")).await.unwrap(), None);

    // lock: esclusivo, rilasciabile solo dal proprietario, scade da solo
    let a = c.try_lock(&k("job"), Duration::from_secs(5)).await.unwrap().expect("libero");
    assert!(c.try_lock(&k("job"), Duration::from_secs(5)).await.unwrap().is_none(), "già tenuto");
    let intruder = noct_cache::LockToken("altro".into());
    assert!(!c.unlock(&k("job"), &intruder).await.unwrap(), "token altrui");
    assert!(c.unlock(&k("job"), &a).await.unwrap());
    let b = c.try_lock(&k("job"), Duration::from_millis(100)).await.unwrap().expect("liberato");
    tokio::time::sleep(Duration::from_millis(150)).await;
    let c2 = c.try_lock(&k("job"), Duration::from_secs(5)).await.unwrap();
    assert!(c2.is_some(), "scaduto: riacquisibile");
    assert!(!c.unlock(&k("job"), &b).await.unwrap(), "il vecchio token non rilascia il nuovo lock");
}

#[tokio::test]
async fn memory_cache_honours_the_contract() {
    contract(&MemoryCache::default(), "mem").await;
}

#[tokio::test]
async fn redis_cache_honours_the_contract() {
    let url =
        std::env::var("NOCT_TEST_REDIS_URL").expect("NOCT_TEST_REDIS_URL non impostata (es. redis://127.0.0.1:6379/0)");
    let cache = RedisCache::connect(&url).await.unwrap();
    contract(&cache, &format!("t{}", uuid::Uuid::new_v4().simple())).await;
}

#[tokio::test]
async fn redis_connection_failure_is_an_error_not_a_panic() {
    assert!(RedisCache::connect("redis://127.0.0.1:1/0").await.is_err());
}
