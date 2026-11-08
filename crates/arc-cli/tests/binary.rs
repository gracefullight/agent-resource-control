use std::{env, path::Path, process::Command};

#[test]
fn arcctl_binary_is_built_next_to_the_test_harness() {
    let path = env!("CARGO_BIN_EXE_arcctl");
    assert!(Path::new(path).is_file(), "missing arcctl binary at {path}");
}

#[test]
fn no_arguments_names_both_commands_and_exits_nonzero() {
    let output = Command::new(env!("CARGO_BIN_EXE_arcctl")).output().unwrap();
    assert!(!output.status.success(), "{output:?}");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for expected in ["status", "doctor"] {
        assert!(
            combined.contains(expected),
            "missing {expected:?} in {combined}"
        );
    }
    assert!(!combined.contains("\u{1b}["), "colored output: {combined}");
}

#[test]
fn legacy_usage_errors_and_version_keep_their_exit_contract() {
    for args in [&["unknown-operation"][..], &["status", "--unknown"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_arcctl"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
        assert!(!output.stderr.contains(&27));
    }
    let output = Command::new(env!("CARGO_BIN_EXE_arcctl"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"arcctl 0.1.0\n");
}
