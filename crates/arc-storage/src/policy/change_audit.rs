//! Before/after `audit_events` rows for every policy change (lane C, POL-004).
//!
//! Called from `governor::set_switch` (`policy.set_switch`, `daemon.pause`,
//! `daemon.resume`) and from the start-rate switch after a committed change,
//! inside the same transaction, and once at daemon startup with the frozen
//! Traffic configuration (class windows, per-class estimates, budgets,
//! start-rate settings). The configuration is audited only when it differs
//! from the last recorded one; with none recorded the baseline is the built-in
//! default, so an unconfigured daemon writes no row. No learned policy value
//! exists at this snapshot, so none is audited.
//!
//! Payloads hold policy values only: never a path, a command or an identity.
use crate::jobs::{JobError, JobResult};
use arc_ipc::governor::GovernorSwitchState;
use arc_traffic::{start_rate::CLASSES, Policy};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

pub const SWITCH_EVENT: &str = "POLICY_SWITCH_CHANGED";
pub const CONFIGURATION_EVENT: &str = "POLICY_CONFIGURATION_CHANGED";

fn insert(
    connection: &Connection,
    event: &str,
    entity: &str,
    payload: &Value,
    now: u64,
) -> JobResult<()> {
    let payload = serde_json::to_string(payload).map_err(|_| JobError::StorageFailure)?;
    connection.execute(
        "INSERT INTO audit_events(id,severity,event_type,entity_id,payload_redacted_json,created_at)
         VALUES(lower(hex(randomblob(16))),'INFO',?1,?2,?3,?4)",
        params![event, entity, payload, now.min(i64::MAX as u64) as i64],
    )?;
    Ok(())
}

fn switch_value(state: &GovernorSwitchState) -> Value {
    json!({
        "enabled": state.enabled,
        "revision": state.revision,
        "updated_at_ms": state.updated_at_ms,
    })
}

pub fn switch(
    connection: &Connection,
    before: &GovernorSwitchState,
    after: &GovernorSwitchState,
    now_ms: u64,
) -> JobResult<()> {
    if before == after {
        return Ok(());
    }
    let key = serde_json::to_value(after.key).map_err(|_| JobError::StorageFailure)?;
    let entity = key.as_str().ok_or(JobError::StorageFailure)?.to_owned();
    insert(
        connection,
        SWITCH_EVENT,
        &entity,
        &json!({"key": key, "before": switch_value(before), "after": switch_value(after)}),
        now_ms,
    )
}

/// The audited projection of a Traffic configuration.
pub fn configuration_value(policy: &Policy) -> Value {
    let mut classes = serde_json::Map::new();
    for class in CLASSES {
        let limits = policy.class(class);
        let rate = policy.start_rate.class(class);
        let name = serde_json::to_value(class)
            .ok()
            .and_then(|value| value.as_str().map(str::to_ascii_lowercase))
            .unwrap_or_default();
        classes.insert(
            name,
            json!({
                "window": limits.window,
                "cpu_millis_estimate": limits.reservation.cpu_millis,
                "memory_bytes_estimate": limits.reservation.memory_bytes,
                "start_tokens_per_minute": rate.tokens_per_minute,
                "start_burst": rate.burst,
            }),
        );
    }
    json!({
        "classes": classes,
        "budget": {"cpu_millis": policy.budget.cpu_millis, "memory_bytes": policy.budget.memory_bytes},
    })
}

pub fn configuration(connection: &Connection, policy: &Policy, now_ms: u64) -> JobResult<()> {
    let current = configuration_value(policy);
    let last: Option<String> = connection
        .query_row(
            "SELECT payload_redacted_json FROM audit_events WHERE event_type=?1
             ORDER BY created_at DESC, rowid DESC LIMIT 1",
            [CONFIGURATION_EVENT],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let before = match last {
        Some(text) => serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|payload| payload.get("after").cloned())
            .ok_or(JobError::StorageFailure)?,
        None => configuration_value(&Policy::default()),
    };
    if before == current {
        return Ok(());
    }
    insert(
        connection,
        CONFIGURATION_EVENT,
        "traffic.configuration",
        &json!({"before": before, "after": current}),
        now_ms,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_ipc::governor::GovernorSwitchKey;

    /// A migrated database in its own directory, removed on drop.
    struct TempDb(std::path::PathBuf, Option<Connection>);
    impl TempDb {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let dir = std::env::temp_dir().join(format!(
                "arc-change-audit-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            let connection = crate::open_and_migrate(dir.join("db")).unwrap();
            Self(dir, Some(connection))
        }
        fn connection(&self) -> &Connection {
            self.1.as_ref().unwrap()
        }
    }
    impl Drop for TempDb {
        fn drop(&mut self) {
            drop(self.1.take());
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn rows(connection: &Connection) -> Vec<(String, String, Value)> {
        connection
            .prepare("SELECT event_type,entity_id,payload_redacted_json FROM audit_events WHERE event_type LIKE 'POLICY_%' ORDER BY rowid")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, serde_json::from_str(&row.get::<_, String>(2)?).unwrap()))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn configuration_is_audited_only_when_it_differs_from_the_last_recorded() {
        let db = TempDb::new();
        let connection = db.connection();
        configuration(connection, &Policy::default(), 1).unwrap();
        assert!(rows(connection).is_empty(), "the default is the baseline");
        let mut tighter = Policy::default();
        tighter.start_rate.build.tokens_per_minute = Some(6);
        tighter.budget.cpu_millis = 4_000;
        configuration(connection, &tighter, 2).unwrap();
        configuration(connection, &tighter, 3).unwrap();
        configuration(connection, &Policy::default(), 4).unwrap();
        let rows = rows(connection);
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert_eq!(rows[0].2["before"], configuration_value(&Policy::default()));
        assert_eq!(
            rows[0].2["after"]["classes"]["build"]["start_tokens_per_minute"],
            6
        );
        assert_eq!(rows[0].2["after"]["budget"]["cpu_millis"], 4_000);
        assert_eq!(rows[1].2["before"], rows[0].2["after"]);
        assert_eq!(rows[1].2["after"], configuration_value(&Policy::default()));
    }

    #[test]
    fn a_switch_change_records_before_and_after() {
        let db = TempDb::new();
        let connection = db.connection();
        let before = GovernorSwitchState::default();
        let after = GovernorSwitchState {
            enabled: false,
            revision: 1,
            updated_at_ms: Some(5),
            key: GovernorSwitchKey::GovernorEnabled,
        };
        switch(connection, &before, &before, 5).unwrap();
        switch(connection, &before, &after, 5).unwrap();
        let rows = rows(connection);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, "governor.enabled");
        assert_eq!(rows[0].2["before"]["enabled"], true);
        assert_eq!(
            rows[0].2["after"],
            json!({"enabled": false, "revision": 1, "updated_at_ms": 5})
        );
    }
}
