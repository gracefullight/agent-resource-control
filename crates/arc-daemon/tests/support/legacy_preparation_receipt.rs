//! Validate the exact preparation used by the Rust consumer while leased.
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

pub(super) fn sha256(path: &Path) -> String {
    let mut command = Command::new("/usr/bin/shasum");
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .args(["-a", "256"])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child =
        crate::native::OwnedChild::new(command.spawn().expect("start legacy artifact hash"));
    let output = child.wait_with_output().unwrap_or_else(|error| {
        panic!(
            "legacy artifact hash did not finish for {}: {error}",
            path.display()
        )
    });
    assert!(
        output.status.success(),
        "legacy artifact hash failed: {output:?}"
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

pub(super) fn validate(
    root: &Path,
    directory: &Path,
    id: &str,
    latest: &[u8],
    receipt: &serde_json::Value,
) {
    assert_eq!(
        fs::read(directory.join("receipt.json")).expect("legacy preparation receipt"),
        latest,
        "legacy pointer and preparation receipt differ"
    );
    let canonical_directory = directory
        .canonicalize()
        .expect("legacy preparation directory");
    let base = format!("target/host-meter/legacy/{id}");
    for (reference, suffix) in [
        (&receipt["archive"], "accepted.tar"),
        (&receipt["stdout"], "stdout.log"),
        (&receipt["stderr"], "stderr.log"),
        (&receipt["binary"], "target/debug/arc-daemon"),
        (&receipt["binaries"]["arcctl"], "target/debug/arcctl"),
        (
            &receipt["binaries"]["fixture"],
            "target/debug/arc-exec-fixture",
        ),
    ] {
        let relative = format!("{base}/{suffix}");
        assert_eq!(
            reference["path"].as_str(),
            Some(relative.as_str()),
            "legacy artifact path mismatch"
        );
        let path = root.join(relative);
        let metadata = fs::symlink_metadata(&path).expect("legacy artifact missing");
        assert!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "legacy artifact must be a regular file"
        );
        assert!(
            path.canonicalize()
                .unwrap()
                .starts_with(&canonical_directory),
            "legacy artifact escaped its preparation"
        );
        assert_eq!(
            reference["bytes"].as_u64(),
            Some(metadata.len()),
            "legacy artifact size mismatch"
        );
        assert_eq!(
            reference["sha256"].as_str(),
            Some(sha256(&path).as_str()),
            "legacy artifact hash mismatch"
        );
    }
}
