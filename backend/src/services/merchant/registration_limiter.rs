use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SHORT_WINDOW_LIMIT: u32 = 5;
const SHORT_WINDOW: Duration = Duration::from_secs(10 * 60);
const DAILY_LIMIT: u32 = 20;
const DAILY_WINDOW: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_TRACKED_IPS: usize = 50_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RegistrationLimitDimension {
    ShortWindow,
    Daily,
    Capacity,
}

impl RegistrationLimitDimension {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::ShortWindow => "short_window",
            Self::Daily => "daily",
            Self::Capacity => "capacity",
        }
    }
}

#[derive(Debug)]
struct RegistrationWindow {
    short_count: u32,
    short_started_at: Instant,
    daily_count: u32,
    daily_started_at: Instant,
}

impl RegistrationWindow {
    fn new(now: Instant) -> Self {
        Self {
            short_count: 0,
            short_started_at: now,
            daily_count: 0,
            daily_started_at: now,
        }
    }

    fn reset_expired(&mut self, now: Instant) {
        if now.duration_since(self.short_started_at) >= SHORT_WINDOW {
            self.short_count = 0;
            self.short_started_at = now;
        }
        if now.duration_since(self.daily_started_at) >= DAILY_WINDOW {
            self.daily_count = 0;
            self.daily_started_at = now;
        }
    }

    fn is_expired(&self, now: Instant) -> bool {
        now.duration_since(self.daily_started_at) >= DAILY_WINDOW
    }
}

#[derive(Default)]
pub(super) struct RegistrationRateLimiter {
    state: Mutex<HashMap<IpAddr, RegistrationWindow>>,
}

impl RegistrationRateLimiter {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub(super) fn check_and_record(
        &self,
        client_ip: IpAddr,
    ) -> Result<(), RegistrationLimitDimension> {
        self.check_and_record_at(client_ip, Instant::now())
    }

    fn check_and_record_at(
        &self,
        client_ip: IpAddr,
        now: Instant,
    ) -> Result<(), RegistrationLimitDimension> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if !state.contains_key(&client_ip) && state.len() >= MAX_TRACKED_IPS {
            state.retain(|_, window| !window.is_expired(now));
            if state.len() >= MAX_TRACKED_IPS {
                return Err(RegistrationLimitDimension::Capacity);
            }
        }

        let window = state
            .entry(client_ip)
            .or_insert_with(|| RegistrationWindow::new(now));
        window.reset_expired(now);

        if window.short_count >= SHORT_WINDOW_LIMIT {
            return Err(RegistrationLimitDimension::ShortWindow);
        }
        if window.daily_count >= DAILY_LIMIT {
            return Err(RegistrationLimitDimension::Daily);
        }

        window.short_count += 1;
        window.daily_count += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip() -> IpAddr {
        "203.0.113.10".parse().unwrap()
    }

    #[test]
    fn blocks_sixth_attempt_in_ten_minutes() {
        let limiter = RegistrationRateLimiter::new();

        for _ in 0..SHORT_WINDOW_LIMIT {
            assert!(limiter.check_and_record(ip()).is_ok());
        }
        assert_eq!(
            limiter.check_and_record(ip()),
            Err(RegistrationLimitDimension::ShortWindow)
        );
    }

    #[test]
    fn daily_limit_survives_short_window_resets() {
        let limiter = RegistrationRateLimiter::new();
        let start = Instant::now();

        for batch in 0..4 {
            let now = start + (SHORT_WINDOW + Duration::from_secs(1)) * batch;
            for _ in 0..SHORT_WINDOW_LIMIT {
                assert!(limiter.check_and_record_at(ip(), now).is_ok());
            }
        }

        let next_window = start + (SHORT_WINDOW + Duration::from_secs(1)) * 4;
        assert_eq!(
            limiter.check_and_record_at(ip(), next_window),
            Err(RegistrationLimitDimension::Daily)
        );
    }
}
