//! Actual daemon/client contract cases; no analyzed command is ever executed.
#[path = "classification/boundaries.rs"]
mod boundaries;
#[path = "support/child_watchdog.rs"]
mod child_watchdog;
#[path = "support/instrumentation.rs"]
#[allow(clippy::duplicate_mod)] // Shared by independently reusable fixture modules.
mod instrumentation;
#[path = "classification/privacy.rs"]
mod privacy;
#[path = "support/runtime.rs"]
#[allow(dead_code)] // Shared owned-child fixture also exposes unrelated workload helpers.
mod runtime;
#[path = "classification/saturation.rs"]
mod saturation;
mod support;

#[path = "../../arc-detector/tests/support/contract.rs"]
mod contract_fixtures;

use arc_ipc::{Request, Response, OPERATION_JOB_CLASSIFY};
use runtime::{Fixture, OwnedChild};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::{fs::symlink, net::UnixStream},
    path::Path,
    process::{Command, Output},
};

fn fixture_interposer(fixture: &Fixture) -> std::path::PathBuf {
    let library = fixture.root.join("classification-interfaces");
    if library.exists() {
        return library;
    }
    let source = fixture.root.join("classification-interfaces.c");
    // The production SSH capture checks the account home even when HOME is
    // isolated. Supply a synthetic account as well as synthetic interfaces.
    let account = r#"
#include <errno.h>
#include <pwd.h>
static int fixture_getpwuid_r(uid_t uid, struct passwd *pwd, char *buf,
                              size_t len, struct passwd **result) {
    (void)uid;
    const char home[] = "/arc-fixture-account-home";
    const char *marker = getenv("ARC_TEST_ACCOUNT_MARKER");
    if (marker != NULL) {
        int fd = open(marker, O_WRONLY | O_CREAT | O_APPEND, 0600);
        if (fd >= 0) { (void)write(fd, "x", 1); (void)close(fd); }
    }
    if (len < sizeof(home)) { *result = NULL; return ERANGE; }
    memset(pwd, 0, sizeof(*pwd));
    memcpy(buf, home, sizeof(home));
    pwd->pw_dir = buf;
    *result = pwd;
    return 0;
}
#ifdef __APPLE__
__attribute__((used, section("__DATA,__interpose")))
static const struct { const void *replacement; const void *original; }
fixture_account_hook = {(const void *)fixture_getpwuid_r, (const void *)getpwuid_r};
#else
int getpwuid_r(uid_t uid, struct passwd *pwd, char *buf, size_t len, struct passwd **result) {
    return fixture_getpwuid_r(uid, pwd, buf, len, result);
}
#endif
"#;
    fs::write(
        &source,
        format!(
            "{}\n{account}",
            include_str!("managed_exec/capture_integration/interfaces.c")
        ),
    )
    .unwrap();
    let mut compiler = Command::new("/usr/bin/cc");
    compiler
        .env_clear()
        .env("HOME", &fixture.root)
        .env("PATH", "/usr/bin:/bin");
    instrumentation::forward(&mut compiler);
    if cfg!(target_os = "macos") {
        compiler.arg("-dynamiclib");
    } else {
        compiler.args(["-shared", "-fPIC"]);
    }
    compiler.arg("-o").arg(&library).arg(source);
    let output = child_watchdog::output(&mut compiler, "classification fixture compiler").unwrap();
    assert!(
        output.status.success(),
        "fixture compiler failed: {output:?}"
    );
    library
}

fn contract() -> Value {
    serde_json::from_str(include_str!(
        "../../../.arc-verify/command-classification/contract.json"
    ))
    .unwrap()
}
fn request(payload: Value) -> Request {
    let mut request = Request::new("classification-fixture", OPERATION_JOB_CLASSIFY);
    request.payload = payload;
    request
}
fn exchange(stream: &mut UnixStream, payload: Value) -> Response {
    arc_ipc::exchange(stream, &request(payload)).unwrap()
}
fn cli(fixture: &Fixture, args: &[&str]) -> Output {
    let path = Path::new(env!("CARGO_BIN_EXE_arc-daemon")).with_file_name("arcctl");
    let library = fixture_interposer(fixture);
    let mut command = Command::new(path);
    command
        .env_clear()
        .env("HOME", &fixture.root)
        .env("PATH", &fixture.root)
        .env("XDG_CONFIG_HOME", fixture.root.join("config"))
        .env("GH_CONFIG_DIR", fixture.root.join("gh"))
        .env("GIT_CONFIG_GLOBAL", fixture.root.join("missing-git-config"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("DOCKER_CONFIG", fixture.root.join("missing-docker-config"))
        .env("BUILDX_CONFIG", fixture.root.join("missing-buildx-config"))
        .env("KUBECONFIG", fixture.root.join("missing-kubeconfig"))
        .env("GH_HOST", "github.com")
        .env("GH_REPO", "github.com/fixture-owner/fixture-repo")
        .env(
            "ARC_TEST_INTERFACE_MARKER",
            fixture.root.join("interface-calls"),
        )
        .env(
            "ARC_TEST_ACCOUNT_MARKER",
            fixture.root.join("account-calls"),
        )
        .env(
            if cfg!(target_os = "macos") {
                "DYLD_INSERT_LIBRARIES"
            } else {
                "LD_PRELOAD"
            },
            library,
        )
        .current_dir(&fixture.root)
        .env(arc_core::ARC_SOCKET_PATH, &fixture.socket)
        .args(args);
    instrumentation::forward(&mut command);
    child_watchdog::output(&mut command, "classification CLI").unwrap()
}
fn human(result: &Value) -> String {
    format!(
        "Classification: {}\nLocality: {}\nReason: {}\nUncertainty: {}\nCommand not executed.\n",
        result["classification"].as_str().unwrap(),
        result["locality"].as_str().unwrap(),
        result["reason"].as_str().unwrap(),
        result["uncertainty"].as_str().unwrap()
    )
}

#[test]
fn actual_daemon_and_cli_delivered_contract_matrix() {
    daemon_and_cli_matrix(contract_fixtures::fixtures());
}

#[test]
fn ssh_post_destination_daemon_human_and_json_cli_matrix() {
    daemon_and_cli_matrix(contract_fixtures::ssh_fixtures());
}

fn daemon_and_cli_matrix(cases: Vec<Value>) {
    let _serial = support::serial();
    let fixture = Fixture::new();
    // Explicitly supply only the system shells attested by capture. An empty
    // PATH cannot prove which executable an actual shell entry point selects.
    for shell in ["bash", "zsh"] {
        symlink(format!("/bin/{shell}"), fixture.root.join(shell)).unwrap();
    }
    let mut child = fixture.spawn_isolated("");
    let mut stream = fixture.connect();
    for case in cases {
        let response = exchange(&mut stream, case["payload"].clone());
        assert_eq!(response.status, "ok", "{}", case["id"]);
        assert_eq!(
            response.result.as_ref(),
            Some(&case["result"]),
            "{}",
            case["id"]
        );
        let command = case["payload"]["command"].as_str().unwrap();
        let shell = case["payload"]["shell_kind"].as_str().unwrap_or("sh");
        let reason = case["result"]["reason"].as_str().unwrap();
        let contextual = if case["id"] == "case-17" {
            // The protocol-1 assertion above remains frozen; CLI inspection
            // must not trust that result across a bash login startup.
            json!({"classification":"UNKNOWN","locality":"UNKNOWN",
                "reason":"UNRESOLVED_SCRIPT","uncertainty":"OBSERVE"})
        } else if matches!(
            reason,
            "SSH_REMOTE_COMMAND" | "SSH_AMBIGUOUS" | "SSH_FORWARDING" | "DOCKER_ENDPOINT_UNKNOWN"
        ) || case["id"] == "terminated-compound"
        {
            json!({"classification":"UNKNOWN","locality":"UNKNOWN",
                "reason":"ENDPOINT_AMBIGUOUS","uncertainty":"OBSERVE"})
        } else {
            case["result"].clone()
        };
        for json in [false, true] {
            let mut args = vec!["classify", "--shell", shell];
            if json {
                args.push("--json");
            }
            args.extend(["--", command]);
            let output = cli(&fixture, &args);
            assert!(output.status.success(), "{}", case["id"]);
            assert!(output.stderr.is_empty(), "{}", case["id"]);
            if json {
                assert_eq!(output.stdout.last(), Some(&b'\n'), "{}", case["id"]);
                assert_eq!(output.stdout.iter().filter(|b| **b == b'\n').count(), 1);
                assert_eq!(
                    serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                    contextual,
                    "{}",
                    case["id"]
                );
            } else {
                assert_eq!(
                    output.stdout,
                    human(&contextual).as_bytes(),
                    "{}",
                    case["id"]
                );
            }
            assert!(!output.stdout.contains(&27));
        }
    }
    assert!(
        !fs::read(fixture.root.join("account-calls"))
            .unwrap()
            .is_empty(),
        "SSH inspection must use the synthetic account fixture"
    );
    let status = cli(&fixture, &["status"]);
    assert!(status.status.success() && status.stderr.is_empty());
    let status = String::from_utf8(status.stdout).unwrap();
    assert!(status.contains("Sweeper off."));
    assert!(status.contains(fixture.socket.to_str().unwrap()));
    let doctor = cli(&fixture, &["doctor"]);
    assert!(doctor.status.success() && doctor.stderr.is_empty());
    let doctor = String::from_utf8(doctor.stdout).unwrap();
    assert_eq!(doctor, "ok. IPC owner-only. Tray not connected. Vendors not detected. Command classification not checked. Sweeper off. Schema compatible. Version 7. Migration history verified.\n");
    assert!(!doctor.contains('\u{1b}'));
    drop(stream);
    child.stop(libc::SIGTERM);
    let report = fixture.report();
    assert_eq!(report["metrics"]["db_outstanding"]["high_water"], 1); // only the explicit doctor
}

#[test]
fn invalid_payloads_version_priority_and_same_connection_recovery() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = fixture.spawn("");
    let mut stream = fixture.connect();
    for case in contract()["malformedPayloadFixtures"].as_array().unwrap() {
        let response = exchange(&mut stream, case["payload"].clone());
        assert_eq!(
            response.error.unwrap().code,
            arc_ipc::ERROR_INVALID_REQUEST,
            "{}",
            case["id"]
        );
        let mut wrong = request(case["payload"].clone());
        wrong.protocol_version = 99;
        assert_eq!(
            arc_ipc::exchange(&mut stream, &wrong)
                .unwrap()
                .error
                .unwrap()
                .code,
            arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE
        );
        assert_eq!(
            exchange(&mut stream, json!({"command":"cargo test"}))
                .result
                .unwrap()["classification"],
            "TEST"
        );
    }
    arc_ipc::write_frame(&mut stream, b"{").unwrap();
    assert_eq!(
        arc_ipc::read_response(&mut stream)
            .unwrap()
            .error
            .unwrap()
            .code,
        arc_ipc::ERROR_INVALID_REQUEST
    );
    assert_eq!(
        exchange(&mut stream, json!({"command":"gh run watch"}))
            .result
            .unwrap()["classification"],
        "REMOTE_CI"
    );
    let unknown = Request::new("unsupported", "job.admit");
    assert_eq!(
        arc_ipc::exchange(&mut stream, &unknown)
            .unwrap()
            .error
            .unwrap()
            .code,
        arc_ipc::ERROR_UNSUPPORTED_CAPABILITY
    );
    drop(stream);
    child.stop(libc::SIGTERM);
    let report = fixture.report();
    assert_eq!(report["metrics"]["db_outstanding"]["high_water"], 0);
}

#[test]
fn ssh_option_values_preserve_typed_daemon_and_cli_observations() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = fixture.spawn("");
    let mut stream = fixture.connect();
    for (index, (command, reason)) in [
        ("ssh -lDora build01 make", "SSH_AMBIGUOUS"),
        ("ssh -oLogLevel=INFO build01 make", "SSH_AMBIGUOUS"),
        ("ssh -l N build01 make", "SSH_AMBIGUOUS"),
        ("ssh -o '--N' build01 make", "SSH_AMBIGUOUS"),
        ("ssh -zN build01 make", "SSH_AMBIGUOUS"),
        ("ssh -nNT build01", "SSH_FORWARDING"),
        ("ssh -vNL8080:host:80 build01 make", "SSH_FORWARDING"),
        ("ssh -D8080 build01 make", "SSH_FORWARDING"),
        ("ssh -lDora -N build01", "SSH_FORWARDING"),
    ]
    .iter()
    .enumerate()
    {
        let expected = json!({"classification":"UNKNOWN","locality":"UNKNOWN",
            "reason":reason,"uncertainty":"OBSERVE"});
        let response = exchange(&mut stream, json!({"command":command}));
        assert_eq!(response.status, "ok", "ssh-option-{index}");
        assert_eq!(
            response.result,
            Some(expected.clone()),
            "ssh-option-{index}"
        );
        let contextual = json!({"classification":"UNKNOWN","locality":"UNKNOWN",
            "reason":"ENDPOINT_AMBIGUOUS","uncertainty":"OBSERVE"});
        for json in [false, true] {
            let mut args = vec!["classify"];
            if json {
                args.push("--json");
            }
            args.extend(["--", command]);
            let output = cli(&fixture, &args);
            assert!(
                output.status.success() && output.stderr.is_empty(),
                "ssh-option-{index}"
            );
            if json {
                assert_eq!(
                    serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                    contextual,
                    "ssh-option-{index}"
                );
            } else {
                assert_eq!(
                    output.stdout,
                    human(&contextual).as_bytes(),
                    "ssh-option-{index}"
                );
            }
        }
    }
    drop(stream);
    child.stop(libc::SIGTERM);
    assert_eq!(
        fixture.report()["metrics"]["db_outstanding"]["high_water"],
        0
    );
}
