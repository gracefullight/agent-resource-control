//! A slow client retains a cursor while the real daemon replaces its one
//! Meter event; a second client observes the same production event route.
use super::fixture::*;
use crate::runtime::Fixture;
use arc_ipc::{
    coalescing::{self, Cursor, Latest},
    governor::*,
};
use serde_json::{json, Value};
use std::{
    thread,
    time::{Duration, Instant},
};

fn poll(f: &Fixture, after: Option<Cursor>) -> Latest {
    let request = coalescing::request("slow-consumer", after);
    coalescing::decode_latest(&rpc(f, &request.operation, request.payload)).unwrap()
}

pub fn slow_consumer_coalesces_latest_and_retains_audit() {
    let f = Fixture::new();
    let daemon = daemon(&f, &[]);
    crate::journey::eventually("first Meter publication", || {
        arc_ipc::meter::snapshot_until(
            &f.socket,
            "initial-meter",
            Instant::now() + Duration::from_secs(2),
            &|| false,
        )
        .is_ok()
    });
    let first = poll(&f, None);
    assert_eq!(first.pending, 1);
    let first = first.snapshot.unwrap();
    let cursor = Cursor {
        daemon_instance: first.daemon_instance.clone(),
        sequence: first.sequence,
    };
    let empty = poll(&f, Some(cursor.clone()));
    if empty.pending == 0 {
        assert!(empty.snapshot.is_none());
    } else {
        assert_eq!(empty.pending, 1);
        assert!(empty.snapshot.unwrap().sequence > cursor.sequence);
    }

    // Distinct durable events are written through the normal policy routes,
    // independently of the paused consumer's replaceable event cursor.
    let rate = GovernorSwitchKey::StartRateControl;
    let governor = GovernorSwitchKey::GovernorEnabled;
    set(&f, OPERATION_SET_SWITCH, rate, false, 0, 1);
    set(&f, OPERATION_PAUSE, governor, false, 0, 2);
    set(&f, OPERATION_RESUME, governor, true, 1, 3);
    set(&f, OPERATION_SET_SWITCH, rate, true, 1, 4);
    let db = database(&f);
    let audits = || -> Vec<(String, Value)> {
        db.prepare("SELECT entity_id,payload_redacted_json FROM audit_events WHERE event_type='POLICY_SWITCH_CHANGED' ORDER BY rowid")
            .unwrap().query_map([], |row| {
                let payload: String = row.get(1)?;
                Ok((row.get(0)?, serde_json::from_str(&payload).unwrap()))
            }).unwrap().map(Result::unwrap).collect()
    };
    let before = audits();
    assert_eq!(before.len(), 4);
    assert_eq!(
        before
            .iter()
            .map(|(entity, _)| entity.as_str())
            .collect::<Vec<_>>(),
        [
            "traffic.start_rate_control",
            "governor.enabled",
            "governor.enabled",
            "traffic.start_rate_control"
        ]
    );
    for (index, (_, payload)) in before.iter().enumerate() {
        assert_eq!(
            payload["after"]["revision"],
            json!(if index < 2 { 1 } else { 2 })
        );
    }

    let end = Instant::now() + Duration::from_secs(90);
    let mut sequences = Vec::new();
    let mut last = cursor.sequence;
    while sequences.len() < 3 {
        let observed = poll(&f, Some(cursor.clone()));
        if observed.pending == 1 {
            let sample = observed.snapshot.unwrap();
            assert_eq!(sample.daemon_instance, cursor.daemon_instance);
            if sample.sequence > last {
                last = sample.sequence;
                sequences.push(last);
            }
        } else {
            assert!(observed.snapshot.is_none());
        }
        assert!(
            Instant::now() < end,
            "three Meter publications: 90 s watchdog"
        );
        if sequences.len() < 3 {
            thread::sleep(Duration::from_millis(100));
        }
    }
    assert!(sequences.windows(2).all(|pair| pair[0] < pair[1]));
    // The paused consumer now asks for its event. Exactly one is available;
    // it is at least the latest publication seen by the other client.
    let resumed = poll(&f, Some(cursor));
    assert_eq!(resumed.pending, 1);
    let delivered = resumed.snapshot.unwrap();
    assert!(delivered.sequence >= last);
    let consumed = Cursor {
        daemon_instance: delivered.daemon_instance,
        sequence: delivered.sequence,
    };
    let after = poll(&f, Some(consumed));
    if after.pending == 0 {
        assert!(after.snapshot.is_none());
    } else {
        assert_eq!(after.pending, 1);
        assert!(after.snapshot.unwrap().sequence > delivered.sequence);
    }
    assert_eq!(
        audits(),
        before,
        "every policy audit event survives coalescing"
    );
    drop(db);
    stop(&f, daemon);
}
