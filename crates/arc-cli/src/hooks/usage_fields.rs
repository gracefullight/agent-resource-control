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
use super::{event_key, Vendor};
use arc_ipc::{jobs::usage, meter::usage::ADVERTISEMENT};
use serde_json::Value;
use std::time::Instant;

const KEYED: &str = " hook-exec --arc-event ";
const KEY_LEN: usize = 64;

#[allow(clippy::duplicate_mod)] // Also included by `hook-exec` and the Meter reads.
#[path = "../meter/usage/advertiser.rs"]
mod advertiser;

pub(super) fn annotate(vendor: Vendor, started: Instant, response: Value) -> Value {
    let advertised = arc_core::socket_path()
        .ok()
        .and_then(|socket| advertiser::holder(&socket, ADVERTISEMENT))
        .is_some();
    if advertised {
        annotated(vendor, started, response)
    } else {
        response
    }
}

fn annotated(vendor: Vendor, started: Instant, mut response: Value) -> Value {
    let adapter = match vendor {
        Vendor::Claude => event_key::CLAUDE,
        Vendor::Cursor => event_key::CURSOR,
    };
    let Some(stored) = usage::Vendor::from_adapter(adapter) else {
        return response;
    };
    let slot = match vendor {
        Vendor::Claude => response
            .get_mut("hookSpecificOutput")
            .and_then(|v| v.get_mut("updatedInput")),
        Vendor::Cursor => response.get_mut("updated_input"),
    }
    .and_then(|input| input.get_mut("command"));
    let Some(Value::String(command)) = slot else {
        return response;
    };
    let overhead = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    if let Some(annotated) = with_fields(command, stored, overhead) {
        *command = annotated;
    }
    response
}

/// `command` with the two options after its event key, or `None` when it is
/// not exactly `<arcctl> hook-exec --arc-event <64 hex> -- <literal>`.
fn with_fields(command: &str, vendor: usage::Vendor, overhead_ms: u64) -> Option<String> {
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
    Some(format!(
        "{} --arc-vendor {} --arc-hook-overhead-ms {}{rest}",
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
            with_fields(&keyed, usage::Vendor::ClaudeCode, 7).unwrap(),
            format!(
                "'/opt/arc/arc ctl' hook-exec --arc-event {KEY} --arc-vendor claude-code \
                 --arc-hook-overhead-ms 7 -- cargo build"
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
                with_fields(&unchanged, usage::Vendor::Cursor, 1),
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
        let out = annotated(Vendor::Claude, started, claude.clone());
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
        let out = annotated(Vendor::Cursor, started, cursor);
        assert!(out["updated_input"]["command"]
            .as_str()
            .unwrap()
            .contains(" --arc-vendor cursor --arc-hook-overhead-ms "));
        // A response without a keyed command, or another shape, is untouched.
        let plain = json!({"updated_input":{"command":"'/a' hook-exec -- cargo build"}});
        assert_eq!(annotated(Vendor::Cursor, started, plain.clone()), plain);
        let other = json!({"permission":"allow"});
        assert_eq!(annotated(Vendor::Claude, started, other.clone()), other);
    }
}
