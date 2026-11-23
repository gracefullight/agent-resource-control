//! The agent-session digest of one hook event (lane B, G-01 item 3).
//!
//! [`digest`] is SHA-256 over [`DOMAIN`] and then, for the stored vendor id
//! (`claude-code`, `cursor`) and the raw vendor `session_id` in that order,
//! each field's length as 8 big-endian bytes and its UTF-8 bytes, written as
//! 64 lowercase hex digits (`arc_ipc::jobs::usage::SessionKey`). The domain
//! differs from the event-key domain, so a session digest never equals an
//! event key; the vendor is framed in, so the same session text under two
//! vendors gives two digests. `None` means the event names no session (or no
//! digest is available in this build); the rewrite then carries no
//! `--arc-session`. The raw id is read only here; it never reaches the
//! command, the wire, SQLite or a diagnostic.
use super::super::event_digest::sha256;
use super::{event_key, Vendor};
use arc_ipc::jobs::usage::{self, SessionKey};
use serde_json::Value;

/// Domain tag of the session digest; never shared with another digest.
pub(super) const DOMAIN: &[u8] = b"arc.agent-session.v1\0";

/// The bytes one (vendor, session) pair is digested from.
fn framed(vendor: &str, session: &str) -> Vec<u8> {
    let mut bytes = DOMAIN.to_vec();
    for field in [vendor, session] {
        bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
        bytes.extend_from_slice(field.as_bytes());
    }
    bytes
}

fn key(vendor: usage::Vendor, session: &str) -> Option<SessionKey> {
    let text: String = sha256(&[&framed(vendor.as_str(), session)])?
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    SessionKey::try_from(text).ok()
}

/// Called by `usage_fields::annotate` only while the daemon holds
/// `arc_ipc::meter::usage::ADVERTISEMENT_V2`.
pub(super) fn digest(vendor: Vendor, event: &Value) -> Option<SessionKey> {
    let adapter = match vendor {
        Vendor::Claude => event_key::CLAUDE,
        Vendor::Cursor => event_key::CURSOR,
    };
    let stored = usage::Vendor::from_adapter(adapter)?;
    let session = event
        .get("session_id")?
        .as_str()
        .filter(|value| !value.is_empty())?;
    key(stored, session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_digest_is_framed_by_vendor_and_never_carries_the_raw_id() {
        let raw = "3acb5359-ba96-4012-935e-ec726553d47c";
        let event = json!({"session_id": raw, "tool_use_id": "t"});
        if sha256(&[]).is_none() {
            // A copy of the hook compiled without a digest names no session.
            assert_eq!(digest(Vendor::Claude, &event), None);
            return;
        }
        let claude = digest(Vendor::Claude, &event).unwrap();
        let cursor = digest(Vendor::Cursor, &event).unwrap();
        assert_ne!(claude, cursor, "the vendor is part of the digest");
        assert_eq!(claude.as_str().len(), 64);
        assert!(!claude.as_str().contains(raw));
        // Stable for one session, distinct for another.
        assert_eq!(digest(Vendor::Claude, &event), Some(claude.clone()));
        let other = json!({"session_id": "another"});
        assert_ne!(digest(Vendor::Claude, &other), Some(claude.clone()));
        // Length framing: moving a byte across the field boundary changes it.
        assert_ne!(framed("ab", "c"), framed("a", "bc"));
        assert!(framed("claude-code", raw).starts_with(DOMAIN));
        // No session, an empty one or a non-string one names none.
        for missing in [
            json!({}),
            json!({"session_id": ""}),
            json!({"session_id": 7}),
        ] {
            assert_eq!(digest(Vendor::Claude, &missing), None, "{missing}");
        }
        // The debug form is opaque.
        assert_eq!(format!("{claude:?}"), "SessionKey([OPAQUE])");
    }
}
