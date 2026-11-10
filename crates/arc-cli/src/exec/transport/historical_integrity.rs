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
            "#[cfg(test)]\n#[path = \"tests.rs\"]\nmod tests;\n",
            "#[cfg(test)]\nmod access_tests;\n#[cfg(test)]\nmod tests;\n",
        );
    blob(original.as_bytes())
}

pub(super) fn verify() {
    static VERIFIED: OnceLock<()> = OnceLock::new();
    VERIFIED.get_or_init(|| {
        assert_eq!(
            historical_blob(include_str!("historical_tests.rs")),
            TRANSPORT_BLOB
        );
        assert_eq!(blob(include_bytes!("tests.rs")), TESTS_BLOB);
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
}
