//! Synthetic endpoint facts exercise one registered, attached and completed child.
use super::{protocol, runtime::Fixture, support};
use arc_ipc::{
    jobs::{
        CompleteRequest, JobId, Outcome, ProcessIdentity, StartRequest, StatusRequest,
        OPERATION_JOB_COMPLETE, OPERATION_JOB_START, OPERATION_JOB_START_LOCALITY,
        OPERATION_JOB_STATUS, OPERATION_JOB_STATUS_LOCALITY,
    },
    locality::OPERATION_CLASSIFY,
    Request,
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::fs::{symlink, PermissionsExt},
    process::{Command as ProcessCommand, Stdio},
    sync::mpsc,
    time::Duration,
};

#[path = "../support/child_watchdog.rs"]
mod child_watchdog;
#[path = "../support/instrumentation.rs"]
#[allow(clippy::duplicate_mod)] // Shared by independently reusable fixture modules.
mod instrumentation;

#[test]
fn synthetic_endpoint_classification_is_bound_to_the_one_executed_child() {
    let _serial = support::serial(); // The daemon itself enforces one instance per user.
    let fixture = Fixture::new();
    let mut daemon = fixture.spawn_isolated("test");
    let argv = ["docker", "--context", "prod", "build", "."];
    let context = json!({
        "captured_at_ms":10000, "facts":[
            {"kind":"DOCKER_CONTEXT", "selector":"prod", "endpoints":["tcp://192.0.2.40:2376"]},
            {"kind":"DOCKER_BUILDER", "selector":"prod", "endpoints":["tcp://192.0.2.40:2376"]},
            {"kind":"SSH_DESTINATION", "selector":"@local-addresses", "endpoints":["192.0.2.5"]}
        ]
    });
    let mut classify = Request::new("synthetic", OPERATION_CLASSIFY);
    classify.payload = json!({"command":{"kind":"ARGV", "argv":argv}, "context":context});
    let classification: arc_ipc::Classification = serde_json::from_value(
        arc_ipc::locality::classify_response_at(&classify, 10000)
            .result
            .unwrap(),
    )
    .unwrap();
    assert_eq!(classification.reason, arc_ipc::Reason::RemoteEndpointExec);
    assert_eq!(classification.locality, arc_ipc::Locality::Remote);

    let id = JobId::try_from("b".repeat(32)).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let register = StartRequest::Register {
        job_id: id.clone(),
        observer: observer.clone(),
        classification,
    };
    let old = protocol::call(&fixture, OPERATION_JOB_START, json!(register));
    assert_eq!(old.error.unwrap().code, arc_ipc::ERROR_INVALID_REQUEST);
    let started = protocol::call(&fixture, OPERATION_JOB_START_LOCALITY, json!(register));
    assert_eq!(started.status, "ok");
    assert_eq!(
        started.result.unwrap()["job"]["classification"],
        json!(classification)
    );
    let claim = StartRequest::Claim {
        job_id: id.clone(),
        observer: observer.clone(),
    };
    let claimed = protocol::call(&fixture, OPERATION_JOB_START, json!(claim));
    assert_eq!(
        claimed.result.unwrap()["job"]["classification"],
        json!(classification.legacy())
    );

    let executable = fixture.root.join("docker");
    let marker = fixture.root.join("calls");
    fs::write(&executable, "#!/bin/sh\nread line\nprintf 'once\\n' >> \"$MARKER\"\nprintf '%s\\n' \"$line\"\nexit 23\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let stdout = fixture.root.join("child-stdout");
    let mut command = ProcessCommand::new("docker");
    command
        .args(&argv[1..])
        .env_clear()
        .env("PATH", &fixture.root)
        .env("HOME", &fixture.root)
        .env("MARKER", &marker)
        .current_dir(&fixture.root)
        .stdin(Stdio::piped())
        .stdout(fs::File::create(&stdout).unwrap())
        .stderr(Stdio::null());
    instrumentation::forward(&mut command);
    let mut child = child_watchdog::Owned::spawn(&mut command).unwrap();
    let identity = ProcessIdentity::capture(child.child.id()).unwrap();
    let attach = StartRequest::Attach {
        job_id: id.clone(),
        observer: observer.clone(),
        child: identity.clone(),
    };
    assert_eq!(
        protocol::call(&fixture, OPERATION_JOB_START, json!(attach)).status,
        "ok"
    );
    let live = protocol::call(
        &fixture,
        OPERATION_JOB_STATUS_LOCALITY,
        json!(StatusRequest::ById { job_id: id.clone() }),
    )
    .result
    .unwrap();
    assert_eq!(live["job"]["classification"], json!(classification));
    assert_eq!(live["job"]["child"]["pid"], identity.pid);
    assert_eq!(live["job"]["phase"], "RUNNING");
    child
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"ready\n")
        .unwrap();
    assert_eq!(
        child.wait("synthetic docker stub").unwrap().code(),
        Some(23)
    );
    let completed = CompleteRequest {
        job_id: id.clone(),
        observer,
        outcome: Outcome::Exited { code: 23 },
    };
    assert_eq!(
        protocol::call(&fixture, OPERATION_JOB_COMPLETE, json!(completed)).status,
        "ok"
    );
    let final_job = protocol::call(
        &fixture,
        OPERATION_JOB_STATUS_LOCALITY,
        json!(StatusRequest::ById { job_id: id.clone() }),
    )
    .result
    .unwrap();
    assert_eq!(final_job["job"]["classification"], json!(classification));
    assert_eq!(final_job["job"]["outcome"]["code"], 23);
    let legacy = protocol::call(
        &fixture,
        OPERATION_JOB_STATUS,
        json!(StatusRequest::ById { job_id: id }),
    )
    .result
    .unwrap();
    assert_eq!(
        legacy["job"]["classification"],
        json!(classification.legacy())
    );
    assert_eq!(fs::read_to_string(marker).unwrap(), "once\n");
    assert_eq!(fs::read_to_string(stdout).unwrap(), "ready\n");
    let db = rusqlite::Connection::open(&fixture.database).unwrap();
    let counts: (i64, i64) = db
        .query_row(
            "SELECT (SELECT count(*) FROM traffic_entries), (SELECT count(*) FROM permits)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(counts, (0, 0));
    drop(db);
    daemon.stop(libc::SIGTERM);
    fixture.report();
}

#[test]
fn synthetic_proven_local_endpoint_uses_negotiated_build_admission() {
    let _serial = support::serial();
    use arc_ipc::{
        governor::{GovernorJobRequest, GovernorJobResult},
        traffic::{self, JobRequest, JobResult},
    };
    let fixture = Fixture::new();
    let mut daemon = fixture.spawn_isolated("test");
    // Explicitly inject a synthetic local classification to exercise admission.
    let classification = arc_ipc::Classification {
        classification: arc_ipc::JobClass::Build,
        locality: arc_ipc::Locality::HostLocal,
        reason: arc_ipc::Reason::LocalEndpointBuild,
        uncertainty: arc_ipc::Uncertainty::None,
    };
    let id = JobId::try_from("c".repeat(32)).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let request = GovernorJobRequest::Traffic {
        request: JobRequest::Enqueue {
            job_id: id.clone(),
            observer: observer.clone(),
            classification,
            lineage_context: None,
        },
    };
    let legacy = protocol::call(&fixture, arc_ipc::governor::OPERATION_JOB, json!(request));
    assert_eq!(legacy.error.unwrap().code, arc_ipc::ERROR_INVALID_REQUEST);
    let capability = protocol::call(
        &fixture,
        traffic::endpoint::OPERATION_CAPABILITIES,
        serde_json::Value::Null,
    );
    assert_eq!(
        capability.result.unwrap(),
        json!(traffic::Capabilities::endpoint_supported())
    );
    let response = protocol::call(&fixture, traffic::endpoint::OPERATION_JOB, json!(request));
    let value = response.result.unwrap();
    let GovernorJobResult::Traffic {
        result: JobResult::Granted {
            permit_generation, ..
        },
    } = serde_json::from_value(value.clone()).unwrap()
    else {
        panic!("expected local Build reservation: {value}")
    };
    let stored = protocol::call(
        &fixture,
        OPERATION_JOB_STATUS_LOCALITY,
        json!(StatusRequest::ById { job_id: id.clone() }),
    )
    .result
    .unwrap();
    assert_eq!(stored["job"]["classification"], json!(classification));
    let cancel = GovernorJobRequest::Traffic {
        request: JobRequest::Cancel {
            job_id: id,
            observer,
            permit_generation: Some(permit_generation),
        },
    };
    let response = protocol::call(&fixture, traffic::endpoint::OPERATION_JOB, json!(cancel));
    assert!(matches!(
        serde_json::from_value(response.result.unwrap()),
        Ok(GovernorJobResult::Traffic {
            result: JobResult::Cancelled {}
        })
    ));
    daemon.stop(libc::SIGTERM);
    fixture.report();
}

#[test]
fn synthetic_local_endpoint_preserves_child_environment_and_lifecycle() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut daemon = fixture.spawn_isolated("test");
    let binary = fixture.binary("arc-exec-fixture");
    assert!(binary.is_file(), "build the arc-cli fixture binary first");
    symlink(&binary, fixture.root.join("docker")).unwrap();
    let key = fixture.root.join("fixture-key");
    fs::write(&key, b"arc-synthetic-endpoint-v1").unwrap();
    let string = |path: &std::path::Path| path.to_str().unwrap().to_owned();
    let mut expected = BTreeMap::from([
        ("HOME".to_owned(), string(&fixture.root)),
        ("PATH".to_owned(), string(&fixture.root)),
        (
            arc_core::ARC_SOCKET_PATH.to_owned(),
            string(&fixture.socket),
        ),
        ("ARC_FIXTURE_ENDPOINT_KEY".to_owned(), string(&key)),
        ("ARC_FIXTURE_ENV_REPORT".to_owned(), "1".to_owned()),
        ("FIXTURE_VALUE".to_owned(), "two words".to_owned()),
        ("FIXTURE_EMPTY".to_owned(), String::new()),
    ]);
    // Keep the coverage runner's destination through both env_clear and the
    // managed execution boundary. It is part of the intended environment.
    instrumentation::include_expected(&mut expected);

    for lineage in [None, Some(""), Some("inherited value")] {
        if let Some(value) = lineage {
            expected.insert("ARC_TRAFFIC_LINEAGE".into(), value.into());
        } else {
            expected.remove("ARC_TRAFFIC_LINEAGE");
        }
        let mut command = ProcessCommand::new(&binary);
        command
            .args([
                "--synthetic-local-endpoint-exec",
                "docker",
                "--context",
                "local",
                "build",
                ".",
            ])
            .env_clear()
            .envs(&expected)
            .current_dir(&fixture.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = child_watchdog::Owned::spawn(&mut command).unwrap();
        let stdout = child.child.stdout.take().unwrap();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
            let _ = sender.send(result);
        });
        let report = receiver
            .recv_timeout(Duration::from_secs(60))
            .expect("local endpoint child did not report before watchdog")
            .unwrap();
        let report: serde_json::Value = serde_json::from_str(&report).unwrap();
        child
            .child
            .stdin
            .take()
            .unwrap()
            .write_all(b"release\n")
            .unwrap();
        assert_eq!(child.wait("local endpoint child").unwrap().code(), Some(23));
        let mut actual: BTreeMap<String, String> =
            serde_json::from_value(report["environment"].clone()).unwrap();
        instrumentation::normalize_actual(&mut actual, &expected);
        assert_eq!(actual, expected, "lineage={lineage:?}");
        assert_eq!(
            report["argv"],
            json!(["docker", "--context", "local", "build", "."])
        );
        assert_eq!(
            report["cwd"],
            json!(fs::canonicalize(&fixture.root).unwrap())
        );

        let db = rusqlite::Connection::open(&fixture.database).unwrap();
        let id: String = db
            .query_row(
                "SELECT job_id FROM job_lifecycle WHERE child_pid=?1 AND classification_reason='LOCAL_ENDPOINT_BUILD'",
                [report["pid"].as_i64().unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        let id = JobId::try_from(id).unwrap();
        let snapshot = protocol::call(
            &fixture,
            OPERATION_JOB_STATUS_LOCALITY,
            json!(StatusRequest::ById { job_id: id.clone() }),
        )
        .result
        .unwrap();
        assert_eq!(snapshot["job"]["classification"]["classification"], "BUILD");
        assert_eq!(snapshot["job"]["classification"]["locality"], "HOST_LOCAL");
        assert_eq!(
            snapshot["job"]["classification"]["reason"],
            "LOCAL_ENDPOINT_BUILD"
        );
        assert_eq!(snapshot["job"]["child"]["pid"], report["pid"]);
        assert_eq!(snapshot["job"]["phase"], "COMPLETED");
        assert_eq!(snapshot["job"]["outcome"]["code"], 23);
        let reservations: (i64, i64, i64) = db
            .query_row(
                "SELECT (SELECT count(*) FROM traffic_entries WHERE job_id=?1 AND granted_at_ms IS NOT NULL), (SELECT count(*) FROM permits WHERE job_id=?1 AND released_at IS NOT NULL), (SELECT count(*) FROM permits WHERE job_id=?1 AND state='ACTIVE')",
                [id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(reservations, (1, 1, 0));
    }
    daemon.stop(libc::SIGTERM);
    fixture.report();
}

#[test]
fn synthetic_remote_facts_follow_the_actual_execution_path_once_while_build_is_held() {
    let _serial = support::serial();
    use arc_ipc::traffic::{JobRequest, JobResult, OPERATION_TRAFFIC_JOB};
    let fixture = Fixture::new();
    let mut daemon = fixture.spawn_isolated("test");
    let id = JobId::try_from("d".repeat(32)).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let local: arc_ipc::Classification = serde_json::from_value(json!({"classification":"BUILD", "locality":"HOST_LOCAL", "reason":"CARGO_BUILD", "uncertainty":"NONE"})).unwrap();
    let hold = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification: local,
        lineage_context: None,
    };
    let granted = protocol::call(&fixture, OPERATION_TRAFFIC_JOB, json!(hold))
        .result
        .unwrap();
    let JobResult::Granted {
        permit_generation, ..
    } = serde_json::from_value(granted).unwrap()
    else {
        panic!("local Build permit was not granted")
    };
    let db = rusqlite::Connection::open(&fixture.database).unwrap();
    let counts = || {
        db.query_row("SELECT (SELECT count(*) FROM traffic_entries), (SELECT count(*) FROM permits WHERE state='ACTIVE')", [], |row| Ok((row.get::<_,i64>(0)?, row.get::<_,i64>(1)?))).unwrap()
    };
    let baseline = counts();
    assert_eq!(baseline, (1, 1));
    let executable = fixture.root.join("docker");
    let marker = fixture.root.join("starts");
    let key = fixture.root.join("fixture-key");
    fs::write(&key, b"arc-synthetic-endpoint-v1").unwrap();
    fs::write(&executable, "#!/bin/sh\nprintf 'once\\n' >> \"$MARKER\"\nprintf 'arg=%s\\n' \"$@\"\nprintf 'value=%s\\n' \"$VALUE\"\nprintf 'cwd='; pwd -P\nread input\nprintf 'input=%s\\n' \"$input\"\nprintf 'child-stderr\\n' >&2\nexit 23\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let stdout = fixture.root.join("fixture-stdout");
    let stderr = fixture.root.join("fixture-stderr");
    let binary = fixture.binary("arc-exec-fixture");
    assert!(
        binary.is_file(),
        "build the arc-cli fixture binary before this focused test"
    );
    let mut command = ProcessCommand::new(binary);
    command
        .args([
            "--synthetic-endpoint-exec",
            "docker",
            "--context",
            "prod",
            "build",
            ".",
        ])
        .env_clear()
        .env("HOME", &fixture.root)
        .env("PATH", &fixture.root)
        .env(arc_core::ARC_SOCKET_PATH, &fixture.socket)
        .env("ARC_FIXTURE_ENDPOINT_KEY", key)
        .env("MARKER", &marker)
        .env("VALUE", "two words")
        .current_dir(&fixture.root)
        .stdin(Stdio::piped())
        .stdout(fs::File::create(&stdout).unwrap())
        .stderr(fs::File::create(&stderr).unwrap());
    instrumentation::forward(&mut command);
    let mut child = child_watchdog::Owned::spawn(&mut command).unwrap();
    child
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"payload\n")
        .unwrap();
    assert_eq!(
        child.wait("contextual execution fixture").unwrap().code(),
        Some(23)
    );
    assert_eq!(fs::read_to_string(&marker).unwrap(), "once\n");
    assert_eq!(
        fs::read_to_string(stdout).unwrap(),
        format!(
            "arg=--context\narg=prod\narg=build\narg=.\nvalue=two words\ncwd={}\ninput=payload\n",
            fs::canonicalize(&fixture.root).unwrap().display()
        )
    );
    assert_eq!(fs::read_to_string(stderr).unwrap(), "child-stderr\n");
    assert_eq!(counts(), baseline);
    let actual_id: String = db
        .query_row(
            "SELECT job_id FROM job_lifecycle WHERE classification_reason='REMOTE_ENDPOINT_EXEC'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let actual_id = JobId::try_from(actual_id).unwrap();
    let snapshot = protocol::call(
        &fixture,
        OPERATION_JOB_STATUS_LOCALITY,
        json!(StatusRequest::ById { job_id: actual_id }),
    )
    .result
    .unwrap();
    assert_eq!(
        snapshot["job"]["classification"]["classification"],
        "REMOTE_EXEC"
    );
    assert_eq!(
        snapshot["job"]["classification"]["reason"],
        "REMOTE_ENDPOINT_EXEC"
    );
    assert!(snapshot["job"]["child"]["pid"].is_u64());
    assert_eq!(snapshot["job"]["outcome"]["code"], 23);
    drop(db);
    let cancel = JobRequest::Cancel {
        job_id: id,
        observer,
        permit_generation: Some(permit_generation),
    };
    assert!(matches!(
        serde_json::from_value(
            protocol::call(&fixture, OPERATION_TRAFFIC_JOB, json!(cancel))
                .result
                .unwrap()
        ),
        Ok(JobResult::Cancelled {})
    ));
    daemon.stop(libc::SIGTERM);
    fixture.report();
}
