use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc;

const DEFAULT_FAILURE_THRESHOLD: u32 = 5;
const DEFAULT_OPEN_DURATION_SECS: i64 = 30 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
}

impl CircuitState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Closed => "CLOSED",
            Self::Open => "OPEN",
        }
    }

    pub fn from_str(raw: &str) -> Self {
        match raw {
            "OPEN" => Self::Open,
            _ => Self::Closed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub open_duration_secs: i64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: DEFAULT_FAILURE_THRESHOLD,
            open_duration_secs: DEFAULT_OPEN_DURATION_SECS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CircuitSnapshot {
    pub state: CircuitState,
    pub failure_count: u32,
    pub failure_threshold: u32,
    pub open_until: Option<i64>,
    pub cooldown_until: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct CircuitTransition {
    pub prev_state: CircuitState,
    pub next_state: CircuitState,
    pub reason: &'static str,
    pub snapshot: CircuitSnapshot,
}

#[derive(Debug, Clone)]
pub struct CircuitChange {
    pub before: CircuitSnapshot,
    pub after: CircuitSnapshot,
    pub transition: Option<CircuitTransition>,
}

#[derive(Debug, Clone)]
pub struct CircuitCheck {
    pub allow: bool,
    pub after: CircuitSnapshot,
    pub transition: Option<CircuitTransition>,
}

#[derive(Debug, Clone)]
pub struct CircuitPersistedState {
    pub provider_id: i64,
    pub state: CircuitState,
    pub failure_count: u32,
    pub open_until: Option<i64>,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
struct ProviderHealth {
    state: CircuitState,
    failure_count: u32,
    open_until: Option<i64>,
    cooldown_until: Option<i64>,
    updated_at: i64,
}

impl ProviderHealth {
    fn closed(provider_id: i64, now_unix: i64) -> (i64, Self) {
        (
            provider_id,
            Self {
                state: CircuitState::Closed,
                failure_count: 0,
                open_until: None,
                cooldown_until: None,
                updated_at: now_unix,
            },
        )
    }
}

#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    health: Mutex<HashMap<i64, ProviderHealth>>,
    persist_tx: Option<mpsc::Sender<CircuitPersistedState>>,
}

impl CircuitBreaker {
    pub fn new(
        config: CircuitBreakerConfig,
        initial: HashMap<i64, CircuitPersistedState>,
        persist_tx: Option<mpsc::Sender<CircuitPersistedState>>,
    ) -> Self {
        let mut map = HashMap::with_capacity(initial.len());
        for (provider_id, item) in initial {
            map.insert(
                provider_id,
                ProviderHealth {
                    state: item.state,
                    failure_count: item.failure_count,
                    open_until: item.open_until,
                    cooldown_until: None,
                    updated_at: item.updated_at,
                },
            );
        }

        Self {
            config,
            health: Mutex::new(map),
            persist_tx,
        }
    }

    #[allow(dead_code)]
    pub fn snapshot(&self, provider_id: i64, now_unix: i64) -> CircuitSnapshot {
        let mut guard = self.health.lock().expect("circuit breaker mutex poisoned");
        let entry = guard
            .entry(provider_id)
            .or_insert_with(|| ProviderHealth::closed(provider_id, now_unix).1);
        self.snapshot_from_health(provider_id, entry)
    }

    pub fn should_allow(&self, provider_id: i64, now_unix: i64) -> CircuitCheck {
        let mut upsert: Option<CircuitPersistedState> = None;
        let mut transition: Option<CircuitTransition> = None;

        let (after, allow) = {
            let mut guard = self.health.lock().expect("circuit breaker mutex poisoned");
            let entry = guard
                .entry(provider_id)
                .or_insert_with(|| ProviderHealth::closed(provider_id, now_unix).1);

            if let Some(until) = entry.cooldown_until {
                if now_unix >= until {
                    entry.cooldown_until = None;
                }
            }

            if entry.state == CircuitState::Open {
                let expired = entry.open_until.map(|t| now_unix >= t).unwrap_or(true);
                if expired {
                    let prev = entry.state;
                    entry.state = CircuitState::Closed;
                    entry.failure_count = 0;
                    entry.open_until = None;
                    entry.updated_at = now_unix;

                    let t = CircuitTransition {
                        prev_state: prev,
                        next_state: entry.state,
                        reason: "OPEN_EXPIRED",
                        snapshot: self.snapshot_from_health(provider_id, entry),
                    };

                    transition = Some(t);
                    upsert = Some(self.persisted_from_health(provider_id, entry));
                }
            }

            let after = self.snapshot_from_health(provider_id, entry);
            let cooldown_active = entry.cooldown_until.map(|t| now_unix < t).unwrap_or(false);
            let allow = entry.state != CircuitState::Open && !cooldown_active;
            (after, allow)
        };

        if let Some(item) = upsert {
            self.try_persist(item);
        }

        CircuitCheck {
            allow,
            after,
            transition,
        }
    }

    pub fn record_success(&self, provider_id: i64, now_unix: i64) -> CircuitChange {
        let mut upsert: Option<CircuitPersistedState> = None;

        let (before, after) = {
            let mut guard = self.health.lock().expect("circuit breaker mutex poisoned");
            let entry = guard
                .entry(provider_id)
                .or_insert_with(|| ProviderHealth::closed(provider_id, now_unix).1);

            let before = self.snapshot_from_health(provider_id, entry);

            match entry.state {
                CircuitState::Closed => {
                    entry.cooldown_until = None;
                    if entry.failure_count != 0 {
                        entry.failure_count = 0;
                        entry.updated_at = now_unix;
                        upsert = Some(self.persisted_from_health(provider_id, entry));
                    }
                }
                CircuitState::Open => {}
            }

            let after = self.snapshot_from_health(provider_id, entry);
            (before, after)
        };

        if let Some(item) = upsert {
            self.try_persist(item);
        }

        CircuitChange {
            before,
            after,
            transition: None,
        }
    }

    pub fn record_failure(&self, provider_id: i64, now_unix: i64) -> CircuitChange {
        let mut upsert: Option<CircuitPersistedState> = None;
        let mut transition: Option<CircuitTransition> = None;

        let (before, after) = {
            let mut guard = self.health.lock().expect("circuit breaker mutex poisoned");
            let entry = guard
                .entry(provider_id)
                .or_insert_with(|| ProviderHealth::closed(provider_id, now_unix).1);

            let before = self.snapshot_from_health(provider_id, entry);

            match entry.state {
                CircuitState::Closed => {
                    entry.failure_count = entry.failure_count.saturating_add(1);
                    entry.updated_at = now_unix;

                    if entry.failure_count >= self.config.failure_threshold {
                        let prev = entry.state;
                        entry.state = CircuitState::Open;
                        entry.open_until =
                            Some(now_unix.saturating_add(self.config.open_duration_secs));

                        let after = self.snapshot_from_health(provider_id, entry);
                        let t = CircuitTransition {
                            prev_state: prev,
                            next_state: entry.state,
                            reason: "FAILURE_THRESHOLD_REACHED",
                            snapshot: after.clone(),
                        };
                        transition = Some(t);
                    }

                    upsert = Some(self.persisted_from_health(provider_id, entry));
                }
                CircuitState::Open => {}
            }

            let after = self.snapshot_from_health(provider_id, entry);
            (before, after)
        };

        if let Some(item) = upsert {
            self.try_persist(item);
        }

        CircuitChange {
            before,
            after,
            transition,
        }
    }

    fn snapshot_from_health(&self, _provider_id: i64, health: &ProviderHealth) -> CircuitSnapshot {
        CircuitSnapshot {
            state: health.state,
            failure_count: health.failure_count,
            failure_threshold: self.config.failure_threshold,
            open_until: health.open_until,
            cooldown_until: health.cooldown_until,
        }
    }

    fn persisted_from_health(
        &self,
        provider_id: i64,
        health: &ProviderHealth,
    ) -> CircuitPersistedState {
        CircuitPersistedState {
            provider_id,
            state: health.state,
            failure_count: health.failure_count,
            open_until: health.open_until,
            updated_at: health.updated_at,
        }
    }

    pub fn trigger_cooldown(
        &self,
        provider_id: i64,
        now_unix: i64,
        cooldown_secs: i64,
    ) -> CircuitSnapshot {
        let cooldown_secs = cooldown_secs.max(0);
        if provider_id <= 0 || cooldown_secs == 0 {
            return self.snapshot(provider_id, now_unix);
        }

        let mut guard = self.health.lock().expect("circuit breaker mutex poisoned");
        let entry = guard
            .entry(provider_id)
            .or_insert_with(|| ProviderHealth::closed(provider_id, now_unix).1);

        let next_until = now_unix.saturating_add(cooldown_secs);
        entry.cooldown_until = Some(match entry.cooldown_until {
            Some(existing) => existing.max(next_until),
            None => next_until,
        });
        entry.updated_at = now_unix;

        self.snapshot_from_health(provider_id, entry)
    }

    pub fn reset(&self, provider_id: i64, now_unix: i64) -> CircuitSnapshot {
        if provider_id <= 0 {
            return CircuitSnapshot {
                state: CircuitState::Closed,
                failure_count: 0,
                failure_threshold: self.config.failure_threshold,
                open_until: None,
                cooldown_until: None,
            };
        }

        let (after, upsert) = {
            let mut guard = self.health.lock().expect("circuit breaker mutex poisoned");
            let entry = guard
                .entry(provider_id)
                .or_insert_with(|| ProviderHealth::closed(provider_id, now_unix).1);

            entry.state = CircuitState::Closed;
            entry.failure_count = 0;
            entry.open_until = None;
            entry.cooldown_until = None;
            entry.updated_at = now_unix;

            let after = self.snapshot_from_health(provider_id, entry);
            let upsert = self.persisted_from_health(provider_id, entry);
            (after, upsert)
        };

        self.try_persist(upsert);

        after
    }

    fn try_persist(&self, item: CircuitPersistedState) {
        if let Some(tx) = &self.persist_tx {
            let _ = tx.try_send(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn breaker() -> CircuitBreaker {
        CircuitBreaker::new(CircuitBreakerConfig::default(), HashMap::new(), None)
    }

    #[test]
    fn closed_to_open_after_threshold() {
        let cb = breaker();
        let pid = 1;
        let now = 1_000;
        for i in 1..=DEFAULT_FAILURE_THRESHOLD {
            let change = cb.record_failure(pid, now + i as i64);
            if i < DEFAULT_FAILURE_THRESHOLD {
                assert_eq!(change.after.state, CircuitState::Closed);
            }
        }

        let snap = cb.snapshot(pid, now + 100);
        assert_eq!(snap.state, CircuitState::Open);
        assert!(snap.open_until.is_some());
    }

    #[test]
    fn open_expires_to_closed() {
        let cb = breaker();
        let pid = 1;
        let now = 1_000;
        for i in 1..=DEFAULT_FAILURE_THRESHOLD {
            cb.record_failure(pid, now + i as i64);
        }

        let snap = cb.snapshot(pid, now + 10);
        assert_eq!(snap.state, CircuitState::Open);
        let open_until = snap.open_until.expect("open_until");

        let check = cb.should_allow(pid, open_until);
        assert!(check.allow);
        assert_eq!(check.after.state, CircuitState::Closed);
        assert!(check.transition.is_some());
    }

    #[test]
    fn success_clears_failure_count() {
        let cb = breaker();
        let pid = 1;
        let now = 1_000;
        cb.record_failure(pid, now);
        let before = cb.snapshot(pid, now + 1);
        assert_eq!(before.failure_count, 1);

        cb.record_success(pid, now + 2);
        let after = cb.snapshot(pid, now + 3);
        assert_eq!(after.failure_count, 0);
        assert_eq!(after.state, CircuitState::Closed);
    }

    #[test]
    fn reset_clears_open_and_cooldown() {
        let cb = breaker();
        let pid = 1;
        let now = 1_000;
        for i in 1..=DEFAULT_FAILURE_THRESHOLD {
            cb.record_failure(pid, now + i as i64);
        }

        let open = cb.snapshot(pid, now + 10);
        assert_eq!(open.state, CircuitState::Open);

        let reset = cb.reset(pid, now + 20);
        assert_eq!(reset.state, CircuitState::Closed);
        assert_eq!(reset.failure_count, 0);
        assert!(reset.open_until.is_none());
        assert!(reset.cooldown_until.is_none());

        let allow = cb.should_allow(pid, now + 21);
        assert!(allow.allow);
    }
}
