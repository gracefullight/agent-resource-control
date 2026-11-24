//! Vendor and hook overhead on the managed command (lane B, MTR-004, MET-026).
//!
//! For a rewrite that already names its event, the hook adds
//! `--arc-vendor <id>` (`claude` maps to `claude-code` through
//! `arc_ipc::jobs::usage::Vendor::from_adapter`) and
//! `--arc-hook-overhead-ms <n>`, its own time from start to writing its
//! response, after `--arc-event <key>` and before `--`. The overhead excludes
//! the vendor's spawn of the hook and the later managed command, so it is not
//! the NFR-003 end-to-end figure. The event key, the rest of the rewrite and
//! the vendor's permission evaluation are unchanged: only the command string
//! of an already keyed rewrite gains the two options, and every other
//! response is returned untouched. No session or tool-call id is read here.
//!
//! The options are written only while a daemon holds
//! `arc_ipc::meter::usage::ADVERTISEMENT` beside this hook's socket, so with
//! an older daemon, or none, the rewrite is exactly today's. `hook-exec`
//! checks that the daemon it reaches holds it before sending anything.
//!
//! While the daemon also holds `arc_ipc::meter::usage::ADVERTISEMENT_V2`, the
//! session digest of `super::session_key::digest` (lane B) follows as
//! `--arc-session <64 hex>` after the overhead. The raw session id is read
//! only there and never leaves this process.
use super::{event_key, Vendor};
use arc_ipc::{
    jobs::usage,
    meter::usage::{ADVERTISEMENT, ADVERTISEMENT_V2, CODEX_ADVERTISEMENT},
};
use serde_json::Value;
use std::time::Instant;

const KEYED: &str = " hook-exec --arc-event ";
const KEY_LEN: usize = 64;

#[allow(clippy::duplicate_mod)] // Also included by `hook-exec` and the Meter reads.
#[path = "../meter/usage/advertiser.rs"]
mod advertiser;

pub(super) fn annotate(vendor: Vendor, event: &Value, started: Instant, response: Value) -> Value {
    let Some(socket) = arc_core::socket_path().ok() else {
        return response;
    };
    let Some(daemon) = advertiser::holder(&socket, ADVERTISEMENT) else {
        return response;
    };
    if matches!(vendor, Vendor::Codex)
        && advertiser::holder(&socket, CODEX_ADVERTISEMENT) != Some(daemon)
    {
        return response;
    }
    let session = (advertiser::holder(&socket, ADVERTISEMENT_V2) == Some(daemon))
        .then(|| super::session_key::digest(vendor, event))
        .flatten();
    annotated(vendor, started, session.as_ref(), response)
}

fn annotated(
    vendor: Vendor,
    started: Instant,
    session: Option<&usage::SessionKey>,
    mut response: Value,
) -> Value {
    let adapter = match vendor {
        Vendor::Claude => event_key::CLAUDE,
        Vendor::Codex => event_key::CODEX,
        Vendor::Cursor => event_key::CURSOR,
    };
    let Some(stored) = usage::Vendor::from_adapter(adapter) else {
        return response;
    };
    let slot = match vendor {
        Vendor::Claude | Vendor::Codex => response
            .get_mut("hookSpecificOutput")
            .and_then(|v| v.get_mut("updatedInput")),
        Vendor::Cursor => response.get_mut("updated_input"),
    }
    .and_then(|input| input.get_mut("command"));
    let Some(Value::String(command)) = slot else {
        return response;
    };
    let overhead = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    if let Some(annotated) = with_fields(command, stored, overhead, session) {
        *command = annotated;
    }
    response
}

/// `command` with the options after its event key, or `None` when it is
/// not exactly `<arcctl> hook-exec --arc-event <64 hex> -- <literal>`.
fn with_fields(
    command: &str,
    vendor: usage::Vendor,
    overhead_ms: u64,
    session: Option<&usage::SessionKey>,
) -> Option<String> {
    let start = command.rfind(KEYED)? + KEYED.len();
    let key = command.get(start..start + KEY_LEN)?;
    if !key
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let rest = command.get(start + KEY_LEN..)?;
    if !rest.starts_with(" -- ") {
        return None;
    }
    let overhead = usage::HookOverheadMs::try_from(overhead_ms).ok()?;
    let session = session.map_or_else(String::new, |key| {
        format!(" --arc-session {}", key.as_str())
    });
    Some(format!(
        "{} --arc-vendor {} --arc-hook-overhead-ms {}{session}{rest}",
        &command[..start + KEY_LEN],
        vendor.as_str(),
        overhead.get()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn only_a_keyed_rewrite_gains_the_vendor_and_overhead_before_the_separator() {
        let keyed = format!("'/opt/arc/arc ctl' hook-exec --arc-event {KEY} -- cargo build");
        assert_eq!(
            with_fields(&keyed, usage::Vendor::ClaudeCode, 7, None).unwrap(),
            format!(
                "'/opt/arc/arc ctl' hook-exec --arc-event {KEY} --arc-vendor claude-code \
                 --arc-hook-overhead-ms 7 -- cargo build"
            )
        );
        let session = usage::SessionKey::try_from("cd".repeat(32)).unwrap();
        assert_eq!(
            with_fields(&keyed, usage::Vendor::Cursor, 2, Some(&session)).unwrap(),
            format!(
                "'/opt/arc/arc ctl' hook-exec --arc-event {KEY} --arc-vendor cursor \
                 --arc-hook-overhead-ms 2 --arc-session {} -- cargo build",
                "cd".repeat(32)
            )
        );
        for unchanged in [
            "'/a' hook-exec -- cargo build".to_owned(),
            "cargo build".to_owned(),
            "case \"$-\" in *) ;; esac; gh workflow run build.yml".to_owned(),
            format!("'/a' hook-exec --arc-event {} -- cargo build", &KEY[1..]),
            format!("'/a' hook-exec --arc-event {}Z -- cargo build", &KEY[1..]),
            format!("'/a' hook-exec --arc-event {KEY} cargo build"),
        ] {
            assert_eq!(
                with_fields(&unchanged, usage::Vendor::Cursor, 1, None),
                None,
                "{unchanged}"
            );
        }
    }

    #[test]
    fn the_vendor_response_shape_is_kept_and_only_its_command_changes() {
        let started = Instant::now();
        let command = format!("'/a' hook-exec --arc-event {KEY} -- cargo test");
        let claude = json!({"hookSpecificOutput":{"hookEventName":"PreToolUse",
            "updatedInput":{"command":command,"description":"d"}}});
        let out = annotated(Vendor::Claude, started, None, claude.clone());
        let mut restored = out.clone();
        restored["hookSpecificOutput"]["updatedInput"]["command"] = json!(command);
        assert_eq!(restored, claude);
        let rewritten = out["hookSpecificOutput"]["updatedInput"]["command"]
            .as_str()
            .unwrap();
        let options = rewritten
            .strip_prefix(&format!(
                "'/a' hook-exec --arc-event {KEY} --arc-vendor claude-code --arc-hook-overhead-ms "
            ))
            .and_then(|rest| rest.strip_suffix(" -- cargo test"))
            .unwrap();
        assert!(options.parse::<u64>().is_ok(), "{rewritten}");
        let cursor = json!({"updated_input":{"command":command}});
        let out = annotated(Vendor::Cursor, started, None, cursor);
        assert!(out["updated_input"]["command"]
            .as_str()
            .unwrap()
            .contains(" --arc-vendor cursor --arc-hook-overhead-ms "));
        // A response without a keyed command, or another shape, is untouched.
        let plain = json!({"updated_input":{"command":"'/a' hook-exec -- cargo build"}});
        assert_eq!(
            annotated(Vendor::Cursor, started, None, plain.clone()),
            plain
        );
        let other = json!({"permission":"allow"});
        assert_eq!(
            annotated(Vendor::Claude, started, None, other.clone()),
            other
        );
    }
}
