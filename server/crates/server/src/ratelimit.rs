//! Schutz vor Passwort- und PIN-Raten: nach mehreren Fehlversuchen wird ein Schlüssel
//! (Benutzername, Personalnummer oder Client-Adresse) für eine Sperrzeit blockiert.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::http::HeaderMap;

const MAX_FAILURES: u32 = 5;
const WINDOW: Duration = Duration::from_secs(15 * 60);
const LOCKOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Default)]
pub struct Limiter {
    inner: Arc<Mutex<HashMap<String, Entry>>>,
}

struct Entry {
    failures: u32,
    first: Instant,
    locked_until: Option<Instant>,
}

impl Limiter {
    /// Verbleibende Sperrzeit in Sekunden, falls der Schlüssel gesperrt ist.
    pub fn locked(&self, key: &str) -> Option<u64> {
        let map = self.inner.lock().unwrap();
        map.get(key)
            .and_then(|e| e.locked_until)
            .and_then(|t| t.checked_duration_since(Instant::now()))
            .map(|d| d.as_secs().max(1))
    }

    /// Meldet einen Fehlversuch. Liefert `true`, wenn der Schlüssel dadurch gesperrt wurde.
    pub fn failure(&self, key: &str) -> bool {
        let mut map = self.inner.lock().unwrap();
        let now = Instant::now();
        let e = map.entry(key.to_string()).or_insert(Entry { failures: 0, first: now, locked_until: None });
        if now.duration_since(e.first) > WINDOW {
            e.failures = 0;
            e.first = now;
        }
        e.failures += 1;
        if e.failures >= MAX_FAILURES {
            e.locked_until = Some(now + LOCKOUT);
            e.failures = 0;
            e.first = now;
            true
        } else {
            false
        }
    }

    pub fn success(&self, key: &str) {
        self.inner.lock().unwrap().remove(key);
    }

    pub fn prune(&self) {
        let now = Instant::now();
        self.inner.lock().unwrap().retain(|_, e| {
            e.locked_until.map(|t| t > now).unwrap_or(false) || now.duration_since(e.first) <= WINDOW
        });
    }
}

/// Client-Adresse hinter dem Reverse-Proxy (erster Eintrag von X-Forwarded-For) oder "local".
pub fn client_key(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "local".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locks_after_five_failures() {
        let l = Limiter::default();
        for _ in 0..4 {
            assert!(!l.failure("k"));
            assert!(l.locked("k").is_none());
        }
        assert!(l.failure("k"));
        assert!(l.locked("k").is_some());
        l.success("k");
        assert!(l.locked("k").is_none());
    }
}
