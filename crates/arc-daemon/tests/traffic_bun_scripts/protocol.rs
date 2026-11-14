//! Lost acknowledgments are observed after committed writes, never guessed.
use super::{projects::*, *};
use std::{
    io::BufRead,
    os::unix::net::{UnixListener, UnixStream},
};

pub fn bounded_stream(stream: UnixStream) -> UnixStream {
    // Accepted macOS sockets inherit the listener's nonblocking flag. Timeouts
    // only bound frame IO after restoring blocking mode on each accepted stream.
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
}

pub fn checkpoint(listener: &UnixListener) -> (UnixStream, Value) {
    listener.set_nonblocking(true).unwrap();
    let mut got = None;
    eventually(|| match listener.accept() {
        Ok((s, _)) => {
            got = Some(s);
            true
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
        Err(e) => panic!("checkpoint accept: {e}"),
    });
    let stream = bounded_stream(got.unwrap());
    let mut line = String::new();
    std::io::BufReader::new(&stream)
        .read_line(&mut line)
        .unwrap();
    (stream, serde_json::from_str(&line).unwrap())
}

#[test]
fn checkpoint_streams_have_bounded_bidirectional_io() {
    use std::os::fd::AsRawFd;

    let fixture = native::Fixture::with_binary(binary("arc-daemon"));
    let path = fixture.root.join("checkpoint.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let mut client = UnixStream::connect(path).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client.write_all(b"{\"checkpoint\":true}\n").unwrap();
    let (mut accepted, observed) = checkpoint(&listener);
    assert_eq!(observed, json!({"checkpoint": true}));
    let flags = unsafe { libc::fcntl(accepted.as_raw_fd(), libc::F_GETFL) };
    assert!(flags >= 0);
    assert_eq!(flags & libc::O_NONBLOCK, 0);
    assert_eq!(
        accepted.read_timeout().unwrap(),
        Some(Duration::from_secs(2))
    );
    assert_eq!(
        accepted.write_timeout().unwrap(),
        Some(Duration::from_secs(2))
    );
    accepted.write_all(b"x").unwrap();
    use std::io::Read;
    let mut reply = [0];
    client.read_exact(&mut reply).unwrap();
    assert_eq!(reply, *b"x");
}

fn payload(request: &arc_ipc::traffic::JobRequest) -> Value {
    json!(arc_ipc::governor::GovernorJobRequest::Traffic {
        request: request.clone()
    })
}
fn lose(case: &Case, request: &arc_ipc::traffic::JobRequest) -> UnixStream {
    let mut wire = arc_ipc::Request::new(
        "lost-script-ack",
        arc_ipc::traffic::OPERATION_BUN_SCRIPTS_JOB,
    );
    wire.payload = payload(request);
    let mut stream = case.fixture.connect();
    arc_ipc::write_message(&mut stream, &wire).unwrap();
    stream
}
fn replay(case: &Case, request: &arc_ipc::traffic::JobRequest) -> Value {
    case.rpc(
        arc_ipc::traffic::OPERATION_BUN_SCRIPTS_JOB,
        payload(request),
    )
}
pub fn acknowledgments(read: Read) {
    use arc_ipc::{
        jobs::{JobId, Outcome},
        traffic::JobRequest,
    };
    let case = Case::new("lost-acks", read, false);
    let id = JobId::try_from("1234567890abcdef1234567890abcdef".to_owned()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let class:arc_ipc::Classification=serde_json::from_value(json!({"classification":"TEST","locality":"HOST_LOCAL","reason":"BUN_SCRIPT_TEST","uncertainty":"NONE"})).unwrap();
    let enqueue = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification: class,
        lineage_context: None,
    };
    let lost = lose(&case, &enqueue);
    eventually(|| case.active()["permits"] == 1);
    drop(lost);
    let grant = replay(&case, &enqueue);
    assert_eq!(replay(&case, &enqueue), grant);
    assert_eq!(grant["result"]["state"], "GRANTED");
    let generation = grant["result"]["permit_generation"].as_u64().unwrap();
    let p = project(&case, "actual-bun-child", Kind::Test, true);
    let mut cmd = Command::new(case.tools.shims.join("bun"));
    cmd.args(["run", "unexpected"])
        .current_dir(&p)
        .env(
            "PATH",
            format!(
                "{}:{}",
                case.tools.shims.display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .env("TBS_PROJECT", &p)
        .env("TBS_BODY", p.join("bodies"))
        .env("TBS_RELEASE", p.join("release"))
        .stdout(File::create(p.join("stdout")).unwrap())
        .stderr(File::create(p.join("stderr")).unwrap())
        .stdin(Stdio::null());
    let wrapper = native::OwnedChild::new(cmd.spawn().unwrap());
    let child = ProcessIdentity::capture(wrapper.child.id()).unwrap();
    let mut run = Run {
        wrapper,
        identity: child.clone(),
        project: p.clone(),
        fifo: None,
    };
    eventually(|| !log(&p.join("bodies")).is_empty());
    assert_eq!(probe_live(&child), IdentityStatus::VerifiedMatch);
    let attach = JobRequest::Attach {
        job_id: id.clone(),
        observer: observer.clone(),
        child: child.clone(),
        permit_generation: generation,
    };
    let lost = lose(&case, &attach);
    eventually(|| case.row(id.as_str())["state"] == "RUNNING");
    drop(lost);
    assert_eq!(
        replay(&case, &attach),
        json!({"state":"TRAFFIC","result":{"state":"RUNNING"}})
    );
    let running = case.reconcile(id.as_str(), "TEST", "BUN_SCRIPT_TEST");
    run.finish(0);
    validate_artifact(&case, &p, Kind::Test);
    let complete = JobRequest::Complete {
        job_id: id.clone(),
        observer,
        permit_generation: generation,
        outcome: Outcome::Exited { code: 0 },
    };
    let lost = lose(&case, &complete);
    eventually(|| case.active()["permits"] == 0);
    drop(lost);
    let completed = replay(&case, &complete);
    assert_eq!(replay(&case, &complete), completed);
    assert_eq!(
        completed["result"]["outcome"],
        json!({"kind":"EXITED","code":0})
    );
    let terminal = case.reconcile(id.as_str(), "TEST", "BUN_SCRIPT_TEST");
    let old = case.rpc(
        arc_ipc::jobs::OPERATION_JOB_STATUS,
        json!(arc_ipc::jobs::StatusRequest::ById { job_id: id.clone() }),
    );
    assert_eq!(
        old["job"]["classification"],
        json!({"classification":"UNKNOWN","locality":"UNKNOWN","reason":"UNRESOLVED_COMMAND","uncertainty":"OBSERVE"})
    );
    assert_eq!(case.rows("SELECT count(*) AS n FROM permits")[0]["n"], 1);
    assert_eq!(case.rows("SELECT count(*) AS n FROM jobs")[0]["n"], 1);
    assert_eq!(run.count("bun"), 1);
    assert_eq!(log(&p.join("bodies")).lines().count(), 1);
    case.save("lost-acks",json!({"scope":"Raw authenticated client loses committed replies; real child executes once. CLI sole-spawn and wait4 Meter proven separately.","grant":grant,"running":running,"completed":terminal,"legacyProjection":old,"generation":generation,"originalStarts":1,"permitRows":1,"activePermitsAtEnd":0}));
    case.done();
}
pub fn before_enqueue_cancel(read: Read) {
    let case = Case::new("pre-enqueue-cancel", read, false);
    let p = project(&case, "never-started", Kind::Test, false);
    let path = case.fixture.root.join("before.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let mut run = Run::start_with(&case, &p, &Kind::Test.argv(), |cmd| {
        cmd.env("ARC_PRESSURE_ACCEPTANCE_STAGE", "before_enqueue")
            .env("ARC_PRESSURE_ACCEPTANCE_BARRIER", &path);
    });
    let (_stream, request) = checkpoint(&listener);
    assert_eq!(request["request"]["action"], "ENQUEUE");
    assert_eq!(case.rows("SELECT count(*) AS n FROM jobs")[0]["n"], 0);
    assert_eq!(run.count("bun"), 0);
    assert_eq!(
        unsafe { libc::kill(run.wrapper.child.id() as i32, libc::SIGINT) },
        0
    );
    assert!(!run.wait().success());
    assert_eq!(case.rows("SELECT count(*) AS n FROM jobs")[0]["n"], 0);
    assert_eq!(run.count("bun"), 0);
    assert_eq!(case.active()["permits"], 0);
    assert!(log(&p.join("stderr")).contains("NOT_STARTED\nExecution cancelled."));
    case.save("before-enqueue-cancel",json!({"checkpoint":request,"originalStarts":0,"jobRows":0,"activePermits":0,"stderr":log(&p.join("stderr"))}));
    case.done();
}
