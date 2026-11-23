//! Overlap real Traffic requests while the original footprint is pending.
use crate::{journey, quality_support as support, runtime, Fixture, Sandbox};
use arc_ipc::{
    jobs::{JobId, Outcome, ProcessIdentity, StatusRequest, OPERATION_JOB_STATUS},
    traffic::{JobRequest, OPERATION_TRAFFIC_JOB},
};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

struct Restore(PathBuf);
impl Drop for Restore {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.0).unwrap();
    }
}

fn marker(gate: &Path, job: &JobId, suffix: &str) -> PathBuf {
    gate.join(format!("{}.{}", job.as_str(), suffix))
}

fn request(socket: &Path, payload: &JobRequest) -> Result<Value, String> {
    exchange(
        socket,
        OPERATION_TRAFFIC_JOB,
        serde_json::to_value(payload).unwrap(),
    )
}

fn exchange(socket: &Path, operation: &str, payload: Value) -> Result<Value, String> {
    let mut request = arc_ipc::Request::new("footprint-overlap", operation);
    request.payload = payload;
    let response =
        arc_ipc::exchange(&mut runtime::connect(socket), &request).map_err(|e| e.to_string())?;
    if response.status == "ok" {
        Ok(response.result.unwrap())
    } else {
        Err(format!("{response:?}"))
    }
}

fn threaded(
    socket: PathBuf,
    payload: JobRequest,
    mutate: Option<PathBuf>,
) -> mpsc::Receiver<Result<Value, String>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let response = request(&socket, &payload);
        if response.is_ok() {
            if let Some(path) = mutate {
                support::write_synced(&path, support::MIB);
            }
        }
        tx.send(response).unwrap();
    });
    rx
}

fn finished(rx: mpsc::Receiver<Result<Value, String>>) -> Result<Value, String> {
    rx.recv_timeout(Duration::from_secs(60))
        .expect("Traffic request watchdog")
}

fn wait(path: &Path) {
    journey::eventually(&format!("fixture barrier {}", path.display()), || {
        path.is_file()
    });
}

fn reading(db: &Connection, job: &JobId, phase: &str) -> (Option<i64>, Option<String>) {
    let sql = match phase {
        "start" => {
            "SELECT footprint_start_bytes,footprint_start_completeness FROM jobs WHERE id=?1"
        }
        "end" => "SELECT footprint_end_bytes,footprint_end_completeness FROM jobs WHERE id=?1",
        _ => unreachable!(),
    };
    db.query_row(sql, [job.as_str()], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
}

fn enqueue(job: &JobId, observer: &ProcessIdentity) -> JobRequest {
    JobRequest::Enqueue {
        job_id: job.clone(), observer: observer.clone(),
        classification: serde_json::from_value(json!({"classification":"BUILD", "locality":"HOST_LOCAL", "reason":"CARGO_BUILD", "uncertainty":"NONE"})).unwrap(),
        lineage_context: None, project: None, event: None, usage: None,
    }
}

fn overlap(
    f: &Fixture,
    db: &Connection,
    gate: &Path,
    directory: &Path,
    job: &JobId,
    phase: &str,
    requests: (JobRequest, JobRequest),
) -> Value {
    let (first, replay) = requests;
    let before = support::allocated(directory);
    fs::write(marker(gate, job, &format!("{phase}.arm")), b"").unwrap();
    let original = threaded(f.socket.clone(), first, None);
    wait(&marker(gate, job, &format!("{phase}.entered")));
    assert_eq!(reading(db, job, phase), (None, None));
    let _ = fs::remove_file(marker(gate, job, "waiting"));
    let replayed = threaded(
        f.socket.clone(),
        replay,
        Some(directory.join(format!("after-{phase}"))),
    );
    // The replay reached the same job's held guard, not merely a client thread.
    wait(&marker(gate, job, "waiting"));
    assert!(!directory.join(format!("after-{phase}")).exists());
    fs::write(marker(gate, job, &format!("{phase}.release")), b"").unwrap();
    let first = finished(original).unwrap();
    let second = finished(replayed).unwrap();
    assert_eq!(
        reading(db, job, phase),
        (Some(before as i64), Some("complete".into()))
    );
    assert!(support::allocated(directory) > before);
    assert_eq!(first["state"], second["state"]);
    first
}

pub fn traffic_grant_completion_and_cancelled_walk(
    f: &Fixture,
    s: &Sandbox,
    db: &Connection,
    gate: &Path,
) {
    let directory = s.project("overlapping-footprint");
    support::write_synced(&directory.join("existing"), support::MIB);
    let _restore = Restore(std::env::current_dir().unwrap());
    std::env::set_current_dir(&directory).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let job = JobId::try_from(arc_core::process_identity::random_job_id().unwrap()).unwrap();
    let granted = overlap(
        f,
        db,
        gate,
        &directory,
        &job,
        "start",
        (
            enqueue(&job, &observer),
            JobRequest::Poll {
                job_id: job.clone(),
                observer: observer.clone(),
            },
        ),
    );
    assert_eq!(granted["state"], "GRANTED");
    let complete = JobRequest::Complete {
        job_id: job.clone(),
        observer: observer.clone(),
        outcome: Outcome::Exited { code: 0 },
        permit_generation: granted["permit_generation"].as_u64().unwrap(),
    };
    let completed = overlap(
        f,
        db,
        gate,
        &directory,
        &job,
        "end",
        (complete.clone(), complete),
    );
    assert_eq!(completed["state"], "COMPLETED");
    successor_cannot_change_the_original_end_reading(f, db, gate, &directory, &observer);

    // A committed event whose handler reaches its deadline leaves a missing
    // original reading; a later release and replay cannot manufacture one.
    let late = JobId::try_from(arc_core::process_identity::random_job_id().unwrap()).unwrap();
    fs::write(marker(gate, &late, "start.arm"), b"").unwrap();
    let original = threaded(f.socket.clone(), enqueue(&late, &observer), None);
    wait(&marker(gate, &late, "start.entered"));
    let _ = finished(original); // deadline may close the request or return its error.
    assert_eq!(reading(db, &late, "start"), (None, None));
    let replay = request(
        &f.socket,
        &JobRequest::Poll {
            job_id: late.clone(),
            observer: observer.clone(),
        },
    )
    .unwrap();
    assert_eq!(replay["state"], "GRANTED");
    support::write_synced(&directory.join("after-cancel"), support::MIB);
    fs::write(marker(gate, &late, "start.release"), b"").unwrap();
    assert_eq!(reading(db, &late, "start"), (None, None));
    request(
        &f.socket,
        &JobRequest::Complete {
            job_id: late.clone(),
            observer,
            outcome: Outcome::Exited { code: 0 },
            permit_generation: replay["permit_generation"].as_u64().unwrap(),
        },
    )
    .unwrap();
    assert_eq!(reading(db, &late, "start"), (None, None));
}

/// A BUILD queued behind the original's permit in the same directory. The
/// original's end reading is taken before its completion releases that
/// permit: while the walk is held, the successor is still queued (so it
/// cannot run) and no lifecycle status reports the outcome. Once granted,
/// the successor changes the directory at once, and the original keeps the
/// reading of the directory it left.
fn successor_cannot_change_the_original_end_reading(
    f: &Fixture,
    db: &Connection,
    gate: &Path,
    directory: &Path,
    observer: &ProcessIdentity,
) {
    let new_job = || JobId::try_from(arc_core::process_identity::random_job_id().unwrap()).unwrap();
    let (original, successor) = (new_job(), new_job());
    let granted = request(&f.socket, &enqueue(&original, observer)).unwrap();
    assert_eq!(granted["state"], "GRANTED", "{granted}");
    let queued = request(&f.socket, &enqueue(&successor, observer)).unwrap();
    assert_eq!(queued["state"], "QUEUED", "{queued}");
    let poll = JobRequest::Poll {
        job_id: successor.clone(),
        observer: observer.clone(),
    };
    let left = support::allocated(directory);
    fs::write(marker(gate, &original, "end.arm"), b"").unwrap();
    let completion = threaded(
        f.socket.clone(),
        JobRequest::Complete {
            job_id: original.clone(),
            observer: observer.clone(),
            outcome: Outcome::Exited { code: 0 },
            permit_generation: granted["permit_generation"].as_u64().unwrap(),
        },
        None,
    );
    wait(&marker(gate, &original, "end.entered"));
    let held = request(&f.socket, &poll).unwrap();
    assert_eq!(
        held["state"], "QUEUED",
        "successor granted during the end walk: {held}"
    );
    let status = exchange(
        &f.socket,
        OPERATION_JOB_STATUS,
        serde_json::to_value(StatusRequest::ById {
            job_id: original.clone(),
        })
        .unwrap(),
    )
    .unwrap();
    assert_eq!(status["job"]["job_id"], original.as_str(), "{status}");
    assert!(
        status["job"]["outcome"].is_null(),
        "outcome exposed before its end reading: {status}"
    );
    assert_eq!(reading(db, &original, "end"), (None, None));
    fs::write(marker(gate, &original, "end.release"), b"").unwrap();
    assert_eq!(finished(completion).unwrap()["state"], "COMPLETED");
    let granted = request(&f.socket, &poll).unwrap();
    assert_eq!(granted["state"], "GRANTED", "{granted}");
    support::write_synced(&directory.join("successor"), support::MIB);
    assert!(support::allocated(directory) > left);
    assert_eq!(
        reading(db, &original, "end"),
        (Some(left as i64), Some("complete".into()))
    );
    let done = request(
        &f.socket,
        &JobRequest::Complete {
            job_id: successor,
            observer: observer.clone(),
            outcome: Outcome::Exited { code: 0 },
            permit_generation: granted["permit_generation"].as_u64().unwrap(),
        },
    )
    .unwrap();
    assert_eq!(done["state"], "COMPLETED");
}

/// A real wrapper dies after the daemon captures its cwd. A second BUILD
/// recovers its permit and changes that cwd before the old walk resumes.
pub fn recovery_cannot_replace_captured_end(
    f: &Fixture,
    s: &Sandbox,
    db: &Connection,
    gate: &Path,
) {
    use arc_core::process_identity::{probe_live, IdentityStatus};
    use std::process::Stdio;

    let directory = s.project("recovered-footprint");
    support::write_synced(&directory.join("existing"), support::MIB);
    let original_control = support::directory(f, "recovered-original-control");
    let successor_control = support::directory(f, "recovered-successor-control");
    let bin = f.root.join("traffic-quality-bin");
    let path = std::env::join_paths([&bin, &s.bin]).unwrap();
    let command = |mode: &str, control: &Path| {
        let mut command = s.arcctl(f, &directory);
        command
            .args(["exec", "--", "cargo", "build"])
            .env("PATH", &path)
            .env("ARC_QUALITY_MODE", mode)
            .env("ARC_QUALITY_CONTROL", control)
            .stdin(Stdio::null());
        command
    };
    let mut original =
        runtime::child_watchdog::Owned::spawn(&mut command("hold", &original_control)).unwrap();
    let root = support::published(&original_control, "hold.pid");
    let root_identity = ProcessIdentity::capture(root as u32).unwrap();
    let job = JobId::try_from(support::running_job(db, root)).unwrap();
    let mut successor =
        runtime::child_watchdog::Owned::spawn(&mut command("mutate", &successor_control)).unwrap();
    journey::eventually("successor BUILD queued before recovery", || {
        assert!(successor.child.try_wait().unwrap().is_none());
        journey::queue_state(db, successor.child.id()).as_deref() == Some("QUEUED")
    });
    fs::write(marker(gate, &job, "end.capture.arm"), b"").unwrap();
    support::release(&original_control);
    wait(&marker(gate, &job, "end.capture.entered"));
    assert_ne!(probe_live(&root_identity), IdentityStatus::VerifiedMatch);
    assert_eq!(reading(db, &job, "end"), (None, None));
    original.child.kill().unwrap();
    assert!(!original
        .wait("killed footprint observer")
        .unwrap()
        .success());

    let successor_pid = support::published(&successor_control, "mutate.pid");
    let successor_job = support::running_job(db, successor_pid);
    let states: (String, String, Option<i64>, Option<String>) = db
        .query_row(
            "SELECT t.state,p.state,j.footprint_end_bytes,j.footprint_end_completeness
         FROM traffic_entries t JOIN permits p ON p.job_id=t.job_id JOIN jobs j ON j.id=t.job_id
         WHERE t.job_id=?1",
            [job.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        states,
        (
            "ABANDONED".into(),
            "RELEASED".into(),
            None,
            Some("unavailable".into())
        )
    );
    assert!(directory.join("successor.bin").is_file());
    assert_eq!(reading(db, &job, "end"), (None, Some("unavailable".into())));
    let successor_allocation = support::allocated(&directory);

    // The already-authenticated request is still held inside the daemon.
    // Release it after the successor's mutation and require its late outcome
    // to commit while the durable unavailable reading remains unchanged.
    fs::write(marker(gate, &job, "end.capture.release"), b"").unwrap();
    wait(&marker(gate, &job, "end.capture.released"));
    let late_reading = marker(gate, &job, "end.capture.reading");
    wait(&late_reading);
    assert_eq!(
        fs::read_to_string(late_reading).unwrap(),
        format!("complete:{successor_allocation}"),
        "late walk did not measure the successor's changed directory"
    );
    journey::eventually("late original completion committed after recovery", || {
        db.query_row(
            "SELECT outcome_kind IS NOT NULL FROM job_lifecycle WHERE job_id=?1",
            [job.as_str()],
            |r| r.get::<_, bool>(0),
        )
        .unwrap()
    });
    assert_eq!(reading(db, &job, "end"), (None, Some("unavailable".into())));
    let successor_state: String = db
        .query_row(
            "SELECT state FROM traffic_entries WHERE job_id=?1",
            [&successor_job],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(successor_state, "RUNNING");
    support::release(&successor_control);
    assert!(successor
        .wait("successor footprint build")
        .unwrap()
        .success());
}
