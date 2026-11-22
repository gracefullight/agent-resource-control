//! Derived hook payloads and an offline gh stub, with a real daemon permit.
use super::{cli::binary, protocol, runtime::Fixture, support};
use arc_ipc::{
    jobs::{JobId, ProcessIdentity},
    traffic::{JobRequest, JobResult, OPERATION_TRAFFIC_JOB},
};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

#[allow(clippy::duplicate_mod)]
#[path = "../support/child_watchdog.rs"]
mod child_watchdog;

const COMMAND: &str = "gh workflow run build.yml";
const CLAUDE: &str = include_str!(
    "../../../arc-cli/tests/fixtures/agent_hooks/claude-recorded-2.1.327-cargo-build.json"
);
const CURSOR: &str = include_str!(
    "../../../arc-cli/tests/fixtures/agent_hooks/cursor/recorded/preToolUse-cargo-build-rewrite.json"
);

fn hook(fixture: &Fixture, vendor: &str, event: &Value, socket: &Path) -> Output {
    let mut command = Command::new(binary("arcctl"));
    command
        .args(["hooks", "run", vendor])
        .env_clear()
        .env("PATH", &fixture.root)
        .env("HOME", &fixture.root)
        .env(arc_core::ARC_SOCKET_PATH, socket)
        .current_dir(&fixture.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let mut child = super::runtime::OwnedChild::new(command.spawn().unwrap());
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(event).unwrap())
        .unwrap();
    child
        .wait_with_output()
        .expect("built hook child and streams exceeded 60s watchdog")
}

fn traffic(fixture: &Fixture, request: &JobRequest) -> JobResult {
    let response = protocol::call(fixture, OPERATION_TRAFFIC_JOB, json!(request));
    assert_eq!(response.status, "ok", "{response:?}");
    serde_json::from_value(response.result.unwrap()).unwrap()
}

// Include total historical rows as well as active reservations: a transient
// local queue entry or a newly released permit must not escape observation.
fn scheduling(fixture: &Fixture) -> (i64, i64, i64, i64, i64, i64) {
    let db = rusqlite::Connection::open(&fixture.database).unwrap();
    db.query_row(
        "SELECT (SELECT count(*) FROM traffic_entries), \
         (SELECT count(*) FROM traffic_entries WHERE state='QUEUED'), \
         count(*), coalesce(sum(state='ACTIVE'),0), \
         coalesce(sum(CASE WHEN state='ACTIVE' THEN cpu_millis_est ELSE 0 END),0), \
         coalesce(sum(CASE WHEN state='ACTIVE' THEN memory_bytes_est ELSE 0 END),0) FROM permits",
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        },
    )
    .unwrap()
}

fn classify(fixture: &Fixture, command: Value, captured_at_ms: u64) -> Value {
    let response = protocol::call(
        fixture,
        arc_ipc::locality::OPERATION_CLASSIFY,
        json!({
            "command": command,
            "context": {"captured_at_ms": captured_at_ms,
              "facts": [{"kind":"CI_HOST", "selector":"gh", "endpoints":["https://github.com"]}]}
        }),
    );
    assert_eq!(response.protocol_version, 1);
    assert_eq!(response.status, "ok", "{response:?}");
    response.result.unwrap()
}

fn classify_fresh(fixture: &Fixture, command: Value) -> Value {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let captured = arc_ipc::locality::now_ms().unwrap();
        let result = classify(fixture, command.clone(), captured);
        // A real daemon validates real time. Retry only an observed UNKNOWN
        // after the snapshot has actually expired, with a diagnostic watchdog.
        if result["classification"] != "UNKNOWN"
            || arc_ipc::locality::now_ms()
                .unwrap()
                .saturating_sub(captured)
                <= 5_000
        {
            return result;
        }
        assert!(
            Instant::now() < deadline,
            "fresh CI facts repeatedly expired before the daemon replied"
        );
    }
}

#[test]
fn derived_hooks_and_real_daemon_bypass_a_held_build_permit_with_one_offline_gh_execution() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut daemon = fixture.spawn_isolated("test");
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let id = JobId::try_from("a123456789abcdef0123456789abcdef".to_owned()).unwrap();
    let classification = serde_json::from_value(json!({
        "classification":"BUILD", "locality":"HOST_LOCAL", "reason":"CARGO_BUILD", "uncertainty":"NONE"
    })).unwrap();
    let hold = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification,
        lineage_context: None,
        project: None,
        event: None,
    };
    let JobResult::Granted {
        permit_generation, ..
    } = traffic(&fixture, &hold)
    else {
        panic!("local BUILD permit was not granted");
    };
    let baseline = scheduling(&fixture);
    assert_eq!(
        (baseline.0, baseline.1, baseline.2, baseline.3),
        (1, 0, 1, 1)
    );
    assert!(
        baseline.4 > 0 && baseline.5 > 0,
        "no actual resources reserved: {baseline:?}"
    );

    let stub = fixture.root.join("gh");
    fs::write(&stub, "#!/bin/sh\nprintf 'call\\n' >> \"$CI_MARKER\"\nprintf 'argc=%s\\n' \"$#\"\nfor arg do printf 'arg=<%s>\\n' \"$arg\"; done\nprintf 'cwd=<%s>\\n' \"$(pwd -P)\"\nprintf 'path=<%s>\\n' \"$PATH\"\nprintf 'marker=<%s>\\n' \"$CI_MARKER\"\nprintf 'env=<%s>\\n' \"$CI_VALUE\"\nIFS= read -r line\nprintf 'stdin=<%s>\\n' \"$line\"\nprintf 'offline stderr <%s>\\n' \"$CI_VALUE\" >&2\nexit 23\n").unwrap();
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o700)).unwrap();
    let marker = fixture.root.join("gh-calls");

    for (vendor, source) in [("claude", CLAUDE), ("cursor", CURSOR)] {
        let mut event: Value = serde_json::from_str(source).unwrap();
        event["cwd"] = json!(fixture.root);
        // These mutations are derived inputs, not new vendor recordings.
        for literal in ["cargo build", "bun run build"] {
            event["tool_input"]["command"] = json!(literal);
            let output = hook(&fixture, vendor, &event, &fixture.socket);
            assert!(
                output.status.success(),
                "{vendor} build control: {output:?}"
            );
            assert!(output.stderr.is_empty());
            let response: Value = serde_json::from_slice(&output.stdout).unwrap();
            let input = if vendor == "claude" {
                assert!(response["hookSpecificOutput"]
                    .get("permissionDecision")
                    .is_none());
                &response["hookSpecificOutput"]["updatedInput"]
            } else {
                assert!(response.get("permission").is_none());
                &response["updated_input"]
            };
            let rewritten = input["command"].as_str().unwrap();
            // Both recordings carry both ids, so a build names its event.
            assert!(
                rewritten.contains(" hook-exec --arc-event ")
                    && rewritten.ends_with(&format!(" -- {literal}")),
                "{rewritten}"
            );
            let mut restored = input.clone();
            restored["command"] = json!(literal);
            assert_eq!(restored, event["tool_input"]);
            assert_eq!(scheduling(&fixture), baseline);
        }
        event["tool_input"]["command"] = json!(COMMAND);
        let missing_socket = fixture.root.join("no-daemon.sock");
        for socket in [&fixture.socket, &missing_socket] {
            let output = hook(&fixture, vendor, &event, socket);
            assert!(output.status.success(), "{vendor} rewrite: {output:?}");
            let response: Value = serde_json::from_slice(&output.stdout).unwrap();
            let input = if vendor == "claude" {
                assert!(response["hookSpecificOutput"]
                    .get("permissionDecision")
                    .is_none());
                &response["hookSpecificOutput"]["updatedInput"]
            } else {
                assert!(response.get("permission").is_none());
                &response["updated_input"]
            };
            let rewritten = input["command"].as_str().unwrap();
            assert!(rewritten.starts_with("case "));
            assert!(rewritten.contains("--arc-check-gh-path"));
            assert!(rewritten.contains(&format!(" hook-exec -- {COMMAND}")));
            let mut restored = input.clone();
            restored["command"] = json!(COMMAND);
            assert_eq!(restored, event["tool_input"]);
            assert!(output.stderr.is_empty());
            assert!(!marker.exists(), "the hook must not execute the payload");
            assert_eq!(event["tool_input"]["command"], COMMAND);
            assert_eq!(scheduling(&fixture), baseline);
        }
    }

    // The hooks above did not classify the command. This independent request
    // uses fresh simulated endpoint facts against the actual daemon.
    let mut result = Value::Null;
    for command in [
        json!({"kind":"SHELL", "command":COMMAND, "shell_kind":"sh"}),
        json!({"kind":"ARGV", "argv":["gh", "workflow", "run", "build.yml"]}),
    ] {
        result = classify_fresh(&fixture, command);
        assert_eq!(
            result,
            json!({"classification":"REMOTE_CI", "locality":"REMOTE", "reason":"GH_WORKFLOW_RUN", "uncertainty":"NONE"})
        );
        assert_eq!(scheduling(&fixture), baseline);
    }
    let remote = JobRequest::Enqueue {
        job_id: JobId::try_from("b123456789abcdef0123456789abcdef".to_owned()).unwrap(),
        observer: observer.clone(),
        classification: serde_json::from_value(result).unwrap(),
        lineage_context: None,
        project: None,
        event: None,
    };
    assert!(matches!(
        traffic(&fixture, &remote),
        JobResult::Bypassed { .. }
    ));
    assert_eq!(scheduling(&fixture), baseline);

    // Independently exercise the unchanged command as a historical offline
    // fixture. The installed-hook execution is covered by closing_exec.
    let stdout = fixture.root.join("gh-stdout");
    let stderr = fixture.root.join("gh-stderr");
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", COMMAND])
        .env_clear()
        .env("HOME", &fixture.root)
        .env("PATH", &fixture.root)
        .env("CI_MARKER", &marker)
        .env("CI_VALUE", "value with spaces")
        .current_dir(&fixture.root)
        .stdin(Stdio::piped())
        .stdout(fs::File::create(&stdout).unwrap())
        .stderr(fs::File::create(&stderr).unwrap());
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    // A dedicated process group permits bounded cleanup even on unwinding.
    let mut child = child_watchdog::Owned::spawn(&mut command).unwrap();
    child
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"input bytes\n")
        .unwrap();
    let status = child.wait("offline gh shell").unwrap();
    let (stdout, stderr) = (fs::read(stdout).unwrap(), fs::read(stderr).unwrap());
    assert_eq!(status.code(), Some(23));
    assert_eq!(stdout, format!("argc=3\narg=<workflow>\narg=<run>\narg=<build.yml>\ncwd=<{}>\npath=<{}>\nmarker=<{}>\nenv=<value with spaces>\nstdin=<input bytes>\n", fs::canonicalize(&fixture.root).unwrap().display(), fixture.root.display(), marker.display()).as_bytes());
    assert_eq!(stderr, b"offline stderr <value with spaces>\n");
    assert_eq!(fs::read_to_string(&marker).unwrap(), "call\n");
    assert_eq!(scheduling(&fixture), baseline);

    assert_eq!(
        traffic(
            &fixture,
            &JobRequest::Cancel {
                job_id: id,
                observer,
                permit_generation: Some(permit_generation)
            }
        ),
        JobResult::Cancelled {}
    );
    assert_eq!(scheduling(&fixture).3, 0);
    daemon.stop(libc::SIGTERM);
    fixture.report();
}

// Exercise both public classification routes with an injected clock. The
// endpoint and host inventories below are simulated caller evidence.
fn classify_at(command: &str, context: &Value, now: u64) -> Vec<Value> {
    let argv: Vec<_> = command.split_whitespace().collect();
    [
        json!({"kind":"SHELL", "command":command, "shell_kind":"sh"}),
        json!({"kind":"ARGV", "argv":argv}),
    ]
    .into_iter()
    .map(|command| {
        let mut request =
            arc_ipc::Request::new("ci-regression", arc_ipc::locality::OPERATION_CLASSIFY);
        request.payload = json!({"command":command, "context":context});
        let response = arc_ipc::locality::classify_response_at(&request, now);
        assert_eq!(response.status, "ok");
        response.result.unwrap()
    })
    .collect()
}

#[test]
fn ci01_through_ci20_remote_denominator_is_zero_false_local_on_both_routes() {
    let gh = json!({"captured_at_ms":10_000, "facts":[
        {"kind":"CI_HOST", "selector":"gh", "endpoints":["https://github.com"]}
    ]});
    let git = json!({"captured_at_ms":10_000, "facts":[
        {"kind":"GIT_REMOTE", "selector":"origin", "endpoints":["https://github.com/org/repo.git"]}
    ]});
    // The effective URL still needs a fact: Git configuration can rewrite
    // an explicit URL. A different local default must not override it.
    let url = json!({"captured_at_ms":10_000, "facts":[
        {"kind":"GIT_REMOTE", "selector":"https://github.com/org/repo.git", "endpoints":["https://github.com/org/repo.git"]},
        {"kind":"GIT_REMOTE", "selector":"origin", "endpoints":["file:///tmp/repo"]}
    ]});
    let ssh = json!({"captured_at_ms":10_000, "facts":[
        {"kind":"GIT_REMOTE", "selector":"ssh://git@github.com/org/repo.git", "endpoints":["ssh://git@github.com/org/repo.git"]},
        {"kind":"SSH_DESTINATION", "selector":"github.com", "endpoints":["ssh://192.0.2.30:22"]},
        {"kind":"SSH_DESTINATION", "selector":"@local-addresses", "endpoints":["192.0.2.10"]}
    ]});
    let gl = json!({"captured_at_ms":10_000, "facts":[
        {"kind":"CI_HOST", "selector":"gitlab", "endpoints":["https://gitlab.com"]}
    ]});
    #[rustfmt::skip]
    let cases = [
        ("CI01", "gh workflow run build.yml", &gh),
        ("CI02", "gh workflow run build.yml --ref main", &gh),
        ("CI03", "gh workflow run build.yml --repo org/repo", &gh),
        ("CI04", "gh workflow run build.yml -f mode=release", &gh),
        ("CI05", "gh workflow run 42", &gh),
        ("CI06", "gh run watch 123", &gh),
        ("CI07", "gh run watch 123 --exit-status", &gh),
        ("CI08", "gh run view 123", &gh),
        ("CI09", "gh run view 123 --json status", &gh),
        ("CI10", "gh run list --workflow build.yml", &gh),
        ("CI11", "gh workflow view build.yml", &gh),
        ("CI12", "gh workflow list", &gh),
        ("CI13", "git push origin main", &git),
        ("CI14", "git push --set-upstream origin main", &git),
        ("CI15", "git push --force-with-lease origin main", &git),
        ("CI16", "git push origin --tags", &git),
        ("CI17", "git push https://github.com/org/repo.git main", &url),
        ("CI18", "git push ssh://git@github.com/org/repo.git main", &ssh),
        ("CI19", "gitlab pipeline run", &gl),
        ("CI20", "gitlab pipeline run --branch main", &gl),
    ];
    let mut false_local = 0;
    for (name, command, context) in cases {
        for result in classify_at(command, context, 10_000) {
            let parsed: arc_ipc::Classification = serde_json::from_value(result).unwrap();
            false_local += usize::from(parsed.locality == arc_ipc::Locality::HostLocal);
            assert_eq!(
                (parsed.classification, parsed.locality, parsed.uncertainty),
                (
                    arc_ipc::JobClass::RemoteCi,
                    arc_ipc::Locality::Remote,
                    arc_ipc::Uncertainty::None
                ),
                "{name} {command}: {parsed:?}"
            );
            assert!(parsed.is_consistent(), "{name}: {parsed:?}");
        }
    }
    assert_eq!(false_local, 0, "CI01-CI20, shell and argv");
    eprintln!("simulated SRS CI denominator CI01-CI20: 0 false-local / 20 cases, both routes");
}

#[test]
fn regression_bare_run_queries_require_proven_consistent_host_on_both_routes() {
    let good = json!({"captured_at_ms":10_000,
        "facts":[{"kind":"CI_HOST", "selector":"gh", "endpoints":["https://github.com"]}]});
    let missing = json!({"captured_at_ms":10_000, "environment":{"GH_HOST":"github.com"}});
    let conflict = json!({"captured_at_ms":10_000, "environment":{"GH_HOST":"localhost"},
        "facts":[{"kind":"CI_HOST", "selector":"gh", "endpoints":["https://github.com"]}]});
    let conflicting_fact = json!({"captured_at_ms":10_000,
        "facts":[{"kind":"CI_HOST", "selector":"gh", "endpoints":["https://localhost"]}]});
    let multiple_hosts = json!({"captured_at_ms":10_000,
        "facts":[{"kind":"CI_HOST", "selector":"gh", "endpoints":["https://github.com", "https://localhost"]}]});
    for (command, reason) in [
        ("gh run watch", "GH_RUN_WATCH"),
        ("gh run watch --exit-status", "GH_RUN_WATCH"),
        ("gh run view", "GH_RUN_VIEW"),
        ("gh run view --json status", "GH_RUN_VIEW"),
    ] {
        for result in classify_at(command, &good, 10_000) {
            assert_eq!(
                result,
                json!({"classification":"REMOTE_CI", "locality":"REMOTE", "reason":reason, "uncertainty":"NONE"}),
                "{command}"
            );
        }
        for context in [&missing, &conflict, &conflicting_fact, &multiple_hosts] {
            for result in classify_at(command, context, 10_000) {
                assert_eq!(
                    result,
                    json!({"classification":"UNKNOWN", "locality":"UNKNOWN", "reason":"ENDPOINT_AMBIGUOUS", "uncertainty":"OBSERVE"}),
                    "{command}"
                );
            }
        }
    }
    for command in ["gh run watch 1 2", "gh run view 1 2"] {
        for result in classify_at(command, &good, 10_000) {
            assert_eq!(result["classification"], "UNKNOWN", "{command}");
            assert_eq!(result["uncertainty"], "OBSERVE", "{command}");
        }
    }
}

#[test]
fn regression_git_ssh_requires_fresh_complete_nonmatching_local_inventory_on_both_routes() {
    let command = "git push ssh://git@github.com/org/repo.git main";
    let base = json!({"captured_at_ms":10_000, "facts":[
        {"kind":"GIT_REMOTE", "selector":"ssh://git@github.com/org/repo.git", "endpoints":["ssh://git@github.com/org/repo.git"]},
        {"kind":"SSH_DESTINATION", "selector":"github.com", "endpoints":["ssh://192.0.2.30:22"]}
    ]});
    for (name, addresses, now, remote) in [
        ("missing", None, 10_000, false),
        (
            "same address",
            Some(vec!["127.0.0.1", "192.0.2.30"]),
            10_000,
            false,
        ),
        (
            "IPv4 mapped",
            Some(vec!["::ffff:192.0.2.30"]),
            10_000,
            false,
        ),
        (
            "invalid",
            Some(vec!["192.0.2.10", "not-an-address"]),
            10_000,
            false,
        ),
        ("unspecified", Some(vec!["0.0.0.0"]), 10_000, false),
        ("empty", Some(vec![]), 10_000, false),
        ("stale", Some(vec!["192.0.2.10"]), 15_001, false),
        ("future", Some(vec!["192.0.2.10"]), 9_999, false),
        (
            "distinct",
            Some(vec!["127.0.0.1", "192.0.2.10", "::1"]),
            10_000,
            true,
        ),
    ] {
        let mut context = base.clone();
        if let Some(addresses) = addresses {
            context["facts"].as_array_mut().unwrap().push(json!({
                "kind":"SSH_DESTINATION", "selector":"@local-addresses", "endpoints":addresses
            }));
        }
        for result in classify_at(command, &context, now) {
            let expected = if remote {
                json!({"classification":"REMOTE_CI", "locality":"REMOTE", "reason":"REMOTE_CI_DISPATCH", "uncertainty":"NONE"})
            } else {
                json!({"classification":"UNKNOWN", "locality":"UNKNOWN", "reason":"ENDPOINT_AMBIGUOUS", "uncertainty":"OBSERVE"})
            };
            assert_eq!(result, expected, "{name}");
        }
    }
}
