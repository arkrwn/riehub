//! Time. UTC milliseconds everywhere (storage spec, section 2).

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Clock: Send + Sync {
    fn now_millis(&self) -> i64;
}

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_millis(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|since| since.as_millis() as i64)
            .unwrap_or(0)
    }
}

#[derive(Debug)]
pub struct TestClock {
    millis: AtomicI64,
}

impl TestClock {
    pub fn new(start_millis: i64) -> Self {
        Self {
            millis: AtomicI64::new(start_millis),
        }
    }

    pub fn advance(&self, millis: i64) {
        self.millis.fetch_add(millis, Ordering::SeqCst);
    }

    pub fn set(&self, millis: i64) {
        self.millis.store(millis, Ordering::SeqCst);
    }
}

impl Clock for TestClock {
    fn now_millis(&self) -> i64 {
        self.millis.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_moves_when_told_to() {
        let clock = TestClock::new(1_000);
        assert_eq!(clock.now_millis(), 1_000);
        clock.advance(500);
        assert_eq!(clock.now_millis(), 1_500);
        clock.set(42);
        assert_eq!(clock.now_millis(), 42);
    }

    #[test]
    fn system_clock_reports_utc_milliseconds() {
        let now = SystemClock.now_millis();
        let after = SystemClock.now_millis();
        assert!(now > 0);
        assert!(after >= now);
    }
}
