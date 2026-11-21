//! Stage fixtures shared by the installed-hook and `arcctl exec` routes
//! (AT-TRF-12, DET-016). Everything is offline: a private daemon, the built
//! `arcctl`, and stub programs that record their own starts in files.
use super::super::super::{
    child_watchdog, cli, instrumentation, protocol, write, Fixture, Sandbox,
};
use arc_ipc::jobs::{JobId, StatusRequest, OPERATION_JOB_STATUS_LOCALITY};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    os::unix::{
        fs::{symlink, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

type OwnedChild = super::super::super::super::runtime::OwnedChild;

pub(super) const BUN: &str = "bun test && gh workflow run ci.yml";
pub(super) const MAKE: &str = "make test && gh workflow run ci.yml";
/// The governed route's queue notice up to the waiting job's id.
const QUEUED: &str = "QUEUED\nWaiting to start.\nReference: ";
const STARTED: &str = "STARTED\nCommand started.\n";

/// The path a user's compound takes: the installed hook's rewrite run by the
/// vendor shell, or `arcctl exec -- bash -c <source>`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Route {
    Hook(&'static str),
    Exec,
}

/// A proven remote CI endpoint, the literal Makefile recipe `cargo test`, the
/// real `bash` and `make`, and offline stubs for `bun`, `cargo` and `gh`.
pub(super) fn sandbox(f: &Fixture) -> Sandbox {
    let s = Sandbox::new(f);
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    write(&s.repo.join("Makefile"), "test:\n\tcargo test\n");
    symlink("/bin/bash", s.bin.join("bash")).unwrap();
    symlink("/usr/bin/make", s.bin.join("make")).unwrap();
    for name in ["bun", "cargo"] {
        symlink(cli::binary("arc-exec-fixture"), s.bin.join(name)).unwrap();
    }
    s
}

/// The production daemon with a four-core Traffic budget, its
/// `ARC_TRAFFIC_CPU_BUDGET_MILLIS` setting. With the default two cores one
/// Build reservation is the whole budget; with four, a held Build permit and
/// a Test fit together, so only the TEST window can make the Test stage wait.
pub(super) fn daemon(f: &Fixture) -> OwnedChild {
    let mut command = f.isolated_command("test");
    command.env("ARC_TRAFFIC_CPU_BUDGET_MILLIS", "4000");
    let mut child = OwnedChild::new(command.spawn().unwrap());
    f.wait_for(&mut child, "arc-daemon ready.");
    child
}

/// Poll daemon-recorded state under the diagnostic watchdog. Correctness comes
/// from the observed state, never from how long it took to appear.
pub(super) fn eventually<T>(label: &str, mut check: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(value) = check() {
            return value;
        }
        assert!(Instant::now() < deadline, "{label}: exceeded 60 s watchdog");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// One job row with its queue entry and reservation, in registration order.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Row {
    pub(super) id: String,
    pub(super) class: String,
    pub(super) locality: String,
    pub(super) entry: Option<String>,
    pub(super) permit: Option<String>,
    pub(super) scheduled: bool,
}

pub(super) fn rows(f: &Fixture) -> Vec<Row> {
    if !f.database.exists() {
        // No daemon has run in this fixture, so nothing was registered.
        return Vec::new();
    }
    let db = rusqlite::Connection::open(&f.database).unwrap();
    let mut statement = db
        .prepare(
            "SELECT j.id, j.job_class, j.locality, \
             (SELECT state FROM traffic_entries t WHERE t.job_id=j.id), \
             (SELECT state FROM permits p WHERE p.job_id=j.id), \
             j.queue_at IS NOT NULL OR j.reserved_at IS NOT NULL \
             FROM jobs j ORDER BY j.rowid",
        )
        .unwrap();
    let rows = statement
        .query_map([], |row| {
            Ok(Row {
                id: row.get(0)?,
                class: row.get(1)?,
                locality: row.get(2)?,
                entry: row.get(3)?,
                permit: row.get(4)?,
                scheduled: row.get(5)?,
            })
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    rows
}

pub(super) fn row(f: &Fixture, id: &str) -> Row {
    rows(f).into_iter().find(|row| row.id == id).unwrap()
}

/// The stored record through the daemon's locality status operation.
pub(super) fn status(f: &Fixture, id: &str) -> Value {
    let job_id: JobId = serde_json::from_value(json!(id)).unwrap();
    let response = protocol::call(
        f,
        OPERATION_JOB_STATUS_LOCALITY,
        json!(StatusRequest::ById { job_id }),
    );
    assert_eq!(response.status, "ok", "{response:?}");
    response.result.unwrap()["job"].clone()
}

/// The stage records linked to one request, in link order (contract D3).
pub(super) fn linked(f: &Fixture, request: &str) -> Vec<String> {
    let response = protocol::call(
        f,
        arc_ipc::request::OPERATION,
        json!({"action":"STAGES","request_id":request}),
    );
    assert_eq!(response.status, "ok", "{response:?}");
    let result = response.result.unwrap();
    assert_eq!(result["request_id"], request);
    serde_json::from_value(result["jobs"].clone()).unwrap()
}

/// `arcctl job status [ID]` as a user reads it.
pub(super) fn job_status(s: &Sandbox, f: &Fixture, id: Option<&str>) -> String {
    let mut command = s.command(f, &[]);
    command.args(["job", "status"]).args(id);
    let output = child_watchdog::output(&mut command, "arcctl job status").unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

pub(super) fn output_with_input(command: &mut Command, input: &[u8], label: &str) -> Output {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = child_watchdog::Owned::spawn(command).unwrap();
    child.child.stdin.take().unwrap().write_all(input).unwrap();
    let status = child.wait(label).unwrap();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let mut out = child.child.stdout.take().unwrap();
    let mut err = child.child.stderr.take().unwrap();
    out.read_to_end(&mut stdout).unwrap();
    err.read_to_end(&mut stderr).unwrap();
    Output {
        status,
        stdout,
        stderr,
    }
}

/// A shell started directly, with only the sandbox's cleared environment.
fn shell(program: &Path, s: &Sandbox, f: &Fixture, source: &str, vendor: bool) -> Command {
    let template = s.command(f, &[]);
    let mut command = Command::new(program);
    command
        .env_clear()
        .args(["-c", source])
        .current_dir(&s.repo);
    if vendor {
        command
            .env("ZDOTDIR", &s.home)
            .env("LOGNAME", "arc-fixture")
            .env("USER", "arc-fixture");
    }
    for (key, value) in template.get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        }
    }
    command
}

/// The installed Claude hook's rewrite of one Bash command, or `None` when the
/// hook leaves the command to the vendor unchanged. The hook starts nothing.
pub(super) fn installed_hook_rewrite(s: &Sandbox, f: &Fixture, source: &str) -> Option<String> {
    let install = child_watchdog::output(
        s.command(f, &[])
            .args(["hooks", "install", "claude", "--scope", "user"]),
        "install hook for stages",
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
    let (before, jobs) = (s.calls(), rows(f).len());
    let response = output_with_input(
        &mut shell(Path::new("/bin/sh"), s, f, installed, true),
        &serde_json::to_vec(&event).unwrap(),
        "stage hook event",
    );
    assert!(response.status.success(), "{source}: {response:?}");
    assert!(response.stderr.is_empty(), "{source}: {response:?}");
    assert_eq!(s.calls(), before, "the hook executed a stage: {source}");
    assert_eq!(rows(f).len(), jobs, "the hook registered a job: {source}");
    if response.stdout.is_empty() {
        return None;
    }
    let response: Value = serde_json::from_slice(&response.stdout).unwrap();
    let updated = &response["hookSpecificOutput"]["updatedInput"];
    let mut restored = updated.clone();
    restored["command"] = json!(source);
    assert_eq!(restored, event["tool_input"], "only the command may change");
    updated["command"].as_str().map(str::to_owned)
}

/// The user's compound on its production route. On the hook route the request
/// id is the one the hook wrote into both stages of its rewrite.
pub(super) fn managed(
    route: Route,
    s: &Sandbox,
    f: &Fixture,
    source: &str,
) -> (Command, Option<String>) {
    match route {
        Route::Hook(vendor) => {
            let rewritten = installed_hook_rewrite(s, f, source).expect("separable compound");
            let arcctl = cli::binary("arcctl");
            let [test, remote] = source.split(" && ").collect::<Vec<_>>()[..] else {
                panic!("{source}")
            };
            let prefix = format!("'{}' hook-exec --arc-request ", arcctl.display());
            let request = rewritten[prefix.len()..prefix.len() + 32].to_owned();
            assert_eq!(
                rewritten,
                format!("{prefix}{request} -- {test} && {prefix}{request} -- {remote}")
            );
            (
                shell(Path::new(vendor), s, f, &rewritten, true),
                Some(request),
            )
        }
        Route::Exec => {
            let mut command = s.command(f, &[]);
            command.args(["exec", "--", "bash", "-c", source]);
            (command, None)
        }
    }
}

/// The same source run by the same shell with no ARC process in the path.
pub(super) fn native(route: Route, s: &Sandbox, f: &Fixture, source: &str) -> Command {
    match route {
        Route::Hook(vendor) => shell(Path::new(vendor), s, f, source, true),
        Route::Exec => shell(&s.bin.join("bash"), s, f, source, false),
    }
}

/// A stub barrier: every stub start appends its pid to `marker` and then
/// reports its identity and argv on the control socket until released.
pub(super) struct Barrier {
    listener: UnixListener,
    control: PathBuf,
    pub(super) marker: PathBuf,
}

impl Barrier {
    pub(super) fn new(f: &Fixture, name: &str) -> Self {
        let control = f.root.join(format!("{name}-ctl"));
        Self {
            listener: UnixListener::bind(&control).unwrap(),
            control,
            marker: f.root.join(format!("{name}-starts")),
        }
    }

    pub(super) fn install(&self, command: &mut Command) {
        command
            .env("ARC_FIXTURE_CONTROL", &self.control)
            .env("ARC_FIXTURE_MARKER", &self.marker);
    }

    /// Stub starts in order, read from the file the stubs wrote.
    pub(super) fn starts(&self) -> Vec<u64> {
        fs::read_to_string(&self.marker)
            .map(|text| text.lines().map(|line| line.parse().unwrap()).collect())
            .unwrap_or_default()
    }

    pub(super) fn accept(&self) -> (UnixStream, Value) {
        cli::accept(&self.listener)
    }
}

/// A separate real CLI that owns one local permit until released.
pub(super) struct Hold {
    release: UnixStream,
    owner: OwnedChild,
    pub(super) id: String,
}

pub(super) fn hold(s: &Sandbox, f: &Fixture, action: &str) -> Hold {
    let barrier = Barrier::new(f, &format!("held-{action}"));
    let before = rows(f).len();
    let mut command = s.command(f, &[]);
    barrier.install(&mut command);
    command
        .args(["exec", "--", "cargo", action])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut owner =
        OwnedChild::spawn_with_descendants(command, f.root.join(format!("held-{action}-owners")))
            .unwrap();
    let (release, child) = barrier.accept();
    owner.own_process(serde_json::from_value(child["identity"].clone()).unwrap());
    let held = rows(f);
    assert_eq!(held.len(), before + 1, "{held:?}");
    let id = held.last().unwrap().id.clone();
    running(f, &id);
    Hold { release, owner, id }
}

/// A started stub's job holds an ACTIVE permit; its wrapper then records the
/// attached child, which moves the queue entry from RESERVED to RUNNING.
pub(super) fn running(f: &Fixture, id: &str) -> Row {
    assert_eq!(row(f, id).permit.as_deref(), Some("ACTIVE"));
    eventually("attached child", || {
        Some(row(f, id)).filter(|row| row.entry.as_deref() == Some("RUNNING"))
    })
}

impl Hold {
    pub(super) fn release(mut self, f: &Fixture) {
        self.release.write_all(b"x").unwrap();
        let output = self.owner.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(17), "{output:?}");
        assert_eq!(row(f, &self.id).permit.as_deref(), Some("RELEASED"));
    }
}

/// ARC's own notices on a stage's stderr, removed for the child-byte
/// comparison (.arc-verify/traffic/ux-contract.md): each QUEUED notice with
/// its `Reference:` line, and each STARTED notice. Returns the child bytes,
/// the number of QUEUED notices and the number of STARTED notices.
pub(super) fn without_notices(stderr: &[u8]) -> (String, usize, usize) {
    let text = String::from_utf8(stderr.to_vec()).unwrap();
    let started = text.matches(STARTED).count();
    let (mut child, mut queued, mut rest) = (String::new(), 0, text.as_str());
    while let Some((before, notice)) = rest.split_once(QUEUED) {
        let (reference, after) = notice.split_once('\n').expect("unterminated reference");
        assert!(
            reference.len() == 32 && reference.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "not a job reference: {reference:?}"
        );
        child.push_str(before);
        queued += 1;
        rest = after;
    }
    child.push_str(rest);
    (child.replace(STARTED, ""), queued, started)
}

/// The waiting stage's notice: exactly the QUEUED text, written once, whose
/// `Reference:` line is the waiting stage's own job id.
pub(super) fn assert_waiting_notice(stderr: &str, stage: &str) {
    assert_eq!(stderr, format!("{QUEUED}{stage}\n"));
}

/// Replace one stub with a script that records nothing and exits 0, so only
/// the other stage's stub observes its own process.
pub(super) fn quiet_stub(s: &Sandbox, name: &str) {
    let path = s.bin.join(name);
    fs::remove_file(&path).unwrap();
    write(&path, "#!/bin/sh\nexit 0\n");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// What one capturing stub observed of its own process.
#[derive(Debug, PartialEq)]
pub(super) struct Observed {
    pub(super) args: Value,
    pub(super) cwd: Value,
    pub(super) argv0: Value,
    pub(super) stdin: Value,
    pub(super) environment: BTreeMap<String, String>,
    pub(super) starts: usize,
    pub(super) code: Option<i32>,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
}

/// Run a compound whose first capturing stub records its arguments, working
/// directory, environment and stdin and exits 17. The environment is compared
/// through the shared instrumentation helper.
pub(super) fn observe(f: &Fixture, mut command: Command, label: &str) -> (Observed, Option<u64>) {
    let key = f.root.join("capture-key");
    let (marker, record) = (f.root.join("capture-starts"), f.root.join("capture.json"));
    write(&key, b"arc-hook-fixture-capture-v1");
    command
        .env("ARC_FIXTURE_MODE", "hook-capture-pid")
        .env("ARC_HOOK_CAPTURE_KEY", &key)
        .env("ARC_HOOK_CAPTURE_MARKER", &marker)
        .env("ARC_HOOK_CAPTURE_RECORD", &record);
    let output = output_with_input(&mut command, b"input\n", label);
    let captured: Value = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
    let starts = fs::read_to_string(&marker).unwrap();
    for path in [&marker, &record] {
        fs::remove_file(path).unwrap();
    }
    let mut environment = BTreeMap::new();
    for entry in captured["environment"].as_array().unwrap() {
        let text = |value: &Value| -> String {
            String::from_utf8(serde_json::from_value::<Vec<u8>>(value.clone()).unwrap()).unwrap()
        };
        environment.insert(text(&entry[0]), text(&entry[1]));
    }
    let mut instrumented = BTreeMap::new();
    instrumentation::include_expected(&mut instrumented);
    instrumentation::normalize_actual(&mut environment, &instrumented);
    let (stderr, queued, started) = without_notices(&output.stderr);
    assert_eq!((queued, started), (0, 0), "{label}: nothing was held");
    (
        Observed {
            args: captured["args"].clone(),
            cwd: captured["cwd"].clone(),
            argv0: captured["argv0"].clone(),
            stdin: captured["stdin"].clone(),
            environment,
            starts: starts.lines().count(),
            code: output.status.code(),
            stdout: output.stdout,
            stderr: stderr.into_bytes(),
        },
        captured["pid"].as_u64(),
    )
}
