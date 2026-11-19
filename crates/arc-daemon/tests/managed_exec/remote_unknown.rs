//! Unknown execution against a real held Build reservation. The shell programs
//! and unavailable socket are local fixtures; no remote service is contacted.
use super::{protocol, runtime::Fixture, support};
use child::{execute, install_script, Scenario, CALLER_LLVM_MARKER, LLVM_MARKER_KEY};

#[path = "remote_unknown/child.rs"]
mod child;
use arc_ipc::{
    jobs::{JobId, Outcome, ProcessIdentity},
    traffic::{JobRequest, JobResult, OPERATION_TRAFFIC_JOB},
};
use rusqlite::OptionalExtension;
use serde_json::json;
use std::{
    ffi::OsString,
    fs,
    io::{self, Read},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::PermissionsExt,
        io::RawFd,
    },
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

fn traffic(f: &Fixture, request: &JobRequest) -> JobResult {
    let response = protocol::call(f, OPERATION_TRAFFIC_JOB, json!(request));
    assert_eq!(response.status, "ok", "{response:?}");
    serde_json::from_value(response.result.unwrap()).unwrap()
}

// Snapshot all entries and permits, including inactive rows, so a transient
// queue entry or a released extra reservation cannot masquerade as a bypass.
type ReservationSnapshot = (Vec<(String, String)>, Vec<(String, String, i64, i64)>);

fn reservations(f: &Fixture) -> ReservationSnapshot {
    let db = rusqlite::Connection::open(&f.database).unwrap();
    let entries = db
        .prepare("SELECT job_id,state FROM traffic_entries ORDER BY job_id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let permits = db
        .prepare("SELECT job_id,state,cpu_millis_est,memory_bytes_est FROM permits ORDER BY job_id")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    (entries, permits)
}

pub(super) fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(super) fn kill_target_group(
    session: u32,
    target: &Option<ProcessIdentity>,
    hint: Option<&Path>,
) -> bool {
    use arc_core::process_identity::{probe_live, IdentityStatus};
    let target = target.clone().or_else(|| {
        let pid = fs::read_to_string(hint?).ok()?.trim().parse().ok()?;
        ProcessIdentity::capture(pid).ok()
    });
    let Some(target) = target else { return false };
    let pid = target.pid as i32;
    if pid == session as i32
        || probe_live(&target) != IdentityStatus::VerifiedMatch
        || unsafe { libc::getsid(pid) } != session as i32
        || unsafe { libc::getpgid(pid) } != pid
    {
        return false;
    }
    if unsafe { libc::kill(-pid, libc::SIGKILL) } != 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            eprintln!("owned target group {pid} cleanup failed: {error}");
        }
        return false;
    }
    true
}

pub(super) fn drain_stream(reader: &mut impl Read, bytes: &mut Vec<u8>) -> io::Result<bool> {
    let mut buffer = [0; 4096];
    for _ in 0..16 {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(true),
            Ok(n) => bytes.extend_from_slice(&buffer[..n]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

fn job_for_observer(f: &Fixture, observer: &ProcessIdentity) -> Option<JobId> {
    rusqlite::Connection::open(&f.database)
        .unwrap()
        .query_row(
            "SELECT job_id FROM job_lifecycle WHERE observer_pid=?1 AND observer_start_token=?2 AND observer_boot_id=?3",
            rusqlite::params![observer.pid, observer.start_token, observer.boot_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .unwrap()
        .map(|id| JobId::try_from(id).unwrap())
}

pub(super) fn assert_descendants_gone(identities: Vec<ProcessIdentity>, mode: &str) {
    use arc_core::process_identity::{probe, IdentityStatus};
    let deadline = Instant::now() + Duration::from_secs(60);
    for identity in identities {
        loop {
            match probe(&identity) {
                IdentityStatus::Missing | IdentityStatus::Mismatch => break,
                _ => assert!(
                    Instant::now() < deadline,
                    "{mode}: descendant remains: {identity:?}"
                ),
            }
            thread::sleep(Duration::from_millis(2));
        }
    }
}

#[test]
fn custom_shell_bypasses_held_build_and_preserves_native_execution_on_fail_open() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let holder = protocol::id(401);
    let second = protocol::id(402);
    let build = serde_json::from_value(json!({"classification":"BUILD","locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"})).unwrap();
    let enqueue = |id: JobId| JobRequest::Enqueue {
        job_id: id,
        observer: observer.clone(),
        classification: build,
        lineage_context: None,
    };
    let JobResult::Granted {
        permit_generation, ..
    } = traffic(&f, &enqueue(holder.clone()))
    else {
        panic!("the real daemon did not grant the Build reservation");
    };
    // A second local Build must actually wait until the holder is released.
    assert!(matches!(
        traffic(&f, &enqueue(second.clone())),
        JobResult::Queued { .. }
    ));
    assert_eq!(
        traffic(
            &f,
            &JobRequest::Cancel {
                job_id: second,
                observer: observer.clone(),
                permit_generation: None,
            }
        ),
        JobResult::Cancelled {}
    );
    let before = reservations(&f);
    assert_eq!(before.1.len(), 1);
    assert_eq!(before.1[0].0, holder.as_str());
    assert_eq!(before.1[0].1, "ACTIVE");

    let program = f.root.join("custom shell program");
    let endpoint_program = f.root.join("kubectl");
    install_script(&program);
    install_script(&endpoint_program);
    for tool in ["cargo", "gh"] {
        let path = f.root.join(tool);
        fs::write(
            &path,
            format!("#!/bin/sh\nprintf '{tool} %s\\n' \"$*\" >> \"$STAGES\"\n"),
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let absent_socket: PathBuf = f.root.join("unavailable.sock");
    let marker = f.root.join("invocations");
    let ordinary = vec![
        OsString::from(""),
        "two words".into(),
        "한글".into(),
        "; | $(touch forbidden)".into(),
    ];
    // The detector deliberately refuses an over-budget input; native exec must
    // still pass its exact bytes through instead of waiting for a permit.
    let oversized = vec![OsString::from("x".repeat(16_385))];
    let raw_bytes = vec![OsString::from_vec(vec![0xff, 0xfe])];
    // This is a shell fixture called kubectl, not a cluster operation. An
    // over-limit KUBECONFIG makes bounded endpoint capture unavailable.
    let endpoint_args = vec!["exec".into(), "pod".into(), "--".into(), "make".into()];
    let unavailable_context = "x".repeat(1025);
    for scenario in [
        Scenario {
            name: "unknown",
            executable: &program,
            args: &ordinary,
            socket: &f.socket,
            kubecfg: None,
            expected_reason: Some("UNRESOLVED_COMMAND"),
            mixed: false,
            caller_llvm_marker: false,
        },
        Scenario {
            name: "caller-supplied-llvm-marker",
            executable: &program,
            args: &ordinary,
            socket: &f.socket,
            kubecfg: None,
            expected_reason: Some("UNRESOLVED_COMMAND"),
            mixed: false,
            caller_llvm_marker: true,
        },
        Scenario {
            name: "classification-input-limit",
            executable: &program,
            args: &oversized,
            socket: &f.socket,
            kubecfg: None,
            expected_reason: Some("INPUT_LIMIT"),
            mixed: false,
            caller_llvm_marker: false,
        },
        Scenario {
            name: "non-utf8-argument",
            executable: &program,
            args: &raw_bytes,
            socket: &f.socket,
            kubecfg: None,
            // The detector can refuse the byte sequence, or caller-context
            // capture can fail before it gets a chance to parse it.
            expected_reason: Some("NON_UTF8"),
            mixed: false,
            caller_llvm_marker: false,
        },
        Scenario {
            name: "classification-context-unavailable",
            executable: &endpoint_program,
            args: &endpoint_args,
            socket: &f.socket,
            kubecfg: Some(&unavailable_context),
            expected_reason: Some("UNRESOLVED_COMMAND"),
            mixed: false,
            caller_llvm_marker: false,
        },
        Scenario {
            name: "mixed-unsplit",
            executable: &program,
            args: &ordinary,
            socket: &f.socket,
            kubecfg: None,
            expected_reason: Some("UNRESOLVED_COMMAND"),
            mixed: true,
            caller_llvm_marker: false,
        },
        Scenario {
            name: "daemon-unavailable",
            executable: &program,
            args: &ordinary,
            socket: &absent_socket,
            kubecfg: None,
            expected_reason: None,
            mixed: false,
            caller_llvm_marker: false,
        },
    ] {
        let name = scenario.name;
        let (direct, _, _) = execute(&f, &scenario, &marker, false, None);
        assert_eq!(fs::read(&marker).unwrap(), b"invoked\n", "{name}");
        fs::remove_file(&marker).unwrap();
        if scenario.mixed {
            let stages = f.root.join("mixed-stages");
            assert_eq!(
                fs::read(&stages).unwrap(),
                b"cargo test\ngh workflow run build.yml\n"
            );
            fs::remove_file(stages).unwrap();
        }
        let (managed, observer, target) = execute(&f, &scenario, &marker, true, Some(&before));
        assert_eq!(fs::read(&marker).unwrap(), b"invoked\n", "{name}");
        fs::remove_file(&marker).unwrap();
        if scenario.mixed {
            let stages = f.root.join("mixed-stages");
            assert_eq!(
                fs::read(&stages).unwrap(),
                b"cargo test\ngh workflow run build.yml\n"
            );
            fs::remove_file(stages).unwrap();
        }
        assert_eq!(direct.status.code(), Some(37), "{name}: {direct:?}");
        assert_eq!(managed.status, direct.status, "{name}");
        let mut arguments = b"program=<".to_vec();
        arguments.extend_from_slice(scenario.executable.as_os_str().as_bytes());
        arguments.extend_from_slice(b">\n");
        arguments.extend_from_slice(format!("argc={}\n", scenario.args.len()).as_bytes());
        for arg in scenario.args {
            arguments.push(b'<');
            arguments.extend_from_slice(arg.as_os_str().as_bytes());
            arguments.extend_from_slice(b">\n");
        }
        assert!(direct.stdout.starts_with(&arguments), "{name}: argv bytes");
        let cwd = format!("cwd={}\n", f.root.canonicalize().unwrap().display());
        let mut environment_prefix = arguments.clone();
        environment_prefix.extend_from_slice(cwd.as_bytes());
        environment_prefix.extend_from_slice(b"environment:\n");
        assert!(
            direct.stdout.starts_with(&environment_prefix),
            "{name}: cwd and environment boundary"
        );
        assert_eq!(
            managed.stdout, direct.stdout,
            "{name}: stdout and environment"
        );
        if scenario.caller_llvm_marker {
            let supplied = format!("{LLVM_MARKER_KEY}={CALLER_LLVM_MARKER}\n");
            for output in [&direct.stdout, &managed.stdout] {
                assert!(
                    output
                        .windows(supplied.len())
                        .any(|bytes| bytes == supplied.as_bytes()),
                    "{name}: caller-supplied LLVM marker"
                );
            }
        }
        let variable = "FIXTURE_ENV=spaces 한글 ; $(literal)\n".as_bytes();
        assert!(
            direct
                .stdout
                .windows(variable.len())
                .any(|bytes| bytes == variable),
            "{name}: environment"
        );
        assert!(direct
            .stdout
            .windows(b"input\0bytes\n".len())
            .any(|bytes| bytes == b"input\0bytes\n"));
        assert_eq!(direct.stderr, b"child-stderr\n");
        if let Some(expected_reason) = scenario.expected_reason {
            assert_eq!(managed.stderr, direct.stderr, "{name}");
            let id = job_for_observer(&f, &observer).expect("daemon must record the unknown child");
            let job = protocol::status(&f, &id);
            let reason = if expected_reason == "NON_UTF8" {
                let reason = serde_json::to_value(job.classification).unwrap()["reason"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                assert!(matches!(
                    reason.as_str(),
                    "UNSUPPORTED_SYNTAX" | "UNRESOLVED_COMMAND"
                ));
                reason
            } else {
                expected_reason.to_owned()
            };
            assert_eq!(
                serde_json::to_value(job.classification).unwrap(),
                json!({"classification":"UNKNOWN","locality":"UNKNOWN","reason":reason,"uncertainty":"OBSERVE"}),
                "{name}"
            );
            assert_eq!(job.outcome, Some(Outcome::Exited { code: 37 }));
            assert_eq!(job.child, Some(target));
        } else {
            assert!(String::from_utf8_lossy(&managed.stderr).contains("TRACKING_UNAVAILABLE"));
            assert_eq!(super::fail_open::arc_stderr(&managed.stderr), direct.stderr);
            assert!(job_for_observer(&f, &observer).is_none());
        }
        // Completion before the permit release proves no dependency on it.
        assert_eq!(reservations(&f), before, "{name} modified Traffic state");
        assert!(!f.root.join("forbidden").exists());
    }
    assert_eq!(
        traffic(
            &f,
            &JobRequest::Cancel {
                job_id: holder,
                observer,
                permit_generation: Some(permit_generation),
            }
        ),
        JobResult::Cancelled {}
    );
    assert_eq!(reservations(&f).1[0].1, "RELEASED");
    daemon.stop(libc::SIGTERM);
    f.report();
}
