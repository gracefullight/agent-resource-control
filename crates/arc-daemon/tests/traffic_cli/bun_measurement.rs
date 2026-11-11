//! Real one-shot Cargo and Bun tests, with barriers inside the test bodies.
use super::*;
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use serde_json::{json, Value};
use std::time::SystemTime;

#[path = "../support/bun_measurement.rs"]
mod process_witness;
use process_witness::{executable, identity, launch, observe_body, shim};

const TOOLS: [&str; 2] = ["cargo", "bun"];

fn sha256(path: &Path) -> String {
    let out = Command::new("/usr/bin/shasum")
        .arg("-a")
        .arg("256")
        .arg(path)
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

fn artifact(root: &Path, path: &Path) -> Value {
    let canonical = fs::canonicalize(path).unwrap();
    let relative = canonical
        .strip_prefix(root)
        .expect("artifact outside workspace");
    json!({"path":relative.to_str().unwrap(), "sha256":sha256(&canonical)})
}

fn event(events: &mut Vec<Value>, start: Instant, name: &str, job: usize) {
    events.push(json!({"seq":events.len(),"atUs":start.elapsed().as_micros(),"event":name,"tool":TOOLS[job]}));
}

fn project(path: &Path, tool: usize) {
    fs::create_dir_all(path).unwrap();
    if tool == 0 {
        fs::create_dir_all(path.join("src")).unwrap();
        fs::write(path.join("Cargo.toml"), "[package]\nname = \"mixed_traffic_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n").unwrap();
        fs::write(path.join("src/lib.rs"), r#"
#[test]
fn real_cargo_barrier() {
    use std::{fs::{self, OpenOptions}, io::Write, thread, time::Duration};
    let marker = std::env::var_os("MIXED_BODY_LOG").unwrap();
    let release = std::env::var_os("MIXED_RELEASE").unwrap();
    writeln!(OpenOptions::new().create(true).append(true).open(marker).unwrap(), "{}", std::process::id()).unwrap();
    while !std::path::Path::new(&release).exists() { thread::sleep(Duration::from_millis(10)); }
    fs::metadata(release).unwrap();
}
"#).unwrap();
    } else {
        fs::write(
            path.join("barrier.test.ts"),
            r#"
import { test, expect } from 'bun:test';
import { appendFileSync, existsSync } from 'node:fs';
test('real bun barrier', async () => {
  appendFileSync(process.env.MIXED_BODY_LOG!, `${process.pid}\n`);
  while (!existsSync(process.env.MIXED_RELEASE!)) await Bun.sleep(10);
  expect(true).toBe(true);
}, 120_000);
"#,
        )
        .unwrap();
    }
}

pub(super) fn real_bun() -> PathBuf {
    let output = Command::new("bun")
        .args(["-p", "process.execPath"])
        .output()
        .unwrap();
    assert!(output.status.success(), "bun executable unavailable");
    fs::canonicalize(String::from_utf8(output.stdout).unwrap().trim()).unwrap()
}

fn body_stopped(body: &Value) -> bool {
    let id: ProcessIdentity = serde_json::from_value(body["test"].clone()).unwrap();
    matches!(
        probe_live(&id),
        IdentityStatus::Missing | IdentityStatus::Mismatch
    )
}

fn unix_us() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_micros()
}

fn finish_job(child: &mut BuildChild, path: &Path) -> (i32, String, u128) {
    eventually(|| child.child.try_wait().unwrap().is_some());
    let code = child.child.wait().unwrap().code().unwrap();
    let output = format!(
        "{}\n{}",
        fs::read_to_string(path.join("stdout.log")).unwrap(),
        fs::read_to_string(path.join("stderr.log")).unwrap()
    );
    assert_eq!(code, 0, "{output}");
    (code, "EXITED:0".to_owned(), unix_us())
}

fn run_case(first: usize, managed: bool) {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut daemon = fixture.spawn("test");
    let daemon_identity = identity(daemon.child.id());
    let daemon_executable = executable(daemon.child.id());
    assert_eq!(
        daemon_executable,
        fs::canonicalize(binary("arc-daemon")).unwrap()
    );
    let daemon_observation = json!({"identity":daemon_identity,"executable":{"path":daemon_executable,"sha256":sha256(&daemon_executable)}});
    let root = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let case_id = format!(
        "{}_first-{}",
        TOOLS[first],
        if managed { "scheduled" } else { "bypass" }
    );
    let artifacts = match std::env::var_os("ARC_TRAFFIC_BUN_RUN_DIR") {
        Some(value) => {
            let path = PathBuf::from(value);
            assert!(path.starts_with(root.join("target/traffic-bun")));
            fs::create_dir_all(&path).unwrap();
            fs::canonicalize(path).unwrap().join(&case_id)
        }
        None => fixture.root.join(&case_id),
    };
    let paths = [artifacts.join("cargo"), artifacts.join("bun")];
    let shim_root = artifacts.join("shims");
    fs::create_dir_all(&shim_root).unwrap();
    for tool in TOOLS {
        shim(&shim_root, tool);
    }
    for (i, path) in paths.iter().enumerate() {
        project(path, i);
    }
    let paths = paths.map(|p| fs::canonicalize(p).unwrap());
    assert_ne!(paths[0], paths[1]);
    let real = [fs::canonicalize(env!("CARGO")).unwrap(), real_bun()];
    let second = 1 - first;
    let start = Instant::now();
    let mut events = Vec::new();
    let mut a = launch(
        &fixture,
        &paths[first],
        first,
        managed,
        &real[first],
        &shim_root,
        None,
    );
    let first_body = observe_body(&a, &paths[first], first, managed);
    let mut first_job_id = String::new();
    if managed {
        eventually(|| {
            let observed = database(&fixture).query_row(
                "SELECT classification_reason,child_pid FROM job_lifecycle WHERE observer_pid=?1",
                [a.child.id()],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<u32>>(1)?)),
            );
            if let Ok((reason, Some(_))) = observed {
                assert_eq!(
                    reason,
                    if first == 0 { "CARGO_TEST" } else { "BUN_TEST" },
                    "first command was not admitted as the expected direct test"
                );
            }
            entry(&fixture, a.child.id()).is_some_and(|row| row.0 == "RUNNING")
        });
        assert_eq!(
            entry(&fixture, a.child.id()).unwrap().1,
            first_body["tool"]["pid"].as_u64().map(|v| v as u32)
        );
        first_job_id = database(&fixture)
            .query_row(
                "SELECT job_id FROM job_lifecycle WHERE observer_pid=?1",
                [a.child.id()],
                |r| r.get(0),
            )
            .unwrap();
    }
    event(&mut events, start, "test-start", first);
    let mut b = launch(
        &fixture,
        &paths[second],
        second,
        managed,
        &real[second],
        &shim_root,
        if managed {
            Some((
                &first_job_id,
                first_body["test"]["pid"].as_u64().unwrap() as u32,
            ))
        } else {
            None
        },
    );
    let mut queue = Value::Null;
    if managed {
        eventually(|| {
            assert!(
                !paths[second].join("launches").exists(),
                "second tool launched before a queued notice and permit release"
            );
            fs::read_to_string(paths[second].join("stderr.log"))
                .unwrap()
                .contains("QUEUED\nWaiting to start.")
                && entry(&fixture, b.child.id()).is_some_and(|row| row.0 == "QUEUED")
        });
        let row = entry(&fixture, b.child.id()).unwrap();
        assert!(row.1.is_none() && row.3.is_none());
        assert!(!paths[second].join("launches").exists() && !paths[second].join("bodies").exists());
        assert_eq!(
            probe_live(&serde_json::from_value(first_body["test"].clone()).unwrap()),
            IdentityStatus::VerifiedMatch
        );
        assert_eq!(active(&fixture).1, 1);
        let job_id: String = database(&fixture)
            .query_row(
                "SELECT job_id FROM job_lifecycle WHERE observer_pid=?1",
                [b.child.id()],
                |r| r.get(0),
            )
            .unwrap();
        let (first_id, first_class, first_permit): (String, String, String) = database(&fixture)
            .query_row("SELECT t.job_id,j.job_class,p.state FROM traffic_entries t JOIN jobs j ON j.id=t.job_id JOIN job_lifecycle l ON l.job_id=t.job_id JOIN permits p ON p.job_id=t.job_id WHERE l.observer_pid=?1", [a.child.id()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!(first_class, "TEST");
        assert_eq!(first_permit, "ACTIVE");
        assert_eq!(first_id, first_job_id);
        queue = json!({"jobId":job_id,"state":row.0,"queuedAtMs":row.2.unwrap(),"activePermits":1,"childAbsent":true,"firstTestLive":true,"firstJobId":first_id,"firstClass":first_class,"firstPermitState":first_permit});
        event(&mut events, start, "queued", second);
        fs::copy(
            paths[second].join("stderr.log"),
            paths[second].join("queued.log"),
        )
        .unwrap();
        event(&mut events, start, "notice", second);
    }
    let mut second_body = Value::Null;
    let mut overlap = Value::Null;
    if !managed {
        second_body = observe_body(&b, &paths[second], second, false);
        event(&mut events, start, "test-start", second);
        assert_eq!(
            probe_live(&serde_json::from_value(first_body["test"].clone()).unwrap()),
            IdentityStatus::VerifiedMatch
        );
        assert_eq!(
            probe_live(&serde_json::from_value(second_body["test"].clone()).unwrap()),
            IdentityStatus::VerifiedMatch
        );
        overlap = json!({"first":first_body["test"], "second":second_body["test"],
            "bothLive":true, "observedAtUnixUs":unix_us()});
        fs::write(paths[second].join("release"), "release").unwrap();
    }
    fs::write(paths[first].join("release"), "release").unwrap();
    event(&mut events, start, "barrier-release", first);
    // Deliberately observe the second body before polling release. The shim's
    // single committed-state query precedes exec and never waits for a permit.
    let mut second_body_observed_at = None;
    if managed {
        second_body = observe_body(&b, &paths[second], second, true);
        second_body_observed_at = Some(unix_us());
        thread::sleep(Duration::from_millis(100));
    }
    // These are controller observations, not the times of the underlying exit
    // and commit; either may have happened before the second body's entry.
    let connection = database(&fixture);
    let mut first_end_us = None;
    let mut release_observation = None;
    let deadline = Instant::now() + Duration::from_secs(45);
    while first_end_us.is_none() || (managed && release_observation.is_none()) {
        if first_end_us.is_none() && body_stopped(&first_body) {
            first_end_us = Some(unix_us());
        }
        if managed && release_observation.is_none() {
            let released: Option<(String, Option<i64>)> = connection
                .query_row(
                    "SELECT state,released_at FROM permits WHERE job_id=?1",
                    [&first_job_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok();
            if let Some((state, Some(released_at))) = released {
                if state == "RELEASED" {
                    release_observation = Some((released_at, unix_us()));
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "first test or committed release not observed"
        );
        thread::sleep(Duration::from_millis(1));
    }
    let first_end_us = first_end_us.unwrap();
    let first_end = json!({"test":first_body["test"], "observedStoppedAtUnixUs":first_end_us});
    event(&mut events, start, "test-end", first);
    let mut permit = Value::Null;
    if managed {
        let (release, release_observed_us) = release_observation.unwrap();
        let release_state = "RELEASED";
        event(&mut events, start, "permit-release", first);
        eventually(|| entry(&fixture, b.child.id()).is_some_and(|row| row.3.is_some()));
        let grant = entry(&fixture, b.child.id()).unwrap().3.unwrap();
        let grant_state: String = database(&fixture)
            .query_row(
                "SELECT state FROM permits WHERE job_id=?1",
                [queue["jobId"].as_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(grant_state, "ACTIVE");
        assert!(grant >= release);
        event(&mut events, start, "grant", second);
        assert!(second_body_observed_at.unwrap() < release_observed_us);
        let witness = fs::read_to_string(paths[second].join("committed-release")).unwrap();
        assert_eq!(
            witness,
            format!(
                "{} {} {} RELEASED {} {} ACTIVE {} {}\n",
                second_body["tool"]["pid"].as_u64().unwrap(),
                first_body["test"]["pid"].as_u64().unwrap(),
                first_job_id,
                release,
                queue["jobId"].as_str().unwrap(),
                grant,
                queue["queuedAtMs"].as_i64().unwrap()
            )
        );
        // Darwin process identities use Unix microseconds. The durable release
        // time is truncated to milliseconds, so the same millisecond cannot
        // establish the order, even if the shim later observes the commit.
        let start_us: u128 = second_body["tool"]["start_token"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            start_us / 1000 > u128::try_from(release).unwrap(),
            "second tool kernel start is before or ambiguous with the release"
        );
        let launch_ms = fs::metadata(paths[second].join("launches"))
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        assert!(launch_ms >= release as u128);
        permit = json!({"releasedAtMs":release,"releaseObservedAtUnixUs":release_observed_us,
            "releaseState":release_state,"releaseJobId":queue["firstJobId"],"grantedAtMs":grant,
            "grantState":grant_state,"grantJobId":queue["jobId"],"queueWaitMs":grant - queue["queuedAtMs"].as_i64().unwrap(),"secondLaunchAtMs":launch_ms,
            "secondBodyObservedAtUnixUs":second_body_observed_at.unwrap()});
        event(&mut events, start, "test-start", second);
        fs::write(paths[second].join("release"), "release").unwrap();
    }
    eventually(|| body_stopped(&second_body));
    let second_end = json!({"test":second_body["test"], "observedStoppedAtUnixUs":unix_us()});
    event(&mut events, start, "test-end", second);
    let (first_code, first_outcome, first_wrapper_exit) = finish_job(&mut a, &paths[first]);
    let (second_code, second_outcome, second_wrapper_exit) = finish_job(&mut b, &paths[second]);
    if managed {
        assert!(fs::read_to_string(paths[second].join("stderr.log"))
            .unwrap()
            .contains("STARTED\nCommand started."));
    }
    let mut bodies = [Value::Null, Value::Null];
    bodies[first] = first_body;
    bodies[second] = second_body;
    let mut exits = [0, 0];
    exits[first] = first_code;
    exits[second] = second_code;
    let mut outcomes = [String::new(), String::new()];
    outcomes[first] = first_outcome;
    outcomes[second] = second_outcome;
    let mut wrapper_exits = [0, 0];
    wrapper_exits[first] = first_wrapper_exit;
    wrapper_exits[second] = second_wrapper_exit;
    for path in &paths {
        assert_eq!(
            fs::read_to_string(path.join("launches"))
                .unwrap()
                .lines()
                .count(),
            1
        );
        assert_eq!(
            fs::read_to_string(path.join("bodies"))
                .unwrap()
                .lines()
                .count(),
            1
        );
    }
    if managed {
        for child in [&a, &b] {
            eventually(|| {
                entry(&fixture, child.child.id()).is_some_and(|row| row.0 == "COMPLETED")
            });
        }
    }
    let mut records = [Value::Null, Value::Null];
    if managed {
        for (i, child) in [(first, &a), (second, &b)] {
            let record: (String, String, Option<i64>, Option<String>, String) = database(&fixture)
                .query_row("SELECT t.job_id,t.state,j.exit_code,l.outcome_kind,j.job_class FROM traffic_entries t JOIN jobs j ON j.id=t.job_id JOIN job_lifecycle l ON l.job_id=t.job_id WHERE l.observer_pid=?1", [child.child.id()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).unwrap();
            assert_eq!(record.1, "COMPLETED");
            assert_eq!(record.2, Some(0));
            assert_eq!(record.3.as_deref(), Some("EXITED"));
            assert_eq!(record.4, "TEST");
            records[i] = json!({"jobId":record.0,"state":record.1,"exitCode":record.2,"outcomeKind":record.3,"class":record.4});
        }
        assert_eq!(records[second]["jobId"], queue["jobId"]);
        assert_ne!(records[first]["jobId"], records[second]["jobId"]);
    }
    if artifacts.starts_with(root.join("target/traffic-bun")) {
        let jobs: Vec<Value> = (0..2).map(|i| json!({"tool":TOOLS[i], "cwd":paths[i], "process":bodies[i],
            "shim":artifact(&root, &shim_root.join(TOOLS[i])),
            "launches":artifact(&root, &paths[i].join("launches")), "bodies":artifact(&root, &paths[i].join("bodies")),
            "source":artifact(&root, &paths[i].join(if i == 0 {"src/lib.rs"} else {"barrier.test.ts"})),
            "manifest":if i == 0 {artifact(&root, &paths[i].join("Cargo.toml"))} else {Value::Null},
            "stdout":artifact(&root, &paths[i].join("stdout.log")), "stderr":artifact(&root, &paths[i].join("stderr.log")),
            "argv":if i == 0 { vec!["test", "--offline", "--", "--nocapture"] } else { vec!["test"] },
            "record":records[i], "exitCode":exits[i],"outcome":outcomes[i], "wrapperExitObservedAtUnixUs":wrapper_exits[i]})).collect();
        let notice = if managed {
            artifact(&root, &paths[second].join("queued.log"))
        } else {
            Value::Null
        };
        println!(
            "TRAFFIC_BUN_CASE {}",
            json!({"schemaVersion":1,"id":case_id,"order":TOOLS[first],"daemon":daemon_observation,
            "mode":if managed {"scheduled"} else {"bypass"}, "jobs":jobs,"events":events,"queue":queue,"permit":permit,"notice":notice,
            "firstEndObservation":first_end, "secondEndObservation":second_end,
            "overlapObservation":overlap,
            "sqliteExecutable":if managed {{let path = fs::canonicalize("/usr/bin/sqlite3").unwrap(); json!({"path":path,"sha256":sha256(&path)})}} else {Value::Null},
            "committedRelease":if managed {artifact(&root, &paths[second].join("committed-release"))} else {Value::Null}})
        );
    }
    daemon.stop(libc::SIGTERM);
    fixture.report();
}

#[test]
fn mixed_cargo_first_bypass() {
    run_case(0, false);
}
#[test]
fn mixed_bun_first_bypass() {
    run_case(1, false);
}
#[test]
fn mixed_cargo_first_scheduled() {
    run_case(0, true);
}
#[test]
fn mixed_bun_first_scheduled() {
    run_case(1, true);
}
