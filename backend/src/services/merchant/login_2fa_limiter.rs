use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TOKEN_MAX_FAILURES: u32 = 5;
const TOKEN_WINDOW: Duration = Duration::from_secs(5 * 60);
const USER_MAX_FAILURES: u32 = 10;
const USER_WINDOW: Duration = Duration::from_secs(15 * 60);
const IP_MAX_FAILURES: u32 = 30;
const IP_WINDOW: Duration = Duration::from_secs(15 * 60);
const MAX_TRACKED_KEYS: usize = 50_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LimitDimension {
    User,
    TempToken,
    Ip,
    Capacity,
}

impl LimitDimension {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::TempToken => "temp_token",
            Self::Ip => "ip",
            Self::Capacity => "capacity",
        }
    }
}

#[derive(Debug)]
struct FailureWindow {
    failures: u32,
    in_flight: u32,
    started_at: Instant,
}

impl FailureWindow {
    fn new(now: Instant) -> Self {
        Self {
            failures: 0,
            in_flight: 0,
            started_at: now,
        }
    }

    fn reset_if_expired(&mut self, now: Instant, window: Duration) {
        if now.duration_since(self.started_at) >= window {
            self.failures = 0;
            self.started_at = now;
        }
    }

    fn used_attempts(&self) -> u32 {
        self.failures.saturating_add(self.in_flight)
    }
}

#[derive(Default)]
struct LimiterState {
    users: HashMap<String, FailureWindow>,
    temp_tokens: HashMap<[u8; 32], FailureWindow>,
    ips: HashMap<IpAddr, FailureWindow>,
}

impl LimiterState {
    fn tracked_keys(&self) -> usize {
        self.users.len() + self.temp_tokens.len() + self.ips.len()
    }

    fn prune_expired(&mut self, now: Instant) {
        self.users.retain(|_, entry| {
            entry.in_flight > 0 || now.duration_since(entry.started_at) < USER_WINDOW
        });
        self.temp_tokens.retain(|_, entry| {
            entry.in_flight > 0 || now.duration_since(entry.started_at) < TOKEN_WINDOW
        });
        self.ips.retain(|_, entry| {
            entry.in_flight > 0 || now.duration_since(entry.started_at) < IP_WINDOW
        });
    }
}

#[derive(Default)]
pub(super) struct Login2faRateLimiter {
    state: Mutex<LimiterState>,
}

impl Login2faRateLimiter {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub(super) fn begin(
        self: &Arc<Self>,
        user_id: Option<&str>,
        temp_token: &str,
        client_ip: IpAddr,
    ) -> Result<Login2faAttempt, LimitDimension> {
        let now = Instant::now();
        let token_hash: [u8; 32] = Sha256::digest(temp_token.as_bytes()).into();
        let mut state = self.lock_state();
        state.prune_expired(now);

        if let Some(user_id) = user_id {
            if let Some(entry) = state.users.get_mut(user_id) {
                entry.reset_if_expired(now, USER_WINDOW);
                if entry.used_attempts() >= USER_MAX_FAILURES {
                    return Err(LimitDimension::User);
                }
            }
        }

        if let Some(entry) = state.temp_tokens.get_mut(&token_hash) {
            entry.reset_if_expired(now, TOKEN_WINDOW);
            if entry.used_attempts() >= TOKEN_MAX_FAILURES {
                return Err(LimitDimension::TempToken);
            }
        }

        if let Some(entry) = state.ips.get_mut(&client_ip) {
            entry.reset_if_expired(now, IP_WINDOW);
            if entry.used_attempts() >= IP_MAX_FAILURES {
                return Err(LimitDimension::Ip);
            }
        }

        let new_keys = usize::from(user_id.is_some_and(|id| !state.users.contains_key(id)))
            + usize::from(!state.temp_tokens.contains_key(&token_hash))
            + usize::from(!state.ips.contains_key(&client_ip));
        if state.tracked_keys().saturating_add(new_keys) > MAX_TRACKED_KEYS {
            return Err(LimitDimension::Capacity);
        }

        if let Some(user_id) = user_id {
            state
                .users
                .entry(user_id.to_string())
                .or_insert_with(|| FailureWindow::new(now))
                .in_flight += 1;
        }
        state
            .temp_tokens
            .entry(token_hash)
            .or_insert_with(|| FailureWindow::new(now))
            .in_flight += 1;
        state
            .ips
            .entry(client_ip)
            .or_insert_with(|| FailureWindow::new(now))
            .in_flight += 1;

        drop(state);
        Ok(Login2faAttempt {
            limiter: Arc::clone(self),
            user_id: user_id.map(str::to_string),
            token_hash,
            client_ip,
            finished: false,
        })
    }

    fn finish_failure(&self, attempt: &Login2faAttempt) {
        let mut state = self.lock_state();

        if let Some(user_id) = &attempt.user_id {
            if let Some(entry) = state.users.get_mut(user_id) {
                entry.in_flight = entry.in_flight.saturating_sub(1);
                entry.failures = entry.failures.saturating_add(1);
            }
        }
        if let Some(entry) = state.temp_tokens.get_mut(&attempt.token_hash) {
            entry.in_flight = entry.in_flight.saturating_sub(1);
            entry.failures = entry.failures.saturating_add(1);
        }
        if let Some(entry) = state.ips.get_mut(&attempt.client_ip) {
            entry.in_flight = entry.in_flight.saturating_sub(1);
            entry.failures = entry.failures.saturating_add(1);
        }
    }

    fn finish_success(&self, attempt: &Login2faAttempt) {
        let mut state = self.lock_state();

        if let Some(user_id) = &attempt.user_id {
            decrement_and_clear_success(&mut state.users, user_id);
        }
        decrement_and_clear_success(&mut state.temp_tokens, &attempt.token_hash);

        if let Some(entry) = state.ips.get_mut(&attempt.client_ip) {
            entry.in_flight = entry.in_flight.saturating_sub(1);
            if entry.failures == 0 && entry.in_flight == 0 {
                state.ips.remove(&attempt.client_ip);
            }
        }
    }

    fn cancel(&self, attempt: &Login2faAttempt) {
        let mut state = self.lock_state();

        if let Some(user_id) = &attempt.user_id {
            decrement_and_remove_empty(&mut state.users, user_id);
        }
        decrement_and_remove_empty(&mut state.temp_tokens, &attempt.token_hash);
        decrement_and_remove_empty(&mut state.ips, &attempt.client_ip);
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, LimiterState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn decrement_and_clear_success<K>(map: &mut HashMap<K, FailureWindow>, key: &K)
where
    K: Eq + std::hash::Hash,
{
    if let Some(entry) = map.get_mut(key) {
        entry.in_flight = entry.in_flight.saturating_sub(1);
        entry.failures = 0;
        if entry.in_flight == 0 {
            map.remove(key);
        }
    }
}

fn decrement_and_remove_empty<K>(map: &mut HashMap<K, FailureWindow>, key: &K)
where
    K: Eq + std::hash::Hash,
{
    if let Some(entry) = map.get_mut(key) {
        entry.in_flight = entry.in_flight.saturating_sub(1);
        if entry.failures == 0 && entry.in_flight == 0 {
            map.remove(key);
        }
    }
}

pub(super) struct Login2faAttempt {
    limiter: Arc<Login2faRateLimiter>,
    user_id: Option<String>,
    token_hash: [u8; 32],
    client_ip: IpAddr,
    finished: bool,
}

impl Login2faAttempt {
    pub(super) fn failure(mut self) {
        self.limiter.finish_failure(&self);
        self.finished = true;
    }

    pub(super) fn success(mut self) {
        self.limiter.finish_success(&self);
        self.finished = true;
    }
}

impl Drop for Login2faAttempt {
    fn drop(&mut self) {
        if !self.finished {
            self.limiter.cancel(self);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn ip(last_octet: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(203, 0, 113, last_octet))
    }

    #[test]
    fn token_limit_blocks_concurrent_attempts() {
        let limiter = Login2faRateLimiter::new();
        let mut attempts = Vec::new();

        for _ in 0..TOKEN_MAX_FAILURES {
            attempts.push(limiter.begin(Some("usr_1"), "token", ip(1)).unwrap());
        }

        assert_eq!(
            limiter.begin(Some("usr_1"), "token", ip(1)).err(),
            Some(LimitDimension::TempToken)
        );
        drop(attempts);
    }

    #[test]
    fn rotating_tokens_still_hits_user_limit() {
        let limiter = Login2faRateLimiter::new();

        for attempt in 0..USER_MAX_FAILURES {
            limiter
                .begin(Some("usr_1"), &format!("token_{attempt}"), ip(1))
                .unwrap()
                .failure();
        }

        assert_eq!(
            limiter.begin(Some("usr_1"), "fresh_token", ip(1)).err(),
            Some(LimitDimension::User)
        );
    }

    #[test]
    fn rotating_users_and_tokens_still_hits_ip_limit() {
        let limiter = Login2faRateLimiter::new();

        for attempt in 0..IP_MAX_FAILURES {
            limiter
                .begin(
                    Some(&format!("usr_{attempt}")),
                    &format!("token_{attempt}"),
                    ip(1),
                )
                .unwrap()
                .failure();
        }

        assert_eq!(
            limiter.begin(Some("usr_fresh"), "fresh_token", ip(1)).err(),
            Some(LimitDimension::Ip)
        );
    }

    #[test]
    fn invalid_tokens_are_limited_by_token_and_ip() {
        let limiter = Login2faRateLimiter::new();

        for _ in 0..TOKEN_MAX_FAILURES {
            limiter
                .begin(None, "invalid_token", ip(1))
                .unwrap()
                .failure();
        }

        assert_eq!(
            limiter.begin(None, "invalid_token", ip(1)).err(),
            Some(LimitDimension::TempToken)
        );
    }

    #[test]
    fn success_clears_user_and_token_but_not_ip_failures() {
        let limiter = Login2faRateLimiter::new();

        limiter
            .begin(Some("usr_1"), "token", ip(1))
            .unwrap()
            .failure();
        limiter
            .begin(Some("usr_1"), "token", ip(1))
            .unwrap()
            .success();

        let state = limiter.lock_state();
        let token_hash: [u8; 32] = Sha256::digest(b"token").into();
        assert!(!state.users.contains_key("usr_1"));
        assert!(!state.temp_tokens.contains_key(&token_hash));
        assert_eq!(state.ips.get(&ip(1)).unwrap().failures, 1);
    }

    #[test]
    fn dropped_attempt_releases_reservations_without_counting_failure() {
        let limiter = Login2faRateLimiter::new();

        drop(limiter.begin(Some("usr_1"), "token", ip(1)).unwrap());

        let state = limiter.lock_state();
        assert!(state.users.is_empty());
        assert!(state.temp_tokens.is_empty());
        assert!(state.ips.is_empty());
    }
}
