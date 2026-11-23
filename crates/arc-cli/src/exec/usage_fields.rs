//! Usage attribution of one `hook-exec` run (lane B, MTR-004, MET-026).
//!
//! The installed hook writes the hidden `--arc-vendor <id>` and
//! `--arc-hook-overhead-ms <n>` after `--arc-event` and before `--`. They are
//! consumed here like `--arc-event`: the child's argv, environment and streams
//! never carry them. Values are sent with this run's registration or enqueue
//! only to a daemon that holds `arc_ipc::meter::usage::ADVERTISEMENT`, so an
//! older daemon and a protocol-1 peer receive the bytes they received before.
//!
//! An enqueue carries the vendor and overhead; its project is the fairness
//! key it already sends. A lifecycle registration (a class Traffic does not
//! admit, such as REMOTE_CI) also carries the project key the fairness gate
//! derives, so every recorded job of a project groups under it. A direct
//! `arcctl exec` run has no vendor or overhead and sends only that key.
//!
//! The hidden `--arc-session <digest>` (lane B's hook session digest) is
//! parsed here as 64 lowercase hex digits and sent as `session_key` only to
//! a daemon that also holds `arc_ipc::meter::usage::ADVERTISEMENT_V2`;
//! otherwise it is dropped and the other fields go out as before.
use arc_ipc::{
    client::Continuity,
    jobs::usage::{HookOverheadMs, SessionKey, UsageFields, Vendor},
    meter::usage::{ADVERTISEMENT, ADVERTISEMENT_V2},
};
use std::{cell::RefCell, path::Path};

thread_local! {
    static USAGE: RefCell<Option<UsageFields>> = const { RefCell::new(None) };
}

/// The parsed hook options. Absent is today's route; a malformed value was
/// not written by the hook and is rejected before any child starts.
pub(super) fn parse(
    vendor: Option<String>,
    overhead_ms: Option<String>,
    session: Option<String>,
) -> Result<Option<UsageFields>, ()> {
    let session_key = session
        .map(|text| SessionKey::try_from(text).map_err(|_| ()))
        .transpose()?;
    let vendor = vendor
        .map(|v| serde_json::from_value::<Vendor>(serde_json::Value::String(v)).map_err(|_| ()))
        .transpose()?;
    let hook_overhead_ms = overhead_ms
        .map(|text| {
            // Decimal digits only: no sign, space or exponent from elsewhere.
            if text.is_empty() || text.len() > 16 || !text.bytes().all(|b| b.is_ascii_digit()) {
                return Err(());
            }
            text.parse::<u64>()
                .map_err(|_| ())
                .and_then(|n| HookOverheadMs::try_from(n).map_err(|_| ()))
        })
        .transpose()?;
    Ok(
        (vendor.is_some() || hook_overhead_ms.is_some() || session_key.is_some()).then_some(
            UsageFields {
                vendor,
                hook_overhead_ms,
                project_key: None,
                session_key,
            },
        ),
    )
}

/// Scope one run to the usage fields it carries.
pub(super) fn with_usage<T>(usage: Option<UsageFields>, run: impl FnOnce() -> T) -> T {
    struct Reset(Option<UsageFields>);
    impl Drop for Reset {
        fn drop(&mut self) {
            USAGE.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let previous = USAGE.with(|slot| slot.replace(usage));
    let _reset = Reset(previous);
    run()
}

fn scoped() -> Option<UsageFields> {
    USAGE.with(|slot| slot.borrow().clone())
}

/// The fields to send with this run's enqueue, or `None` to send none.
/// `daemon` is the peer that answered this run's capability probe; without
/// the advertisement held by that very process nothing is sent.
pub(super) fn negotiated(socket: &Path, daemon: &Continuity) -> Option<UsageFields> {
    let mut usage = scoped()?;
    let daemon = daemon.identity()?;
    if !super::project_key::holds(socket, daemon.pid, ADVERTISEMENT_V2) {
        usage.session_key = None;
    }
    super::project_key::holds(socket, daemon.pid, ADVERTISEMENT).then_some(usage)
}

/// The fields to send with this run's lifecycle registration, or `None`.
/// The registration has no capability probe of its own, so the daemon is
/// the process holding the advertisement beside this run's socket; the
/// project key is sent only when that same process also advertises fairness.
pub(super) fn registration() -> Option<UsageFields> {
    let socket = arc_core::socket_path().ok()?;
    registration_at(&socket, scoped())
}

fn registration_at(socket: &Path, scoped: Option<UsageFields>) -> Option<UsageFields> {
    let daemon = holder(socket, ADVERTISEMENT)?;
    let continuity = Continuity::default();
    continuity.record(arc_core::process_identity::ProcessIdentity::capture(daemon).ok());
    let mut usage = scoped.unwrap_or_default();
    if holder(socket, ADVERTISEMENT_V2) != Some(daemon) {
        usage.session_key = None;
    }
    usage.project_key = super::project_key::negotiated(socket, &continuity);
    (usage != UsageFields::default()).then_some(usage)
}

#[allow(clippy::duplicate_mod)] // Also included by the hook and the Meter reads.
#[path = "../meter/usage/advertiser.rs"]
mod advertiser;
use advertiser::holder;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_options_parse_strictly_and_absent_options_are_todays_route() {
        assert_eq!(parse(None, None, None), Ok(None));
        let parsed = parse(Some("claude-code".into()), Some("12".into()), None)
            .unwrap()
            .unwrap();
        assert_eq!(parsed.vendor, Some(Vendor::ClaudeCode));
        assert_eq!(parsed.hook_overhead_ms.map(HookOverheadMs::get), Some(12));
        assert_eq!(parsed.project_key, None);
        assert_eq!(parsed.session_key, None);
        let key = "ab".repeat(32);
        let session = parse(None, None, Some(key.clone())).unwrap().unwrap();
        assert_eq!(session.session_key.map(String::from), Some(key));
        for invalid in ["AB".repeat(32), "ab".repeat(31), String::new()] {
            assert_eq!(
                parse(None, None, Some(invalid.clone())),
                Err(()),
                "{invalid}"
            );
        }
        assert_eq!(
            parse(Some("cursor".into()), None, None)
                .unwrap()
                .unwrap()
                .vendor,
            Some(Vendor::Cursor)
        );
        assert_eq!(
            parse(None, Some("0".into()), None)
                .unwrap()
                .unwrap()
                .hook_overhead_ms
                .map(HookOverheadMs::get),
            Some(0)
        );
        // The adapter id is not a stored vendor, and only decimal digits count.
        for (vendor, overhead) in [
            (Some("claude"), None),
            (Some("Claude-Code"), None),
            (Some(""), None),
            (None, Some("-1")),
            (None, Some("+1")),
            (None, Some(" 1")),
            (None, Some("1e3")),
            (None, Some("")),
            (None, Some("99999999999999999")),
        ] {
            assert_eq!(
                parse(vendor.map(str::to_owned), overhead.map(str::to_owned), None),
                Err(()),
                "{vendor:?} {overhead:?}"
            );
        }
    }

    #[test]
    fn scoped_fields_reset_after_the_run_and_nothing_is_sent_without_an_advertisement() {
        let fields = parse(Some("cursor".into()), Some("3".into()), None).unwrap();
        let inside = with_usage(fields.clone(), scoped);
        assert_eq!(inside, fields);
        assert_eq!(scoped(), None);
        // No advertisement beside this socket: no registration fields, even
        // with hook fields in scope, and no enqueue fields without a peer.
        let directory = std::env::temp_dir().join(format!("arc-usage-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let socket = directory.join("arc.sock");
        assert_eq!(registration_at(&socket, fields.clone()), None);
        std::fs::write(directory.join(ADVERTISEMENT), b"").unwrap();
        // An unlocked file advertises nothing.
        assert_eq!(holder(&socket, ADVERTISEMENT), None);
        assert_eq!(registration_at(&socket, fields.clone()), None);
        with_usage(fields, || {
            assert_eq!(negotiated(&socket, &Continuity::default()), None);
        });
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
