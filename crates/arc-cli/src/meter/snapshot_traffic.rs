//! `traffic.queue.depth` in `arcctl meter snapshot --json` (lane B, MET-017).
//!
//! With the daemon holding `arc_ipc::meter::usage::SNAPSHOT_TRAFFIC_ADVERTISEMENT`,
//! the snapshot is read with `{"include_traffic":true}` and its `traffic`
//! object is emitted beside the v1 fields, which keep their order and shape.
//! Without the advertisement no such request is sent and the v1 bytes are
//! emitted unchanged. A failed or unverifiable Traffic read also leaves the
//! v1 bytes alone: an absent `traffic` key means not reported, never zero.
use arc_core::meter::MAX_SNAPSHOT_BYTES;
use arc_ipc::{
    client::{exchange_until, Continuity, ExchangeOptions},
    meter::{
        decode_snapshot, snapshot_request,
        usage::{
            SnapshotRequest, SnapshotTraffic, SNAPSHOT_TRAFFIC_ADVERTISEMENT, SNAPSHOT_TRAFFIC_KEY,
        },
    },
};
use serde_json::Value;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(super) fn extend(socket: &Path, snapshot: Vec<u8>) -> Result<Vec<u8>, String> {
    let Some(advertiser) = super::usage::advertiser(socket, SNAPSHOT_TRAFFIC_ADVERTISEMENT) else {
        return Ok(snapshot);
    };
    let mut request = snapshot_request(crate::request_id());
    request.payload = serde_json::json!(SnapshotRequest {
        include_traffic: true
    });
    let continuity = Continuity::default();
    let reply = exchange_until(
        socket,
        &request,
        Instant::now() + Duration::from_secs(2),
        &|| false,
        &mut false,
        ExchangeOptions {
            continuity: Some(&continuity),
            retry_busy: false,
        },
    );
    // Only the advertising daemon's own reply is used.
    let verified = continuity
        .identity()
        .is_some_and(|daemon| daemon.pid == advertiser);
    Ok(match reply {
        Ok(value) if verified => splice(value).unwrap_or(snapshot),
        _ => snapshot,
    })
}

/// The v1 rendering of the reply's snapshot fields followed by `traffic`.
fn splice(mut value: Value) -> Option<Vec<u8>> {
    let traffic = value.as_object_mut()?.remove(SNAPSHOT_TRAFFIC_KEY)?;
    let traffic: SnapshotTraffic = serde_json::from_value(traffic).ok()?;
    let meter = decode_snapshot(&value).ok()?;
    let mut bytes = serde_json::to_vec(&meter).ok()?;
    if bytes.len() > MAX_SNAPSHOT_BYTES || bytes.pop() != Some(b'}') {
        return None;
    }
    bytes.extend_from_slice(format!(",\"{SNAPSHOT_TRAFFIC_KEY}\":").as_bytes());
    bytes.extend_from_slice(&serde_json::to_vec(&traffic).ok()?);
    bytes.push(b'}');
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../../arc-core/tests/fixtures/meter-snapshot.json"
        ))
        .unwrap()
    }

    #[test]
    fn traffic_follows_the_unchanged_v1_fields_and_a_bad_reply_adds_nothing() {
        let v1 = serde_json::to_vec(&decode_snapshot(&fixture()).unwrap()).unwrap();
        let mut reply = fixture();
        reply[SNAPSHOT_TRAFFIC_KEY] = serde_json::json!({"queue_depth":2,"observed_at_ms":5});
        let bytes = splice(reply.clone()).unwrap();
        assert!(bytes.starts_with(&v1[..v1.len() - 1]));
        let parsed: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed["traffic"]["queue_depth"], 2);
        assert_eq!(parsed["traffic"]["observed_at_ms"], 5);
        parsed
            .as_object()
            .unwrap()
            .iter()
            .filter(|(key, _)| *key != SNAPSHOT_TRAFFIC_KEY)
            .for_each(|(key, value)| assert_eq!(*value, fixture()[key], "{key}"));
        assert_eq!(splice(fixture()), None);
        let mut invalid = reply.clone();
        invalid[SNAPSHOT_TRAFFIC_KEY]["queue_depth"] = serde_json::json!(-1);
        assert_eq!(splice(invalid), None);
        let mut extra = reply;
        extra[SNAPSHOT_TRAFFIC_KEY]["source"] = serde_json::json!("x");
        assert_eq!(splice(extra), None);
    }

    #[test]
    fn without_an_advertisement_the_v1_bytes_are_emitted_and_nothing_is_sent() {
        let socket = std::env::temp_dir()
            .join(format!("arc-no-daemon-{}", std::process::id()))
            .join("arc.sock");
        assert_eq!(extend(&socket, b"{}".to_vec()).unwrap(), b"{}".to_vec());
    }
}
