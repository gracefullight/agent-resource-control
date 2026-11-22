//! Negotiation carrier for vendor hook event identity (AT-TRF-09, ADR-014).
//!
//! A vendor adapter may deliver one PreToolUse event more than once. The
//! installed hook names the event by an [`EventKey`], `hook-exec` sends it in
//! the optional `event` field of `JobRequest::Enqueue`, and the daemon answers
//! a key it has already recorded with `JobResult::DuplicateDelivery` naming
//! the existing job, creating no job row, queue entry or reservation.
//!
//! The capability gates exactly those two additions. A client sends the key
//! only to a daemon that advertised [`CAPABILITY`]; an absent key is omitted,
//! so protocol-1 bytes are unchanged, and a daemon sends the duplicate reply
//! only to an enqueue that carried a key. There is no probe operation: the
//! daemon advertises by holding [`ADVERTISEMENT`] for its whole lifetime
//! (`arc-daemon` `lifecycle.rs`), and the decision is made by
//! `arc_storage::hook_event` inside the admission transaction.
use serde::{Deserialize, Serialize};
use std::fmt;

pub const CAPABILITY: &str = "traffic.hook-event.v1";
/// The daemon's standing advertisement of [`CAPABILITY`]: a file beside its
/// socket on which it holds a POSIX write lock for its whole lifetime, read
/// by an authenticated client with `F_GETLK` exactly as
/// `fairness::ADVERTISEMENT` is. The file holds no data.
pub const ADVERTISEMENT: &str = ".traffic-hook-event-v1";
/// Separates the event digest from any other digest of the same text.
pub const DIGEST_DOMAIN: &[u8] = b"arc.traffic.hook-event.v1\0";

/// An opaque vendor hook event identity: 64 lowercase hexadecimal digits of a
/// domain-separated SHA-256 digest, computed by the installed hook, of the
/// tuple (adapter id, session id, tool call id, event phase). The raw session
/// and tool call ids never cross the wire, enter SQLite or reach diagnostics.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EventKey(String);

impl EventKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for EventKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EventKey([OPAQUE])")
    }
}
impl TryFrom<String> for EventKey {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(value))
        } else {
            Err("invalid event key")
        }
    }
}
impl From<EventKey> for String {
    fn from(value: EventKey) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        jobs::JobId,
        traffic::{decode_job, JobRequest, JobResult},
    };
    use serde_json::{json, Value};

    const ID: &str = "0123456789abcdef0123456789abcdef";
    const KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    /// The exact bytes a protocol-1 client writes today, in field order.
    const PROTOCOL_ONE_BYTES: &str = concat!(
        r#"{"action":"ENQUEUE","job_id":"0123456789abcdef0123456789abcdef","#,
        r#""observer":{"pid":1,"start_token":"1","boot_id":"01234567-89ab-cdef-0123-456789abcdef"},"#,
        r#""classification":{"classification":"BUILD","locality":"HOST_LOCAL","#,
        r#""reason":"CARGO_BUILD","uncertainty":"NONE"},"lineage_context":null}"#
    );

    #[test]
    fn absent_event_leaves_the_enqueue_request_bytes_unchanged() {
        let wire: Value = serde_json::from_str(PROTOCOL_ONE_BYTES).unwrap();
        let request = decode_job(&wire).unwrap();
        let JobRequest::Enqueue { event, project, .. } = &request else {
            panic!("{request:?}");
        };
        assert_eq!((event, project), (&None, &None));
        assert_eq!(serde_json::to_string(&request).unwrap(), PROTOCOL_ONE_BYTES);
    }

    #[test]
    fn event_key_is_an_opaque_digest_and_round_trips_on_enqueue() {
        let mut wire: Value = serde_json::from_str(PROTOCOL_ONE_BYTES).unwrap();
        wire["event"] = json!(KEY);
        let request = decode_job(&wire).unwrap();
        let JobRequest::Enqueue { event, .. } = &request else {
            panic!("{request:?}");
        };
        assert_eq!(event.as_ref().map(EventKey::as_str), Some(KEY));
        assert_eq!(json!(request), wire);
        assert_eq!(
            format!("{:?}", event.as_ref().unwrap()),
            "EventKey([OPAQUE])"
        );
        for invalid in [
            json!("session-1:toolu_01"),
            json!(KEY.to_uppercase()),
            json!(&KEY[..63]),
            json!(format!("{KEY}0")),
            json!(""),
            json!(7),
        ] {
            let mut wire: Value = serde_json::from_str(PROTOCOL_ONE_BYTES).unwrap();
            wire["event"] = invalid.clone();
            assert!(decode_job(&wire).is_err(), "{invalid}");
        }
    }

    #[test]
    fn duplicate_delivery_reply_names_only_the_existing_job() {
        let reply = JobResult::DuplicateDelivery {
            job_id: JobId::try_from(ID.to_owned()).unwrap(),
        };
        assert!(reply.is_valid());
        let wire = json!({"state":"DUPLICATE_DELIVERY","job_id":ID});
        assert_eq!(json!(reply), wire);
        assert_eq!(serde_json::from_value::<JobResult>(wire).unwrap(), reply);
        for invalid in [
            json!({"state":"DUPLICATE_DELIVERY"}),
            json!({"state":"DUPLICATE_DELIVERY","job_id":"short"}),
            json!({"state":"DUPLICATE_DELIVERY","job_id":ID,"event":KEY}),
        ] {
            assert!(
                serde_json::from_value::<JobResult>(invalid.clone()).is_err(),
                "{invalid}"
            );
        }
    }

    #[test]
    fn capability_and_advertisement_names_are_fixed() {
        assert_eq!(CAPABILITY, "traffic.hook-event.v1");
        assert_eq!(ADVERTISEMENT, ".traffic-hook-event-v1");
        assert_ne!(ADVERTISEMENT, crate::traffic::fairness::ADVERTISEMENT);
        assert_eq!(DIGEST_DOMAIN.last(), Some(&0));
    }
}
