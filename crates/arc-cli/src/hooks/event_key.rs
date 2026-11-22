//! Vendor hook event identity on the hook route (AT-TRF-09, ADR-014).
//!
//! A vendor adapter may deliver one PreToolUse event more than once. The hook
//! names the event by the SRS tuple (adapter id, session id, tool call id,
//! event phase), read from `session_id` and `tool_use_id` of the payload, and
//! writes only a digest of it into the managed command it returns:
//! `<arcctl> hook-exec --arc-event <key> -- <literal>`. The option sits
//! directly after `hook-exec`, so a rule matching that prefix still matches.
//!
//! The digest is SHA-256 over `hook_event::DIGEST_DOMAIN` and then, for each
//! field in tuple order, its length as 8 big-endian bytes and its UTF-8
//! bytes, so no two tuples share an encoding. The raw ids never leave this
//! process: they are not written to the command, the wire, SQLite or stderr.
//! The SHA-256 itself is `event_digest::sha256` of the module beside `hooks`
//! (`arc-cli/src/event_digest.rs` in `arcctl`); see that file for why.
//!
//! Only the four whole literals of [`LITERALS`] carry a key. A payload
//! without both ids, and the stage, container and gh rewrites, keep exactly
//! the command they had before.
use super::super::event_digest::sha256;
use arc_ipc::traffic::hook_event::{EventKey, DIGEST_DOMAIN};
use serde_json::Value;

/// Adapter ids of the event tuple.
pub(super) const CLAUDE: &str = "claude";
pub(super) const CURSOR: &str = "cursor";
/// Both adapters deliver the event before the tool runs.
const PHASE: &str = "pre_tool_use";
/// Whole commands whose managed rewrite names its event.
const LITERALS: [&str; 4] = ["cargo build", "bun run build", "cargo test", "bun run test"];

/// The bytes one event tuple is digested from.
fn framed(adapter: &str, session: &str, tool_call: &str, phase: &str) -> Vec<u8> {
    let mut bytes = DIGEST_DOMAIN.to_vec();
    for field in [adapter, session, tool_call, phase] {
        bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
        bytes.extend_from_slice(field.as_bytes());
    }
    bytes
}

/// The digest of one event tuple.
fn digest(adapter: &str, session: &str, tool_call: &str, phase: &str) -> Option<EventKey> {
    let text: String = sha256(&[&framed(adapter, session, tool_call, phase)])?
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    EventKey::try_from(text).ok()
}

/// The key of `event`, or `None` when it does not carry both ids as
/// non-empty strings: such an event has no identity.
fn key(adapter: &str, event: &Value) -> Option<EventKey> {
    let field = |name: &str| event.get(name)?.as_str().filter(|value| !value.is_empty());
    digest(adapter, field("session_id")?, field("tool_use_id")?, PHASE)
}

/// What sits between the quoted `arcctl` and a listed literal for `event`.
#[cfg(test)]
pub(super) fn route(adapter: &str, event: &Value) -> String {
    match key(adapter, event) {
        Some(key) => format!(" hook-exec --arc-event {} -- ", key.as_str()),
        None => " hook-exec -- ".to_owned(),
    }
}

/// `rewritten`, the managed command for `source`, naming the event that
/// asked for it. Anything but a one-child rewrite of a listed literal is
/// returned unchanged.
pub(super) fn keyed(adapter: &str, event: &Value, source: &str, rewritten: String) -> String {
    if !LITERALS.contains(&source) {
        return rewritten;
    }
    let Some(key) = key(adapter, event) else {
        return rewritten;
    };
    match rewritten.strip_suffix(&format!(" hook-exec -- {source}")) {
        Some(arcctl) => format!(
            "{arcctl} hook-exec --arc-event {} -- {source}",
            key.as_str()
        ),
        None => rewritten,
    }
}

#[cfg(test)]
mod tests {
    //! These also run where this source is compiled beside a stand-in
    //! `event_digest` that names no event, so they state what holds for
    //! either: the framing, and the key as the digest of the framed tuple.
    use super::*;
    use serde_json::json;

    const SESSION: &str = "3acb5359-ba96-4012-935e-ec726553d47c";
    const TOOL: &str = "toolu_013uZbuwaEGmySEFAiGDX8op";

    fn event(session: Value, tool: Value) -> Value {
        json!({"session_id":session,"tool_use_id":tool,"hook_event_name":"PreToolUse"})
    }

    fn hex(parts: &[&[u8]]) -> Option<String> {
        sha256(parts).map(|digest| digest.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    #[test]
    fn the_tuple_is_framed_by_its_domain_and_big_endian_field_lengths() {
        let mut expected = b"arc.traffic.hook-event.v1\0".to_vec();
        for field in ["claude", SESSION, TOOL, "pre_tool_use"] {
            expected.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, field.len() as u8]);
            expected.extend_from_slice(field.as_bytes());
        }
        assert_eq!(framed(CLAUDE, SESSION, TOOL, PHASE), expected);
        assert_eq!(
            (CLAUDE, CURSOR, PHASE),
            ("claude", "cursor", "pre_tool_use")
        );
        // Every field changes the bytes, and so does moving bytes between
        // adjacent fields: no two tuples share an encoding.
        let base = framed(CLAUDE, SESSION, TOOL, PHASE);
        for other in [
            framed(CURSOR, SESSION, TOOL, PHASE),
            framed(CLAUDE, "another-session", TOOL, PHASE),
            framed(CLAUDE, SESSION, "toolu_other", PHASE),
            framed(CLAUDE, SESSION, TOOL, "post_tool_use"),
        ] {
            assert_ne!(other, base);
        }
        assert_ne!(framed("a", "bc", "d", "e"), framed("a", "b", "cd", "e"));
        assert_ne!(framed("ab", "", "c", "d"), framed("a", "b", "c", "d"));
        assert_ne!(framed("a", "b", "", "cd"), framed("a", "b", "c", "d"));
    }

    #[test]
    fn the_key_is_the_digest_of_the_framed_tuple_read_from_both_ids() {
        let expected = hex(&[&framed(CLAUDE, SESSION, TOOL, PHASE)]);
        let delivered = event(json!(SESSION), json!(TOOL));
        let key = key(CLAUDE, &delivered).map(|key| key.as_str().to_owned());
        assert_eq!(key, expected);
        // The same event gives the same key, the other adapter another one.
        assert_eq!(
            super::key(CLAUDE, &delivered).map(|key| key.as_str().to_owned()),
            expected
        );
        if let Some(key) = key {
            assert_eq!(key.len(), 64);
            assert_ne!(
                super::key(CURSOR, &delivered).map(|key| key.as_str().to_owned()),
                Some(key.clone())
            );
            // It names no part of either id.
            for raw in [SESSION, TOOL, "toolu_", "3acb5359"] {
                assert!(!key.contains(raw), "{raw}");
            }
        }
    }

    #[test]
    fn an_event_without_both_ids_has_no_key() {
        for incomplete in [
            json!({"tool_use_id":TOOL}),
            json!({"session_id":SESSION}),
            event(json!(""), json!(TOOL)),
            event(json!(SESSION), json!("")),
            event(json!(7), json!(TOOL)),
            event(json!(SESSION), json!(null)),
            event(json!([SESSION]), json!(TOOL)),
            json!({}),
        ] {
            assert_eq!(key(CLAUDE, &incomplete), None, "{incomplete}");
            let today = "'/opt/arc/arcctl' hook-exec -- cargo build".to_owned();
            assert_eq!(
                keyed(CLAUDE, &incomplete, "cargo build", today.clone()),
                today
            );
        }
    }

    #[test]
    fn only_the_four_literals_name_their_event_directly_after_hook_exec() {
        let event = event(json!(SESSION), json!(TOOL));
        let route = route(CLAUDE, &event);
        if let Some(key) = key(CLAUDE, &event) {
            assert_eq!(
                route,
                format!(" hook-exec --arc-event {} -- ", key.as_str())
            );
        }
        for literal in LITERALS {
            let today = format!("'/opt/arc/arc ctl' hook-exec -- {literal}");
            assert_eq!(
                keyed(CLAUDE, &event, literal, today),
                format!("'/opt/arc/arc ctl'{route}{literal}")
            );
        }
        assert_eq!(
            LITERALS,
            ["cargo build", "bun run build", "cargo test", "bun run test"]
        );
        // The stage, container and gh rewrites, and any other text, are kept.
        for (source, today) in [
            (
                "bun test && gh workflow run ci.yml",
                "'/a' hook-exec --arc-request 0123456789abcdef0123456789abcdef -- bun test && '/a' hook-exec --arc-request 0123456789abcdef0123456789abcdef -- gh workflow run ci.yml",
            ),
            ("docker build .", "'/a' hook-exec -- docker build ."),
            (
                "gh workflow run build.yml",
                "case \"$-\" in *) ;; esac; gh workflow run build.yml",
            ),
            ("cargo build", "cargo build"),
            ("cargo build", "'/a' hook-exec -- cargo build --release"),
        ] {
            assert_eq!(keyed(CLAUDE, &event, source, today.to_owned()), today);
        }
    }
}
