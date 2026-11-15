//! Resident Traffic configuration, frozen once and shared by admission and diagnostics.
use arc_traffic::Policy;
use std::{ffi::OsString, sync::OnceLock};

const CPU_BUDGET: &str = "ARC_TRAFFIC_CPU_BUDGET_MILLIS";
// Desktop numeric DTOs require exact JavaScript integers as well as SQLite integers.
const MAX_JSON_INTEGER: u64 = 9_007_199_254_740_991;
const INVALID: &str = "invalid ARC_TRAFFIC_CPU_BUDGET_MILLIS: expected decimal millicores fitting every class reservation and JSON/SQLite integers";

pub(crate) fn current() -> Result<&'static Policy, &'static str> {
    static POLICY: OnceLock<Result<Policy, &'static str>> = OnceLock::new();
    POLICY
        .get_or_init(|| parse(std::env::var_os(CPU_BUDGET)))
        .as_ref()
        .map_err(|error| *error)
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
