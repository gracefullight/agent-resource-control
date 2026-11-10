//! Compare a complete synthetic native environment without reading host secrets.
use super::{
    cli::{command, restart},
    fail_open::{arc_stderr, receipt},
    fault::{Fault, MODES},
    runtime::Fixture,
    support,
};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::Path,
    process::{Command, Stdio},
};
const RUNTIME_MARKER: &str = "__LLVM_PROFILE_RT_INIT_ONCE";
const SCRIPT: &str = r#"import json,os,sys
with open(sys.argv[1],'a') as marker:
    marker.write(str(os.getpid())+'\n')
print(json.dumps(sorted([[list(k),list(v)] for k,v in os.environb.items()])))
sys.exit(17)
"#;
type Environment = BTreeMap<OsString, OsString>;
fn synthetic_environment(socket: &Path) -> Environment {
    let mut input = Environment::from([
        (arc_core::ARC_SOCKET_PATH.into(), socket.as_os_str().into()),
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("EMPTY".into(), "".into()),
        ("MULTILINE".into(), "first\nsecond".into()),
        ("UNICODE".into(), "한글".into()),
        (
            OsString::from_vec(b"RAW\xff".to_vec()),
            OsString::from_vec(vec![0xfe, b' ', b'|']),
        ),
    ]);
    // LLVM 22's Darwin initializer sets this before main, even without ARC.
    // runtime_environment_probe.rs reproduces this with pinned rustc alone.
    // When that runtime is active, define the same literal input for BOTH
    // commands and require it in BOTH outputs. No observed key is discarded.
    // Ordinary native execution retains the strict, unseeded original input.
    if let Some(value) = std::env::var_os(RUNTIME_MARKER) {
        assert_eq!(value, RUNTIME_MARKER, "unexpected profiling runtime input");
        input.insert(RUNTIME_MARKER.into(), RUNTIME_MARKER.into());
    }
    input
}
fn observe(
    mut c: Command,
    input: &Environment,
    marker: &Path,
    cwd: &Path,
    wrapped: bool,
) -> Vec<u8> {
    let out = c
        .args(["-c", SCRIPT])
        .arg(marker)
        .env_clear()
        .envs(input)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(17));
    assert!(if wrapped {
        arc_stderr(&out.stderr)
    } else {
        out.stderr
    }
    .is_empty());
    assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
    let entries: BTreeMap<Vec<u8>, Vec<u8>> =
        serde_json::from_slice::<Vec<(Vec<u8>, Vec<u8>)>>(&out.stdout)
            .unwrap()
            .into_iter()
            .collect();
    for (key, value) in input {
        assert_eq!(
            entries.get(key.as_bytes()),
            Some(&value.as_bytes().to_vec())
        );
    }
    out.stdout
}
fn compare_complete_environment(expected: &[u8], actual: &[u8]) {
    assert_eq!(expected, actual, "complete native environment changed");
}
#[test]
fn every_outage_preserves_the_complete_native_environment_byte_for_byte() {
    let _serial = support::serial();
    let mut observations = Vec::new();
    for mode in MODES {
        let f = Fixture::new();
        let mut daemon = f.spawn("test");
        let fault = Fault::new(&f, mode);
        let input = synthetic_environment(&fault.path);
        let mut results = Vec::new();
        for wrapped in [false, true] {
            let marker = f.root.join(format!("env-{wrapped}.calls"));
            let c = if wrapped {
                let mut c = command(&f);
                c.args(["exec", "--", "/usr/bin/python3"]);
                c
            } else {
                Command::new("/usr/bin/python3")
            };
            results.push(observe(c, &input, &marker, &f.root, wrapped));
        }
        compare_complete_environment(&results[0], &results[1]);
        observations.push(serde_json::json!({
            "mode":mode,
            "inputBytes":input.iter().map(|(k,v)| (k.as_bytes(),v.as_bytes())).collect::<Vec<_>>(),
            "directExpectedBytes":serde_json::from_slice::<serde_json::Value>(&results[0]).unwrap(),
            "wrappedObservedBytes":serde_json::from_slice::<serde_json::Value>(&results[1]).unwrap(),
            "exit":17,"invocationsPerCommand":1,
        }));
        fault.finish();
        daemon.stop(libc::SIGTERM);
        f.report();
        let mut recovered = restart(&f);
        for _ in 0..2 {
            assert!(command(&f)
                .args(["job", "status"])
                .output()
                .unwrap()
                .status
                .success());
        }
        for wrapped in [false, true] {
            let marker = f.root.join(format!("env-{wrapped}.calls"));
            assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
        }
        recovered.stop(libc::SIGTERM);
        f.report();
    }
    receipt(
        "environment",
        serde_json::json!({"scenario":"environment","passed":true,"invocationsPerCommand":1,
            "completeSyntheticEnvironment":true,"hostEnvironmentRead":false,
            "runtimeMarkerInput":std::env::var_os(RUNTIME_MARKER).is_some(),
            "recoveryInvocationsPerCommand":1,"observations":observations}),
    );
}
#[test]
fn complete_environment_comparison_rejects_added_removed_and_changed_entries() {
    let _serial = support::serial();
    let f = Fixture::new();
    let input = synthetic_environment(&f.socket);
    let baseline = observe(
        Command::new("/usr/bin/python3"),
        &input,
        &f.root.join("baseline.calls"),
        &f.root,
        false,
    );
    let raw = OsString::from_vec(b"RAW\xff".to_vec());
    let changes: Vec<(&str, OsString, Option<OsString>)> = vec![
        ("added-empty", "EXTRA".into(), Some("".into())),
        (
            "added-native",
            OsString::from_vec(b"EXTRA\xfe".to_vec()),
            Some(raw.clone()),
        ),
        ("removed-native", raw.clone(), None),
        (
            "changed-native",
            raw,
            Some(OsString::from_vec(vec![0xfd, b'\n'])),
        ),
        ("removed-empty", "EMPTY".into(), None),
        ("changed-empty", "EMPTY".into(), Some("not-empty".into())),
        ("removed-multiline", "MULTILINE".into(), None),
        (
            "changed-multiline",
            "MULTILINE".into(),
            Some("first\nchanged".into()),
        ),
    ];
    for (name, key, value) in changes {
        let mut altered = input.clone();
        if let Some(value) = value {
            altered.insert(key, value);
        } else {
            assert!(altered.remove(&key).is_some());
        }
        let actual = observe(
            Command::new("/usr/bin/python3"),
            &altered,
            &f.root.join(format!("{name}.calls")),
            &f.root,
            false,
        );
        assert!(
            std::panic::catch_unwind(|| compare_complete_environment(&baseline, &actual)).is_err(),
            "comparison accepted {name}"
        );
    }
    receipt(
        "environment-negative-controls",
        serde_json::json!({
            "passed":true,"cases":8,"comparison":"same complete stdout byte equality as outage test",
            "added":true,"removed":true,"changed":true,"nonUtf8":true,"empty":true,"multiline":true,
        }),
    );
}
