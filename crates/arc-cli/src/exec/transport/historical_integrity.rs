//! Bind the historical transport and its unchanged tests to their Git blobs.
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::OnceLock,
};

const TRANSPORT_BLOB: &str = "27ac37b19d515804df5cd8dc253023697b60ed0b";
const TESTS_BLOB: &str = "41902733a5c1b741ac2097dcccb1ad7fd439e9ca";

fn blob(bytes: &[u8]) -> String {
    let mut child = Command::new("git")
        .args(["hash-object", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("git is required to verify the historical fixture");
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "historical fixture hash failed");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn historical_blob(source: &str) -> String {
    // Reverse only the module-location changes and the verification call.
    let original = source
        .replace("    super::historical_integrity::verify();\n", "")
        .replace("crate::request_id()", "super::super::request_id()")
        .replace(
            "#[cfg(test)]\n#[path = \"historical_bounded_tests.rs\"]\nmod tests;\n",
            "#[cfg(test)]\nmod access_tests;\n#[cfg(test)]\nmod tests;\n",
        );
    blob(original.as_bytes())
}

fn verify_test_bodies(original: &str, bounded: &str) {
    let marker = "#[test]\nfn unavailable_uncertain_and_idempotent_retry_keep_distinct_decisions()";
    let deadline_marker =
        "#[test]\nfn absolute_deadline_is_shared_and_partial_reads_cannot_renew_it()";
    let next_marker =
        "#[test]\nfn cancellation_is_observed_while_waiting_and_at_expired_admission()";
    let original = original.split_once(marker).unwrap().1;
    let bounded = bounded.split_once(marker).unwrap().1;
    let (original_before, original_deadline) = original.split_once(deadline_marker).unwrap();
    let (bounded_before, bounded_deadline) = bounded.split_once(deadline_marker).unwrap();
    let (original_deadline, original_after) = original_deadline.split_once(next_marker).unwrap();
    let (bounded_deadline, bounded_after) = bounded_deadline.split_once(next_marker).unwrap();
    assert_eq!(bounded_before, original_before);
    assert_eq!(bounded_after, original_after);

    // Normalize only the explicit deadline-test adaptation. Every other byte
    // of that body and all other historical bodies still match the fixture.
    let mut normalized = bounded_deadline.to_owned();
    for (adapted, original) in [
        (
            "    let (received_tx, received_rx) = mpsc::channel();\n",
            "",
        ),
        ("        received_tx.send(()).unwrap();\n", ""),
        (
            "        rx.recv_timeout(Duration::from_secs(60)).unwrap();\n",
            "        rx.recv_timeout(Duration::from_secs(5)).unwrap();\n",
        ),
        (
            "    // Stay below the historical transport's 600 ms per-attempt bound.\n",
            "",
        ),
        (
            "    let deadline = Instant::now() + Duration::from_millis(500);\n",
            "    let deadline = Instant::now() + Duration::from_millis(60);\n",
        ),
        (
            "    received_rx.recv_timeout(Duration::from_secs(30)).unwrap();\n",
            "",
        ),
        ("    assert!(Instant::now() >= deadline);\n", ""),
    ] {
        assert_eq!(normalized.matches(adapted).count(), 1);
        normalized = normalized.replacen(adapted, original, 1);
    }
    assert_eq!(normalized, original_deadline);
}

pub(super) fn verify() {
    static VERIFIED: OnceLock<()> = OnceLock::new();
    VERIFIED.get_or_init(|| {
        assert_eq!(
            historical_blob(include_str!("historical_tests.rs")),
            TRANSPORT_BLOB
        );
        assert_eq!(
            blob(include_bytes!(
                "../../../tests/fixtures/historical_transport_tests.rs"
            )),
            TESTS_BLOB
        );
        verify_test_bodies(
            include_str!("../../../tests/fixtures/historical_transport_tests.rs"),
            include_str!("historical_bounded_tests.rs"),
        );
    });
}

#[test]
fn fixed_historical_transport_and_pinned_tests_match_baseline_hashes() {
    verify();
    let changed = include_str!("historical_tests.rs").replacen(
        "return Err(Failure::Local);",
        "return Err(Failure::Unavailable);",
        1,
    );
    assert_ne!(historical_blob(&changed), TRANSPORT_BLOB);
    assert_eq!(
        super::historical_tests::call(
            std::path::Path::new("/missing/arc.sock"),
            "fixture",
            serde_json::Value::Null
        ),
        Err(())
    );
    let original = include_str!("../../../tests/fixtures/historical_transport_tests.rs");
    let bounded = include_str!("historical_bounded_tests.rs");
    let changed_other = bounded.replacen("Err(Failure::Busy)", "Err(Failure::Local)", 1);
    assert!(std::panic::catch_unwind(|| verify_test_bodies(original, &changed_other)).is_err());
    let changed_deadline = bounded.replacen(
        "call_until(&peer.path, \"REGISTER\", Value::Null, deadline, &|| false),\n        Err(Failure::Uncertain)",
        "call_until(&peer.path, \"REGISTER\", Value::Null, deadline, &|| false),\n        Err(Failure::Local)",
        1,
    );
    assert_ne!(changed_deadline, bounded);
    assert!(std::panic::catch_unwind(|| verify_test_bodies(original, &changed_deadline)).is_err());
}
