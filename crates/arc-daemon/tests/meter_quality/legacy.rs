//! Built-CLI exchanges with a peer implementing only the legacy Meter reads.
use crate::{
    journey::Sandbox,
    runtime::{child_watchdog, Fixture},
};
use arc_ipc::{Request, Response};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    os::{
        fd::AsRawFd,
        unix::net::{UnixListener, UnixStream},
    },
    path::Path,
    thread,
    time::{Duration, Instant},
};

const QUERY: &str = r#"{"bucket":"hour","by":"vendor","from_ms":1,"to_ms":2}"#;
const REPLY: &str = r#"{"generated_at_ms":3,"groups":[{"bucket_start_ms":0,"local":{"cpu_core_seconds":{"value":1.25}},"vendor":"none"}],"query":{"bucket":"hour","by":"vendor","from_ms":1,"to_ms":2},"schema_version":1}"#;
const CLI: &str = "{\"schema_version\":1,\"query\":{\"by\":\"vendor\",\"from_ms\":1,\"to_ms\":2,\"bucket\":\"hour\"},\"generated_at_ms\":3,\"groups\":[{\"bucket_start_ms\":0,\"local\":{\"cpu_core_seconds\":{\"value\":1.25}},\"vendor\":\"none\"}]}\n";
const JOB: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TREE: &str = r#"{"completeness":"partial","cpu_core_seconds":{"basis":"tree_samples","value":1.25},"disk_read_bytes":{"basis":"unobserved","value":null},"disk_write_bytes":{"basis":"tree_samples","value":8},"job_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","memory_peak_bytes":{"basis":"tree_samples","value":16},"missing_coverage":["unobserved_descendants"],"processes":1,"root_sampled_cpu_core_seconds":{"basis":"tree_samples","value":1.0},"samples":2,"schema_version":1,"terminal":true,"unwaited_cpu_core_seconds":{"basis":"tree_samples","value":0.25},"waited_cpu_core_seconds":{"basis":"waited_rusage","value":1.0}}"#;

fn accept(listener: &UnixListener, closed_expected: bool) -> UnixStream {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                if !closed_expected {
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(60)))
                        .unwrap();
                }
                return stream;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "legacy peer accept watchdog");
                thread::sleep(Duration::from_millis(5));
            }
            Err(e) => panic!("legacy peer accept: {e}"),
        }
    }
}

fn exchange(listener: &UnixListener, operation: &str, payload: &str, result: &str) {
    let mut stream = accept(listener, false);
    let bytes = arc_ipc::read_frame(&mut stream).unwrap();
    let request = Request::decode(&bytes).unwrap();
    let id = serde_json::to_string(&request.request_id).unwrap();
    // The envelope is sorted by write_message; only its request id varies.
    let expected = format!("{{\"operation\":\"{operation}\",\"payload\":{payload},\"protocol_version\":1,\"request_id\":{id}}}");
    assert_eq!(bytes, expected.as_bytes(), "legacy request bytes");
    let response = Response::ok(&request.request_id, serde_json::from_str(result).unwrap());
    let bytes = serde_json::to_vec(&serde_json::to_value(response).unwrap()).unwrap();
    let expected = format!(
        "{{\"protocol_version\":1,\"request_id\":{id},\"result\":{result},\"status\":\"ok\"}}"
    );
    assert_eq!(bytes, expected.as_bytes(), "legacy reply bytes");
    arc_ipc::write_frame(&mut stream, &bytes).unwrap();
}

fn listener(root: &Path) -> (UnixListener, std::path::PathBuf) {
    fs::create_dir(root).unwrap();
    let socket = root.join("arc.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    (listener, socket)
}

fn advertise(root: &Path, name: &str) -> File {
    let file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(root.join(name))
        .unwrap();
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as _;
    lock.l_whence = libc::SEEK_SET as _;
    assert_eq!(
        unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &lock) },
        0
    );
    file
}

fn old_summary(f: &Fixture, s: &Sandbox, project: &Path) {
    let (peer, socket) = listener(&f.root.join("legacy-summary"));
    let server = thread::spawn(move || exchange(&peer, "meter.usage_summary", QUERY, REPLY));
    let output = child_watchdog::output(
        s.arcctl(f, project)
            .env(arc_core::ARC_SOCKET_PATH, socket)
            .args([
                "meter", "summary", "--by", "vendor", "--from", "1", "--to", "2", "--bucket",
                "hour", "--json",
            ]),
        "legacy summary CLI",
    )
    .unwrap();
    server.join().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        output.stdout,
        CLI.as_bytes(),
        "legacy CLI bytes, including newline"
    );
    assert!(output.stderr.is_empty());
}

fn advertised_fallback(f: &Fixture, s: &Sandbox, project: &Path) {
    let root = f.root.join("replaced-summary");
    let (peer, socket) = listener(&root);
    let advertisement = advertise(&root, arc_ipc::meter::usage::ADVERTISEMENT_V2);
    let server = thread::spawn(move || {
        let mut stream = accept(&peer, false);
        let request = Request::decode(&arc_ipc::read_frame(&mut stream).unwrap()).unwrap();
        assert_eq!(request.operation, "meter.usage_summary.v2");
        assert_eq!(serde_json::to_string(&request.payload).unwrap(), QUERY);
        arc_ipc::write_response(
            &mut stream,
            &Response::error(&request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY),
        )
        .unwrap();
        exchange(&peer, "meter.usage_summary", QUERY, REPLY);
    });
    let output = child_watchdog::output(
        s.arcctl(f, project)
            .env(arc_core::ARC_SOCKET_PATH, socket)
            .args([
                "meter", "summary", "--by", "vendor", "--from", "1", "--to", "2", "--bucket",
                "hour", "--json",
            ]),
        "advertised v2 fallback",
    )
    .unwrap();
    server.join().unwrap();
    drop(advertisement);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, CLI.as_bytes());
    assert!(output.stderr.is_empty());
}

fn job_run(f: &Fixture, s: &Sandbox, project: &Path, name: &str, quality: bool) -> Vec<u8> {
    let root = f.root.join(name);
    let (peer, socket) = listener(&root);
    // This legacy peer advertises job-tree v1, but not job-tree-quality.
    let advertisement = advertise(&root, arc_ipc::meter::job_tree::ADVERTISEMENT);
    let server = thread::spawn(move || {
        let payload = format!(r#"{{"job_id":"{JOB}"}}"#);
        let summary = format!(
            r#"{{"accounting":{{"state":"missing"}},"job_id":"{JOB}","queue_wait":{{"basis":"unavailable","clock":"unknown","seconds":null}},"schema_version":1}}"#
        );
        exchange(&peer, "meter.summary", &payload, &summary);
        if quality {
            // No quality advertisement: the CLI authenticates this peer and
            // closes without sending an extension request, then falls back
            // to the v1 tree read.
            let mut stream = accept(&peer, true);
            stream.set_nonblocking(true).unwrap();
            crate::journey::eventually("old peer receives no quality request", || {
                match stream.read(&mut [0u8; 1]) {
                    Ok(0) => true,
                    Ok(_) => panic!("quality request sent without an advertisement"),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
                    Err(e) => panic!("legacy quality fallback: {e}"),
                }
            });
        }
        let tree = format!(r#"{{"job_id":"{JOB}","process_tree":{TREE},"schema_version":1}}"#);
        exchange(&peer, "meter.job_tree", &payload, &tree);
    });
    let mut command = s.arcctl(f, project);
    command
        .env(arc_core::ARC_SOCKET_PATH, socket)
        .args(["meter", "summary", "--job", JOB]);
    if quality {
        command.arg("--quality");
    }
    command.arg("--json");
    let output = child_watchdog::output(&mut command, "legacy job CLI").unwrap();
    server.join().unwrap();
    drop(advertisement);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty());
    let expected = format!("{{\"accounting\":{{\"state\":\"missing\"}},\"job_id\":\"{JOB}\",\"process_tree\":{TREE},\"queue_wait\":{{\"basis\":\"unavailable\",\"clock\":\"unknown\",\"seconds\":null}},\"schema_version\":1}}\n");
    assert_eq!(output.stdout, expected.as_bytes(), "legacy job CLI bytes");
    output.stdout
}

pub fn assert_legacy_bytes(f: &Fixture, s: &Sandbox, project: &Path) {
    old_summary(f, s, project);
    advertised_fallback(f, s, project);
    assert_eq!(
        job_run(f, s, project, "legacy-job", false),
        job_run(f, s, project, "legacy-job-quality-fallback", true)
    );
    // An incomplete explicit range remains a usage error before any socket call.
    for args in [
        vec![
            "meter", "summary", "--by", "vendor", "--from", "1", "--json",
        ],
        vec![
            "meter", "summary", "--by", "vendor", "--to", "2", "--bucket", "hour", "--json",
        ],
    ] {
        let output =
            child_watchdog::output(s.arcctl(f, project).args(args), "partial range").unwrap();
        assert!(!output.status.success(), "{output:?}");
    }
}
