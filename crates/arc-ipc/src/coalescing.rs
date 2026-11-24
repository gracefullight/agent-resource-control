//! Cursor-based delivery of the latest replaceable Meter event.
//!
//! The daemon retains one snapshot, independent of the number or speed of
//! consumers. Clients retain the cursor and poll when ready; audit events use
//! their durable database routes and never enter this replaceable slot.
use crate::{Request, ERROR_INVALID_REQUEST};
use arc_core::{
    meter::{MeterSnapshot, MAX_SAFE_INTEGER},
    process_identity::ProcessIdentity,
};
use serde::{Deserialize, Serialize};

pub const OPERATION: &str = "meter.events.latest";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cursor {
    pub daemon_instance: ProcessIdentity,
    pub sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Poll {
    pub after: Option<Cursor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Latest {
    /// Number of replaceable events available to this cursor: zero or one.
    pub pending: u8,
    pub snapshot: Option<MeterSnapshot>,
}

pub fn request(id: impl Into<String>, after: Option<Cursor>) -> Request {
    let mut request = Request::new(id, OPERATION);
    request.payload = serde_json::to_value(Poll { after }).expect("serializable cursor");
    request
}

pub fn decode(value: &serde_json::Value) -> Result<Poll, &'static str> {
    let poll: Poll = serde_json::from_value(value.clone()).map_err(|_| ERROR_INVALID_REQUEST)?;
    if poll.after.as_ref().is_some_and(|cursor| {
        !cursor.daemon_instance.is_valid() || cursor.sequence > MAX_SAFE_INTEGER
    }) {
        return Err(ERROR_INVALID_REQUEST);
    }
    Ok(poll)
}

/// Validate both the one-slot count and the Meter payload from a peer.
pub fn decode_latest(value: &serde_json::Value) -> Result<Latest, &'static str> {
    let reply: Latest = serde_json::from_value(value.clone()).map_err(|_| ERROR_INVALID_REQUEST)?;
    match (reply.pending, &reply.snapshot) {
        (0, None) => Ok(reply),
        (1, Some(_)) if crate::meter::decode_snapshot(&value["snapshot"]).is_ok() => Ok(reply),
        _ => Err(ERROR_INVALID_REQUEST),
    }
}

pub fn latest(poll: &Poll, snapshot: MeterSnapshot) -> Latest {
    let changed = poll.after.as_ref().is_none_or(|cursor| {
        cursor.daemon_instance != snapshot.daemon_instance || cursor.sequence < snapshot.sequence
    });
    Latest {
        pending: u8::from(changed),
        snapshot: changed.then_some(snapshot),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cursor_contract_is_bounded_and_survives_daemon_replacement() {
        let sample: MeterSnapshot = serde_json::from_str(include_str!(
            "../../arc-core/tests/fixtures/meter-snapshot.json"
        ))
        .unwrap();
        let cursor = Cursor {
            daemon_instance: sample.daemon_instance.clone(),
            sequence: sample.sequence,
        };
        let request = request("latest", Some(cursor.clone()));
        let poll = decode(&request.payload).unwrap();
        let current = latest(&poll, sample.clone());
        assert_eq!(current.pending, 0);
        assert!(current.snapshot.is_none());
        assert!(decode_latest(&serde_json::to_value(&current).unwrap()).is_ok());
        let mut newer = sample.clone();
        newer.sequence += 1;
        let event = latest(&poll, newer.clone());
        assert_eq!(event.pending, 1);
        assert!(decode_latest(&serde_json::to_value(&event).unwrap()).is_ok());
        assert_eq!(event.snapshot.unwrap(), newer);
        let mut old_identity = cursor;
        old_identity.daemon_instance.start_token = "42".into();
        if old_identity.daemon_instance == sample.daemon_instance {
            old_identity.daemon_instance.start_token = "43".into();
        }
        old_identity.sequence = MAX_SAFE_INTEGER;
        let event = latest(
            &Poll {
                after: Some(old_identity),
            },
            sample.clone(),
        );
        assert_eq!(
            event.pending, 1,
            "new daemon is never hidden by an old sequence"
        );
        assert_eq!(event.snapshot.unwrap(), sample);
        assert_eq!(
            crate::dispatch(&request, || panic!("no database"))
                .error
                .unwrap()
                .code,
            crate::ERROR_UNSUPPORTED_CAPABILITY
        );
        for value in [
            json!(null),
            json!({"after":null,"extra":1}),
            json!({"after":{"daemon_instance":{"pid":0,"start_token":"1","boot_id":"00000000-0000-0000-0000-000000000000"},"sequence":1}}),
            json!({"after":{"daemon_instance":sample.daemon_instance,"sequence":MAX_SAFE_INTEGER+1}}),
        ] {
            assert!(decode(&value).is_err(), "{value}");
        }
        assert!(decode(&json!({"after":null})).is_ok());
        assert!(decode(&json!({})).is_ok());
        for invalid in [
            json!({"pending":2,"snapshot":null}),
            json!({"pending":1,"snapshot":null}),
            json!({"pending":0,"snapshot":sample}),
            json!({"pending":1,"snapshot":{}}),
        ] {
            assert!(decode_latest(&invalid).is_err(), "{invalid}");
        }
    }
}
