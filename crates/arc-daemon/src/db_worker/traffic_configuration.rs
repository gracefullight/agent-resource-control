//! Resident Traffic configuration, frozen once and shared by admission and diagnostics.
use arc_traffic::Policy;
use std::{ffi::OsString, sync::OnceLock};

const CPU_BUDGET: &str = "ARC_TRAFFIC_CPU_BUDGET_MILLIS";
/// Start-rate settings (DEC-013); grammar in `arc_traffic::start_rate`.
const START_RATE: &str = "ARC_TRAFFIC_START_RATE_PER_MINUTE";
const START_BURST: &str = "ARC_TRAFFIC_START_BURST";
const INVALID_START_RATE: &str = "invalid ARC_TRAFFIC_START_RATE_PER_MINUTE or ARC_TRAFFIC_START_BURST: see docs/managed-exec.md for the start-rate grammar";
// Desktop numeric DTOs require exact JavaScript integers as well as SQLite integers.
const MAX_JSON_INTEGER: u64 = 9_007_199_254_740_991;
const INVALID: &str = "invalid ARC_TRAFFIC_CPU_BUDGET_MILLIS: expected decimal millicores fitting every class reservation and JSON/SQLite integers";

pub(crate) fn current() -> Result<&'static Policy, &'static str> {
    static POLICY: OnceLock<Result<Policy, &'static str>> = OnceLock::new();
    POLICY
        .get_or_init(|| {
            configure(
                std::env::var_os(CPU_BUDGET),
                std::env::var_os(START_RATE).as_deref(),
                std::env::var_os(START_BURST).as_deref(),
            )
        })
        .as_ref()
        .inspect(|policy| crate::connection::policy_dispatch::decision_history::install(policy))
        .map_err(|error| *error)
}

/// The budget and the start-rate settings; every start-rate burst admits at
/// least its class window (DEC-013).
fn configure(
    budget: Option<OsString>,
    rate: Option<&std::ffi::OsStr>,
    burst: Option<&std::ffi::OsStr>,
) -> Result<Policy, &'static str> {
    let mut policy = parse(budget)?;
    policy.start_rate = arc_traffic::start_rate::StartRatePolicy::parse(rate, burst)
        .and_then(|start| start.validate(&policy).map(|()| start))
        .map_err(|_| INVALID_START_RATE)?;
    Ok(policy)
}

fn parse(value: Option<OsString>) -> Result<Policy, &'static str> {
    let mut policy = Policy::default();
    if let Some(value) = value {
        policy.budget.cpu_millis = value
            .to_str()
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|s| s.parse().ok())
            .filter(|value| *value <= MAX_JSON_INTEGER)
            .ok_or(INVALID)?;
    }
    policy.validate().map_err(|_| INVALID)?;
    Ok(policy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_reservations_and_windows_are_preserved_with_an_additive_budget() {
        assert_eq!(parse(None), Ok(Policy::default()));
        let configured = parse(Some("4000".into())).unwrap();
        assert_eq!(configured.budget.cpu_millis, 4000);
        assert_eq!(
            configured.budget.memory_bytes,
            Policy::default().budget.memory_bytes
        );
        assert_eq!(configured.build, Policy::default().build);
        assert_eq!(configured.test, Policy::default().test);
        assert_eq!(configured.lint, Policy::default().lint);
        assert_eq!(configured.typecheck, Policy::default().typecheck);
        assert!(configured.budget.cpu_millis >= 2 * configured.build.reservation.cpu_millis);
        assert!(configured.budget.memory_bytes >= 2 * configured.build.reservation.memory_bytes);
        assert!(parse(Some(MAX_JSON_INTEGER.to_string().into())).is_ok());
        assert_eq!(
            parse(Some((MAX_JSON_INTEGER + 1).to_string().into())),
            Err(INVALID)
        );
    }

    #[test]
    fn start_rate_settings_are_conservative_by_default_and_tighten_when_configured() {
        let default = configure(None, None, None).unwrap();
        assert_eq!(default, Policy::default());
        let build = default.start_rate.build;
        assert_eq!(
            (build.tokens_per_minute, build.burst),
            (Some(120), Some(120))
        );
        let tight = configure(None, Some("build=6".as_ref()), Some("1".as_ref())).unwrap();
        assert_eq!(tight.start_rate.build.tokens_per_minute, Some(6));
        assert_eq!(tight.start_rate.test.tokens_per_minute, Some(120));
        assert_eq!(tight.start_rate.lint.burst, Some(1));
        assert_eq!((tight.build, tight.budget), (default.build, default.budget));
        for (rate, burst) in [(Some("0"), None), (None, Some("x")), (Some("make=1"), None)] {
            assert_eq!(
                configure(None, rate.map(AsRef::as_ref), burst.map(AsRef::as_ref)),
                Err(INVALID_START_RATE)
            );
        }
    }

    #[test]
    fn malformed_overflow_and_impossible_budgets_are_rejected() {
        for value in [
            "",
            "0",
            "1",
            "1999",
            "-1",
            "+4000",
            "4_000",
            "4e3",
            " 4000",
            "4000 ",
            "9223372036854775808",
            "18446744073709551616",
        ] {
            assert_eq!(parse(Some(value.into())), Err(INVALID), "{value}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            assert_eq!(parse(Some(OsString::from_vec(vec![0xff]))), Err(INVALID));
        }
    }
}
