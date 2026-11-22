//! Per-class start-rate control under the existing windows and reservations
//! (lane C, DEC-013, POL-001, SRS §9.4).
//!
//! A token bucket per class: `burst` tokens, refilled at `tokens_per_minute`.
//! It is evaluated only after a class window and the budgets admitted a job,
//! so it can delay a start and never admits one the windows would hold.
//! `None` is unlimited. The daemon reads the startup configuration names
//! `ARC_TRAFFIC_START_RATE_PER_MINUTE` and `ARC_TRAFFIC_START_BURST` once per
//! process (`arc-daemon db_worker/traffic_configuration.rs`); this module owns
//! their grammar. Policy switch `traffic.start_rate_control` turns it on/off.
//!
//! Grammar of each value: one decimal applied to every class (`12`), or a
//! comma-separated list of `class=decimal` (`build=6,test=30`); an unlisted
//! class keeps its default. Rates are 1..=60000 per minute, bursts 1..=10000.
//!
//! The conservative default is 120 starts per minute with a burst of 120 for
//! every class: never more concurrent jobs than the class window of one, and a
//! runaway agent's start storm is bounded to two starts per second per class
//! after its first burst. The burst is far above the window, so ordinary use
//! and every existing queue (up to 101 waiters) never meets the bucket.
use std::ffi::OsStr;

pub const DEFAULT_TOKENS_PER_MINUTE: u32 = 120;
pub const DEFAULT_BURST: u32 = 120;
pub const MAX_TOKENS_PER_MINUTE: u32 = 60_000;
pub const MAX_BURST: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartRate {
    pub tokens_per_minute: Option<u32>,
    pub burst: Option<u32>,
}

impl Default for StartRate {
    fn default() -> Self {
        Self {
            tokens_per_minute: Some(DEFAULT_TOKENS_PER_MINUTE),
            burst: Some(DEFAULT_BURST),
        }
    }
}

impl StartRate {
    /// Milliseconds between two refilled tokens, rounded up so the configured
    /// rate is never exceeded. `None` when the class is unlimited.
    pub fn interval_ms(self) -> Option<u64> {
        let rate = u64::from(self.tokens_per_minute.filter(|r| *r > 0)?);
        self.burst.filter(|b| *b > 0)?;
        Some(60_000_u64.div_ceil(rate))
    }

    /// The earliest time a new start may take a token, given the recorded
    /// start times of this class in ascending order (a generic cell-rate
    /// evaluation of the bucket). `None` when the class is unlimited. A value
    /// at or before `now_ms` means a token is available now.
    pub fn next_token_ms(self, starts: &[u64], now_ms: u64) -> Option<u64> {
        let interval = self.interval_ms()?;
        let burst = u64::from(self.burst?);
        let lookback = interval.saturating_mul(burst);
        let horizon = now_ms.saturating_sub(lookback);
        let mut theoretical = 0_u64;
        for start in starts.iter().copied().filter(|start| *start > horizon) {
            theoretical = theoretical.max(start).saturating_add(interval);
        }
        Some(theoretical.saturating_sub((burst - 1).saturating_mul(interval)))
    }

    /// Start times older than this cannot affect [`Self::next_token_ms`].
    pub fn lookback_ms(self) -> Option<u64> {
        Some(self.interval_ms()?.saturating_mul(u64::from(self.burst?)))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StartRatePolicy {
    pub build: StartRate,
    pub test: StartRate,
    pub lint: StartRate,
    pub typecheck: StartRate,
}

/// Every class, in the order of `Policy`.
pub const CLASSES: [crate::TrafficClass; 4] = [
    crate::TrafficClass::Build,
    crate::TrafficClass::Test,
    crate::TrafficClass::Lint,
    crate::TrafficClass::Typecheck,
];

const INVALID: &str = "invalid start-rate configuration";

impl StartRatePolicy {
    /// Parse the two startup configuration values; `None` keeps the default.
    pub fn parse(rate: Option<&OsStr>, burst: Option<&OsStr>) -> Result<Self, &'static str> {
        let mut policy = Self::default();
        if let Some(rate) = rate {
            for (class, value) in entries(rate, MAX_TOKENS_PER_MINUTE)? {
                policy.class_mut(class).tokens_per_minute = Some(value);
            }
        }
        if let Some(burst) = burst {
            for (class, value) in entries(burst, MAX_BURST)? {
                policy.class_mut(class).burst = Some(value);
            }
        }
        Ok(policy)
    }

    /// Every burst admits at least the class window, so a window is never
    /// narrowed by the bucket alone.
    pub fn validate(&self, policy: &crate::Policy) -> Result<(), &'static str> {
        for class in CLASSES {
            let rate = self.class(class);
            if rate
                .burst
                .is_some_and(|burst| burst < policy.class(class).window)
            {
                return Err(INVALID);
            }
        }
        Ok(())
    }

    pub fn class(&self, class: crate::TrafficClass) -> StartRate {
        match class {
            crate::TrafficClass::Build => self.build,
            crate::TrafficClass::Test => self.test,
            crate::TrafficClass::Lint => self.lint,
            crate::TrafficClass::Typecheck => self.typecheck,
        }
    }

    fn class_mut(&mut self, class: crate::TrafficClass) -> &mut StartRate {
        match class {
            crate::TrafficClass::Build => &mut self.build,
            crate::TrafficClass::Test => &mut self.test,
            crate::TrafficClass::Lint => &mut self.lint,
            crate::TrafficClass::Typecheck => &mut self.typecheck,
        }
    }
}

fn decimal(text: &str, max: u32) -> Result<u32, &'static str> {
    if text.is_empty() || text.len() > 5 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(INVALID);
    }
    text.parse::<u32>()
        .ok()
        .filter(|value| (1..=max).contains(value))
        .ok_or(INVALID)
}

fn entries(value: &OsStr, max: u32) -> Result<Vec<(crate::TrafficClass, u32)>, &'static str> {
    let text = value.to_str().ok_or(INVALID)?;
    if !text.contains('=') {
        let value = decimal(text, max)?;
        return Ok(CLASSES.map(|c| (c, value)).to_vec());
    }
    let mut seen = Vec::new();
    for item in text.split(',') {
        let (name, number) = item.split_once('=').ok_or(INVALID)?;
        let class = match name {
            "build" => crate::TrafficClass::Build,
            "test" => crate::TrafficClass::Test,
            "lint" => crate::TrafficClass::Lint,
            "typecheck" => crate::TrafficClass::Typecheck,
            _ => return Err(INVALID),
        };
        if seen.iter().any(|(c, _)| *c == class) {
            return Err(INVALID);
        }
        seen.push((class, decimal(number, max)?));
    }
    Ok(seen)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Policy;

    fn os(text: &str) -> Option<&OsStr> {
        Some(OsStr::new(text))
    }

    #[test]
    fn conservative_defaults_cover_every_class_and_admit_at_least_the_window() {
        let policy = StartRatePolicy::parse(None, None).unwrap();
        assert_eq!(policy, StartRatePolicy::default());
        for class in CLASSES {
            assert_eq!(
                policy.class(class),
                StartRate {
                    tokens_per_minute: Some(120),
                    burst: Some(120)
                }
            );
            assert!(policy.class(class).burst.unwrap() >= Policy::default().class(class).window);
        }
        assert_eq!(policy.validate(&Policy::default()), Ok(()));
    }

    #[test]
    fn uniform_and_per_class_values_parse_and_malformed_values_are_rejected() {
        let all = StartRatePolicy::parse(os("6"), os("1")).unwrap();
        assert_eq!(all.build.tokens_per_minute, Some(6));
        assert_eq!(all.typecheck.burst, Some(1));
        let one = StartRatePolicy::parse(os("build=12,test=30"), None).unwrap();
        assert_eq!(one.build.tokens_per_minute, Some(12));
        assert_eq!(one.test.tokens_per_minute, Some(30));
        assert_eq!(one.lint, StartRate::default());
        for bad in [
            "",
            "0",
            "-1",
            "+6",
            "6 ",
            "60001",
            "99999999",
            "build",
            "build=",
            "build=0",
            "make=1",
            "build=1,build=2",
            "build=1;test=2",
            "4e3",
        ] {
            assert_eq!(StartRatePolicy::parse(os(bad), None), Err(INVALID), "{bad}");
        }
        assert_eq!(StartRatePolicy::parse(None, os("10001")), Err(INVALID));
        let mut wide = Policy::default();
        wide.build.window = 2;
        assert_eq!(
            StartRatePolicy::parse(None, os("1"))
                .unwrap()
                .validate(&wide),
            Err(INVALID)
        );
    }

    #[test]
    fn the_bucket_holds_after_its_burst_and_refills_at_its_rate() {
        let rate = StartRate {
            tokens_per_minute: Some(6),
            burst: Some(2),
        };
        assert_eq!(rate.interval_ms(), Some(10_000));
        assert!(rate.next_token_ms(&[], 50_000).unwrap() <= 50_000);
        assert!(rate.next_token_ms(&[50_000], 50_000).unwrap() <= 50_000);
        assert_eq!(rate.next_token_ms(&[50_000, 50_000], 50_000), Some(60_000));
        assert!(rate.next_token_ms(&[50_000, 50_000], 60_000).unwrap() <= 60_000);
        // Starts older than the lookback no longer count.
        assert!(rate.next_token_ms(&[1, 2], 50_000).unwrap() <= 50_000);
        let unlimited = StartRate {
            tokens_per_minute: None,
            burst: None,
        };
        assert_eq!(unlimited.next_token_ms(&[1, 1, 1], 1), None);
        assert_eq!(rate.lookback_ms(), Some(20_000));
    }
}
