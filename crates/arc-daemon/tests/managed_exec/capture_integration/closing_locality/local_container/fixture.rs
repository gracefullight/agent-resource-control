//! Offline fixtures for the local container tests: an isolated Docker
//! configuration, the socket inventory that proves a local engine, and one
//! build per production route. No Docker engine is started or contacted.
// The socket inventory is injected only on macOS; elsewhere nothing proves a
// local engine and the tests that need one are not compiled.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]
use super::*;
use std::os::{fd::AsRawFd, unix::net::UnixListener};

/// sha256("local"): Docker's store directory for the fixture context.
const LOCAL_CONTEXT: &str = "25bf8e1a2393f1108d37029b3df5593236c755742ec93465bbafa9b290bddcf6";

pub(super) const BUILDX: [&str; 4] = ["docker", "buildx", "build", "."];
pub(super) const BUILD: [&str; 3] = ["docker", "build", "."];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Route {
    Exec,
    HookExec,
}

impl Route {
    pub(super) const BOTH: [Self; 2] = [Self::Exec, Self::HookExec];

    fn verb(self) -> &'static str {
        match self {
            Self::Exec => "exec",
            Self::HookExec => "hook-exec",
        }
    }
}

/// The one record every local container build must have, per reader.
pub(super) fn container_record(locality: &str) -> Value {
    json!({"classification":"BUILD","locality":locality,
        "reason":"LOCAL_ENDPOINT_BUILD","uncertainty":"NONE"})
}

/// An isolated Docker context and Buildx builder that both name one fixture
/// Unix socket, plus the inventory proving a local engine listens there. The
/// listener is this test's: any connection to it is a contact.
pub(super) struct LocalEngine {
    listener: UnixListener,
    connections: PathBuf,
    pub(super) host: String,
    vars: Vec<(&'static str, String)>,
}

impl LocalEngine {
    #[cfg(target_os = "macos")]
    pub(super) fn proven(f: &Fixture, s: &Sandbox) -> Self {
        let source = s.home.join("socket-inventory.c");
        let library = s.home.join("socket-inventory.dylib");
        write(
            &source,
            include_str!("../docker_controls/socket_inventory.c"),
        );
        let mut compiler = Command::new("/usr/bin/cc");
        compiler
            .env_clear()
            .env("HOME", &s.home)
            .env("PATH", "/usr/bin:/bin")
            .args(["-dynamiclib", "-o"])
            .arg(&library)
            .arg(&source);
        instrumentation::forward(&mut compiler);
        let result =
            child_watchdog::output(&mut compiler, "socket inventory fixture compiler").unwrap();
        assert!(result.status.success(), "{result:?}");

        let socket = f.root.join("local-docker.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let socket = socket.to_str().unwrap().to_owned();
        let host = format!("unix://{socket}");
        let docker = s.home.join("docker-config");
        let buildx = s.home.join("buildx-config");
        write(&docker.join("config.json"), r#"{"currentContext":"local"}"#);
        write(
            &docker
                .join("contexts/meta")
                .join(LOCAL_CONTEXT)
                .join("meta.json"),
            json!({"Name":"local","Endpoints":{"docker":{"Host":host}}}).to_string(),
        );
        write(
            &buildx.join("instances/local-builder"),
            json!({"Name":"local-builder","Driver":"docker-container","Dynamic":false,
                "Nodes":[{"Name":"node0","Endpoint":"local"}]})
            .to_string(),
        );
        write(
            &buildx.join("instances/remote-builder"),
            json!({"Name":"remote-builder","Driver":"remote","Dynamic":false,
                "Nodes":[{"Name":"node0","Endpoint":"tcp://192.0.2.21:1234"}]})
            .to_string(),
        );
        write(
            &buildx.join("current"),
            json!({"Key":"local","Name":"local-builder","Global":false}).to_string(),
        );
        let path = |path: &Path| path.to_str().unwrap().to_owned();
        Self {
            listener,
            connections: f.root.join("docker-socket-connections"),
            host,
            vars: vec![
                (
                    preload_key(),
                    format!("{}:{}", s.interfaces.display(), library.display()),
                ),
                ("ARC_TEST_DOCKER_SOCKET", socket),
                ("ARC_TEST_DOCKER_LISTENER", "dockerd".into()),
                ("DOCKER_CONFIG", path(&docker)),
                ("BUILDX_CONFIG", path(&buildx)),
            ],
        }
    }

    pub(super) fn env(&self) -> Vec<(&str, Option<&str>)> {
        self.vars
            .iter()
            .map(|(key, value)| (*key, Some(value.as_str())))
            .collect()
    }

    /// Drain the listener into the stub's connection log and count its lines.
    /// A connect to a Unix socket is queued synchronously, so nothing here
    /// depends on how long the client ran.
    pub(super) fn connections(&self) -> usize {
        loop {
            match self.listener.accept() {
                Ok(_) => {
                    let mut log = fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&self.connections)
                        .unwrap();
                    log.write_all(b"connection\n").unwrap();
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("docker stub socket: {error}"),
            }
        }
        fs::read_to_string(&self.connections)
            .map(|log| log.lines().count())
            .unwrap_or(0)
    }
}

/// The installed Claude hook's rewrite of one Bash command, or `None` when the
/// hook leaves it to the vendor's shell. The hook itself starts nothing.
pub(super) fn installed_hook_rewrite(s: &Sandbox, f: &Fixture, source: &str) -> Option<String> {
    let install = child_watchdog::output(
        s.command(f, &[])
            .args(["hooks", "install", "claude", "--scope", "user"]),
        "install hook for container builds",
    )
    .unwrap();
    assert!(install.status.success(), "{install:?}");
    let settings: Value =
        serde_json::from_slice(&fs::read(s.home.join(".claude/settings.json")).unwrap()).unwrap();
    let installed = settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    let mut event: Value = serde_json::from_str(include_str!(
        "../../../../../../arc-cli/tests/fixtures/agent_hooks/claude-recorded-2.1.327-cargo-build.json"
    ))
    .unwrap();
    event["cwd"] = json!(s.repo);
    event["tool_input"]["command"] = json!(source);
    let template = s.command(f, &[]);
    let mut hook = Command::new("/bin/sh");
    hook.env_clear()
        .args(["-c", installed])
        .current_dir(&s.repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in template.get_envs() {
        if let Some(value) = value {
            hook.env(key, value);
        }
    }
    let before = s.calls();
    let mut child = child_watchdog::Owned::spawn(&mut hook).unwrap();
    child
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&event).unwrap())
        .unwrap();
    let status = child.wait("container hook event").unwrap();
    let (stdout, stderr) = drain(&mut child);
    assert!(status.success(), "{source}: {stderr:?}");
    assert!(stderr.is_empty(), "{source}: {stderr:?}");
    assert_eq!(s.calls(), before, "the hook executed a build: {source}");
    if stdout.is_empty() {
        return None;
    }
    let response: Value = serde_json::from_slice(&stdout).unwrap();
    response["hookSpecificOutput"]["updatedInput"]["command"]
        .as_str()
        .map(str::to_owned)
}

fn drain(child: &mut child_watchdog::Owned) -> (Vec<u8>, Vec<u8>) {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    if let Some(mut pipe) = child.child.stdout.take() {
        pipe.read_to_end(&mut stdout).unwrap();
    }
    if let Some(mut pipe) = child.child.stderr.take() {
        pipe.read_to_end(&mut stderr).unwrap();
    }
    (stdout, stderr)
}

/// Start one command on a production route with its input already delivered.
pub(super) fn start(
    s: &Sandbox,
    f: &Fixture,
    route: Route,
    argv: &[&str],
    env: &[(&str, Option<&str>)],
) -> child_watchdog::Owned {
    let mut command = s.command(f, env);
    command
        .args([route.verb(), "--"])
        .args(argv)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = child_watchdog::Owned::spawn(&mut command).unwrap();
    child
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"input\n")
        .unwrap();
    child
}

/// The governed route's queue notice up to the waiting job's id: written once
/// by the waiting wrapper as `QUEUED`, `Waiting to start.`, `Reference: <id>`.
pub(super) const QUEUED: &str = "QUEUED\nWaiting to start.\nReference: ";

/// Read the waiting wrapper's notice from its stderr and return the reference
/// it names. Each read has the 60 s diagnostic watchdog; a wrapper that starts
/// its child instead writes other bytes or closes stderr, and fails here
/// without waiting.
pub(super) fn queued_notice(child: &mut child_watchdog::Owned, label: &str) -> String {
    let stderr = child.child.stderr.as_mut().unwrap();
    let (prefix, mut bytes) = (QUEUED.as_bytes(), Vec::new());
    loop {
        let reference = bytes.strip_prefix(prefix);
        if let Some(reference) = reference.and_then(|rest| rest.strip_suffix(b"\n")) {
            return String::from_utf8(reference.to_vec()).unwrap();
        }
        let mut ready = libc::pollfd {
            fd: stderr.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        assert_eq!(
            unsafe { libc::poll(&mut ready, 1, 60_000) },
            1,
            "{label}: no QUEUED notice before the watchdog"
        );
        let mut chunk = [0u8; 256];
        let read = stderr.read(&mut chunk).unwrap();
        bytes.extend_from_slice(&chunk[..read]);
        let shared = bytes.len().min(prefix.len());
        let reference = bytes[shared..]
            .strip_suffix(b"\n")
            .unwrap_or(&bytes[shared..]);
        assert!(
            read > 0
                && bytes[..shared] == prefix[..shared]
                && reference.iter().all(u8::is_ascii_hexdigit),
            "{label}: expected the QUEUED notice, read {:?}",
            String::from_utf8_lossy(&bytes)
        );
    }
}

/// Wait for the command and return its exit code, stdout and the stderr not
/// yet consumed by `queued_notice`.
pub(super) fn finish(
    child: &mut child_watchdog::Owned,
    label: &str,
) -> (Option<i32>, Vec<u8>, Vec<u8>) {
    let status = child.wait(label).unwrap();
    let (stdout, stderr) = drain(child);
    (status.code(), stdout, stderr)
}

/// The offline stub ran once as the same native child the user's command names.
pub(super) fn assert_native_child(s: &Sandbox, argv: &[&str], stdout: &[u8], label: &str) {
    let child: Value = serde_json::from_slice(stdout.strip_prefix(b"input\n").unwrap()).unwrap();
    let arguments: Vec<Vec<u8>> = argv[1..]
        .iter()
        .map(|arg| arg.as_bytes().to_vec())
        .collect();
    assert_eq!(child["args"], json!(arguments), "{label}");
    assert_eq!(
        child["cwd"],
        json!(fs::canonicalize(&s.repo).unwrap().as_os_str().as_bytes()),
        "{label}"
    );
    assert_eq!(
        child["environment"],
        json!(b"offline-fixture-value"),
        "{label}"
    );
}

/// Run one command to completion on a route and return its negotiated record.
pub(super) fn run(
    s: &Sandbox,
    f: &Fixture,
    route: Route,
    argv: &[&str],
    env: &[(&str, Option<&str>)],
) -> (Value, JobId) {
    let label = format!("{route:?} {argv:?}");
    let before = s.calls();
    let mut child = start(s, f, route, argv, env);
    let (code, stdout, stderr) = finish(&mut child, &label);
    assert_eq!(code, Some(17), "{label}: {stderr:?}");
    assert_eq!(stderr, b"fixture-stderr\n", "{label}");
    assert_native_child(s, argv, &stdout, &label);
    assert_eq!(s.calls(), before + 1, "{label}");
    let id = cli::recent(f);
    let job = job(f, arc_ipc::compute_location::OPERATION_JOB_STATUS, &id);
    assert_eq!(job["outcome"]["code"], 17, "{label}: {job}");
    assert_recorded_fixture_child(s, &job);
    (job, id)
}

/// One job snapshot as the named status reader receives it.
pub(super) fn job(f: &Fixture, operation: &str, id: &JobId) -> Value {
    let response = protocol::call(
        f,
        operation,
        json!(StatusRequest::ById { job_id: id.clone() }),
    );
    assert_eq!(response.status, "ok", "{operation}: {response:?}");
    response.result.unwrap()["job"].clone()
}

/// (queue entries, queued, reservations, active reservations) in the daemon's
/// database, optionally for one job.
pub(super) fn admission(f: &Fixture, id: Option<&JobId>) -> (i64, i64, i64, i64) {
    let db = rusqlite::Connection::open(&f.database).unwrap();
    let filter = id.map(JobId::as_str).unwrap_or("%");
    db.query_row(
        "SELECT (SELECT count(*) FROM traffic_entries WHERE job_id LIKE ?1), \
         (SELECT count(*) FROM traffic_entries WHERE job_id LIKE ?1 AND state='QUEUED'), \
         (SELECT count(*) FROM permits WHERE job_id LIKE ?1), \
         (SELECT count(*) FROM permits WHERE job_id LIKE ?1 AND state='ACTIVE')",
        [filter],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .unwrap()
}

/// The stored class and compute location of one job row.
pub(super) fn stored(f: &Fixture, id: &JobId) -> (String, String) {
    let db = rusqlite::Connection::open(&f.database).unwrap();
    db.query_row(
        "SELECT job_class, locality FROM jobs WHERE id=?1",
        [id.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .unwrap()
}
