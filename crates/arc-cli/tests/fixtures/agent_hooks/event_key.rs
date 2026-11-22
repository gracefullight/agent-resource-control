//! The vendor hook event key a test expects in a returned command, computed
//! here independently of the hook (AT-TRF-09, ADR-014): SHA-256 over the
//! domain and, for each of adapter id, `session_id`, `tool_use_id` and the
//! phase, its length as 8 big-endian bytes and its bytes.
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The key of `event` as `adapter` (`claude` or `cursor`) delivers it, or
/// `None` when the event does not carry both ids as non-empty strings.
pub fn expected(adapter: &str, event: &Value) -> Option<String> {
    let field = |name: &str| event.get(name)?.as_str().filter(|value| !value.is_empty());
    let (session, tool_call) = (field("session_id")?, field("tool_use_id")?);
    let mut digest = Sha256::new();
    digest.update(b"arc.traffic.hook-event.v1\0");
    for field in [adapter, session, tool_call, "pre_tool_use"] {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field.as_bytes());
    }
    Some(
        digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

/// What the hook writes between its quoted `arcctl` and one of the four
/// managed literals (`cargo build`, `bun run build`, `cargo test`, `bun run
/// test`) for `event`: the key directly after `hook-exec` when the event has
/// an identity, and exactly the earlier text when it has none.
pub fn route(adapter: &str, event: &Value) -> String {
    match expected(adapter, event) {
        Some(key) => format!(" hook-exec --arc-event {key} -- "),
        None => " hook-exec -- ".to_owned(),
    }
}
