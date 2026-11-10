//! Exactly two incompatible historical assertions retain their accepted binary.
use std::{path::PathBuf, process::Command, sync::OnceLock};
fn binary() -> &'static PathBuf {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| {
        let output = Command::new("bun")
            .arg("scripts/harness/exec-fail-open-history.ts")
            .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .output()
            .expect("historical preparation process");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
    })
}
fn route(command: Command, site: &str) -> Command {
    let mut routed = Command::new(binary());
    routed.args(command.get_args());
    for (key, value) in command.get_envs() {
        match value {
            Some(value) => {
                routed.env(key, value);
            }
            None => {
                routed.env_remove(key);
            }
        }
    }
    if let Some(cwd) = command.get_current_dir() {
        routed.current_dir(cwd);
    }
    // Synthetic fixture argv only; no product arguments or environment recorded.
    let digest = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(binary())
        .output()
        .unwrap();
    assert!(digest.status.success());
    let binary_hash = String::from_utf8(digest.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();
    let record = serde_json::json!({"kind":"historical-route", "site":site,
        "test":std::thread::current().name(), "program":binary(),
        "argc":routed.get_args().count(), "childPid":null, "programSha256":binary_hash,
        "argumentByteLengths":routed.get_args().map(|s| s.as_encoded_bytes().len()).collect::<Vec<_>>(),
        "suffixArgc":if site == "after-shutdown" { 3 } else { 0 },
        "recordedUnixMs":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64,
        "reason":"adapter constructs command; unchanged test body observes its output and exit"});
    use std::io::Write;
    let path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/exec-fail-open/history/routes.jsonl"
    ));
    writeln!(
        std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .unwrap(),
        "{record}"
    )
    .unwrap();
    routed
}
pub fn before_register(command: Command, mode: &str) -> Command {
    if mode == "before-register" {
        route(command, "before-register")
    } else {
        command
    }
}
pub fn after_shutdown(command: Command) -> Command {
    route(command, "after-shutdown")
}

pub fn negative_control_binary() -> &'static PathBuf {
    binary()
}
