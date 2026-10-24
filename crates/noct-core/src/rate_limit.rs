use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

/// Rate limiter a finestra fissa, in memoria (un solo processo). Dietro questa
/// interfaccia, in Fase 2, arriverà l'implementazione condivisa tra repliche.
#[derive(Default)]
pub struct RateLimiter {
    windows: Mutex<HashMap<String, (Instant, u32)>>,
}

impl RateLimiter {
    /// `Ok` se la richiesta rientra nel limite, `Err(secondi_da_attendere)` altrimenti.
    pub fn check(&self, key: &str, max: u32, window: Duration) -> Result<(), u64> {
        let now = Instant::now();
        let mut map = self.windows.lock().expect("lock avvelenato");
        if map.len() > 10_000 {
            map.retain(|_, (start, _)| now.duration_since(*start) < window);
        }
        let entry = map.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(entry.0) >= window {
            *entry = (now, 0);
        }
        if entry.1 >= max {
            let wait = window.saturating_sub(now.duration_since(entry.0));
            return Err(wait.as_secs().max(1));
        }
        entry.1 += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_max_and_resets_after_window() {
        let rl = RateLimiter::default();
        let w = Duration::from_millis(50);
        assert!(rl.check("k", 2, w).is_ok());
        assert!(rl.check("k", 2, w).is_ok());
        assert!(rl.check("k", 2, w).is_err());
        assert!(rl.check("other", 2, w).is_ok(), "chiavi indipendenti");
        std::thread::sleep(Duration::from_millis(60));
        assert!(rl.check("k", 2, w).is_ok());
    }
}
