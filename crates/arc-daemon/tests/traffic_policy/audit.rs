//! POL-004 item 3: every policy change the product can make at this snapshot
//! writes one `audit_events` row with its before and after values. The
//! product's changes are the two switches (`policy.set_switch`,
//! `daemon.pause`, `daemon.resume`) and the startup Traffic configuration
//! (class windows and estimates, budgets, start-rate settings). No learned
//! policy value exists, so none is audited or expected.
use super::fixture::*;
use crate::runtime::Fixture;
use arc_ipc::governor::*;
use serde_json::{json, Value};

/// Every policy-change audit row, oldest first: (type, entity, payload).
fn audits(f: &Fixture) -> Vec<(String, String, Value)> {
    database(f)
        .prepare(
            "SELECT event_type,entity_id,payload_redacted_json FROM audit_events
             WHERE event_type LIKE 'POLICY_%' ORDER BY rowid",
        )
        .unwrap()
        .query_map([], |r| {
            let payload: String = r.get(2)?;
            Ok((
                r.get(0)?,
                r.get(1)?,
                serde_json::from_str(&payload).unwrap(),
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn read(f: &Fixture, key: GovernorSwitchKey) -> GovernorSwitchState {
    let query = GetGovernorSwitchRequest {
        key,
        mutation_id: None,
    };
    decode_switch_read_result(&rpc(f, OPERATION_GET_SWITCH, json!(query)), &query)
        .unwrap()
        .switch
}

fn state(switch: &GovernorSwitchState) -> Value {
    json!({"enabled": switch.enabled, "revision": switch.revision,
        "updated_at_ms": switch.updated_at_ms})
}

pub fn every_policy_change_writes_before_and_after_audit() {
    let f = Fixture::new();
    let daemon_default = daemon(&f, &[]);
    assert!(
        audits(&f).is_empty(),
        "an unconfigured daemon changed nothing"
    );

    // The start-rate switch is ON by default and changed through
    // policy.set_switch; the Governor switch through pause and resume.
    let rate = GovernorSwitchKey::StartRateControl;
    let governor = GovernorSwitchKey::GovernorEnabled;
    let default_rate = read(&f, rate);
    assert!(default_rate.enabled && default_rate.revision == 0);
    let mut before = vec![default_rate, read(&f, governor)];
    let off = set(&f, OPERATION_SET_SWITCH, rate, false, 0, 1);
    let paused = set(&f, OPERATION_PAUSE, governor, false, 0, 2);
    let resumed = set(&f, OPERATION_RESUME, governor, true, 1, 3);
    let on = set(&f, OPERATION_SET_SWITCH, rate, true, 1, 4);
    assert_eq!(read(&f, rate), on);
    assert_eq!(read(&f, governor), resumed);
    before.extend([paused.clone(), off.clone()]);
    let changes = [
        ("traffic.start_rate_control", &before[0], &off),
        ("governor.enabled", &before[1], &paused),
        ("governor.enabled", &before[2], &resumed),
        ("traffic.start_rate_control", &before[3], &on),
    ];
    let rows = audits(&f);
    assert_eq!(rows.len(), changes.len(), "{rows:?}");
    for ((event, entity, payload), (key, from, to)) in rows.iter().zip(changes) {
        assert_eq!(
            (event.as_str(), entity.as_str()),
            ("POLICY_SWITCH_CHANGED", key)
        );
        assert_eq!(
            *payload,
            json!({"key": key, "before": state(from), "after": state(to)}),
        );
    }
    stop(&f, daemon_default);

    // A startup configuration that differs from the last recorded one:
    // a CPU budget and a tighter Build start rate and burst. The class
    // windows and estimates are part of the audited values.
    let tight = [
        (CPU_BUDGET, "4000"),
        (START_RATE, "build=6"),
        (START_BURST, "build=1"),
    ];
    for _ in 0..2 {
        // The same configuration again is not a change.
        stop(&f, daemon(&f, &tight));
    }
    let rows = audits(&f);
    assert_eq!(rows.len(), 5, "{rows:?}");
    let (event, entity, payload) = &rows[4];
    assert_eq!(
        (event.as_str(), entity.as_str()),
        ("POLICY_CONFIGURATION_CHANGED", "traffic.configuration")
    );
    let (from, to) = (&payload["before"], &payload["after"]);
    assert_eq!(
        (&from["budget"]["cpu_millis"], &to["budget"]["cpu_millis"]),
        (&2_000.into(), &4_000.into())
    );
    assert_eq!(from["budget"]["memory_bytes"], to["budget"]["memory_bytes"]);
    let (old, new) = (&from["classes"]["build"], &to["classes"]["build"]);
    assert_eq!(
        (
            &old["start_tokens_per_minute"],
            &new["start_tokens_per_minute"]
        ),
        (&120.into(), &6.into())
    );
    assert_eq!(
        (&old["start_burst"], &new["start_burst"]),
        (&120.into(), &1.into())
    );
    for class in ["build", "test", "lint", "typecheck"] {
        for field in ["window", "cpu_millis_estimate", "memory_bytes_estimate"] {
            let value = &from["classes"][class][field];
            assert!(value.is_u64(), "{class}.{field} audited: {payload}");
            assert_eq!(*value, to["classes"][class][field]);
        }
    }
    assert_eq!(from["classes"]["test"], to["classes"]["test"]);

    // Returning to the default is a change back.
    stop(&f, daemon(&f, &[]));
    let rows = audits(&f);
    assert_eq!(rows.len(), 6, "{rows:?}");
    assert_eq!(rows[5].2["before"], *to);
    assert_eq!(rows[5].2["after"], *from);
}
