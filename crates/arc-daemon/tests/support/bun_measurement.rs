//! Process witnesses for the mixed-tool measurement fixture.
use super::*;
use std::os::unix::{ffi::OsStrExt, fs::PermissionsExt};

// The PID is unchanged across exec, so ARC observes the native tool itself.
const SHIM: &str = r##"#!/bin/sh
printf '%s\n' "$$" >> "$MIXED_LAUNCH_LOG" || exit 125
if [ -n "${MIXED_FIRST_JOB:-}" ]; then
  case "$MIXED_FIRST_JOB" in *[!0123456789abcdef]*|'') exit 125;; esac
  case "$MIXED_FIRST_TEST_PID" in *[!0123456789]*|'') exit 125;; esac
  [ "${#MIXED_FIRST_JOB}" -eq 32 ] || exit 125
  if kill -0 "$MIXED_FIRST_TEST_PID" 2>/dev/null; then exit 125; fi
  observer=$PPID
  state=$(/usr/bin/sqlite3 -readonly -cmd '.timeout 0' -cmd ".parameter set :first \"'$MIXED_FIRST_JOB'\"" -cmd ".parameter set :observer $observer" "$MIXED_DATABASE" "SELECT p.job_id || ' ' || p.state || ' ' || p.released_at || ' ' || t.job_id || ' ' || q.state || ' ' || t.granted_at_ms || ' ' || t.queued_at_ms FROM permits p JOIN traffic_entries t JOIN job_lifecycle l ON l.job_id=t.job_id JOIN permits q ON q.job_id=t.job_id WHERE p.job_id=:first AND l.observer_pid=:observer AND p.state='RELEASED' AND p.released_at IS NOT NULL AND q.state='ACTIVE' AND t.granted_at_ms >= p.released_at;") || exit 125
  [ -n "$state" ] || exit 125
  printf '%s %s %s\n' "$$" "$MIXED_FIRST_TEST_PID" "$state" > "$MIXED_RELEASE_WITNESS" || exit 125
fi
exec "$MIXED_REAL_TOOL" "$@"
"##;

pub(super) fn executable(pid: u32) -> PathBuf {
    let mut path = [0u8; 4096];
    // SAFETY: the initialized buffer is valid for the size supplied to proc_pidpath.
    let size =
        unsafe { libc::proc_pidpath(pid as i32, path.as_mut_ptr().cast(), path.len() as u32) };
    assert!(size > 0, "cannot read executable for {pid}");
    fs::canonicalize(std::ffi::OsStr::from_bytes(&path[..size as usize])).unwrap()
}

pub(super) fn identity(pid: u32) -> ProcessIdentity {
    let identity = ProcessIdentity::capture(pid).unwrap();
    assert_eq!(probe_live(&identity), IdentityStatus::VerifiedMatch);
    identity
}

fn marker_pid(path: &Path) -> u32 {
    let data = fs::read_to_string(path).unwrap();
    let lines: Vec<_> = data.lines().collect();
    assert_eq!(lines.len(), 1, "missing or duplicate invocation: {path:?}");
    lines[0].parse().unwrap()
}

fn marker_ready(path: &Path) -> bool {
    fs::read_to_string(path).is_ok_and(|text| !text.is_empty() && text.ends_with('\n'))
}

#[test]
fn incomplete_marker_is_not_observed_as_an_invocation() {
    let fixture = Fixture::new();
    let marker = fixture.root.join("marker");
    fs::write(&marker, "").unwrap();
    assert!(!marker_ready(&marker));
    fs::write(&marker, "1234").unwrap();
    assert!(!marker_ready(&marker));
    fs::write(&marker, "1234\n").unwrap();
    assert!(marker_ready(&marker));
    assert_eq!(marker_pid(&marker), 1234);
}

#[test]
fn shim_rejects_early_launch_without_waiting_for_a_permit() {
    let fixture = Fixture::new();
    shim(&fixture.root, "cargo");
    let db_path = fixture.root.join("witness.db");
    let db = rusqlite::Connection::open(&db_path).unwrap();
    db.execute_batch("CREATE TABLE permits(job_id TEXT, state TEXT, released_at INTEGER); CREATE TABLE traffic_entries(job_id TEXT, granted_at_ms INTEGER, queued_at_ms INTEGER); CREATE TABLE job_lifecycle(job_id TEXT, observer_pid INTEGER);").unwrap();
    let first = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let second = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    db.execute("INSERT INTO permits VALUES (?1,'ACTIVE',NULL)", [first])
        .unwrap();
    db.execute("INSERT INTO permits VALUES (?1,'ACTIVE',NULL)", [second])
        .unwrap();
    db.execute("INSERT INTO traffic_entries VALUES (?1,101,90)", [second])
        .unwrap();
    db.execute(
        "INSERT INTO job_lifecycle VALUES (?1,?2)",
        rusqlite::params![second, std::process::id()],
    )
    .unwrap();
    let mut gone = Command::new("/usr/bin/true").spawn().unwrap();
    let gone_pid = gone.id();
    assert!(gone.wait().unwrap().success());
    let tool = fixture.root.join("real-tool");
    fs::write(&tool, "#!/bin/sh\nprintf executed > \"$ACTUAL_EXEC\"\n").unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
    let run = |name: &str, pid: u32| {
        let path = fixture.root.join(name);
        fs::create_dir(&path).unwrap();
        let status = Command::new(fixture.root.join("cargo"))
            .env("MIXED_LAUNCH_LOG", path.join("launches"))
            .env("MIXED_REAL_TOOL", &tool)
            .env("MIXED_FIRST_JOB", first)
            .env("MIXED_FIRST_TEST_PID", pid.to_string())
            .env("MIXED_DATABASE", &db_path)
            .env("MIXED_RELEASE_WITNESS", path.join("committed-release"))
            .env("ACTUAL_EXEC", path.join("executed"))
            .status()
            .unwrap();
        (path, status)
    };
    let (live, status) = run("first-still-running", std::process::id());
    assert_eq!(status.code(), Some(125));
    assert!(live.join("launches").exists());
    assert!(!live.join("executed").exists());
    let (early, status) = run("permit-still-active", gone_pid);
    assert_eq!(status.code(), Some(125));
    assert!(early.join("launches").exists());
    assert!(!early.join("committed-release").exists());
    assert!(!early.join("executed").exists());
    db.execute(
        "UPDATE permits SET state='RELEASED', released_at=100 WHERE job_id=?1",
        [first],
    )
    .unwrap();
    let (ordered, status) = run("permit-released", gone_pid);
    assert!(status.success());
    assert!(ordered.join("executed").exists());
    let witness = fs::read_to_string(ordered.join("committed-release")).unwrap();
    assert!(witness.contains(&format!(
        "{gone_pid} {first} RELEASED 100 {second} ACTIVE 101 90"
    )));
}

pub(super) fn shim(root: &Path, tool: &str) {
    let path = root.join(tool);
    fs::write(&path, SHIM).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

pub(super) fn launch(
    f: &Fixture,
    path: &Path,
    tool: usize,
    managed: bool,
    real: &Path,
    shim_root: &Path,
    first_job: Option<(&str, u32)>,
) -> BuildChild {
    let mut command = if managed {
        let mut cmd = Command::new(binary("arcctl"));
        cmd.args(["exec", "--", TOOLS[tool]])
            .env(arc_core::ARC_SOCKET_PATH, &f.socket);
        cmd
    } else {
        Command::new(TOOLS[tool])
    };
    if tool == 0 {
        command.args(["test", "--offline", "--", "--nocapture"]);
    } else {
        command.arg("test");
    }
    command
        .current_dir(path)
        .env(
            "PATH",
            format!("{}:{}", shim_root.display(), std::env::var("PATH").unwrap()),
        )
        .env("MIXED_REAL_TOOL", real)
        .env("MIXED_LAUNCH_LOG", path.join("launches"))
        .env("MIXED_BODY_LOG", path.join("bodies"))
        .env("MIXED_RELEASE", path.join("release"))
        .env_remove("MIXED_FIRST_JOB")
        .env_remove("MIXED_FIRST_TEST_PID")
        .env_remove("MIXED_DATABASE")
        .env_remove("MIXED_RELEASE_WITNESS")
        .env("CARGO_TARGET_DIR", path.join("target"))
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .stdin(Stdio::null())
        .stdout(Stdio::from(File::create(path.join("stdout.log")).unwrap()))
        .stderr(Stdio::from(File::create(path.join("stderr.log")).unwrap()));
    if let Some((job, test_pid)) = first_job {
        assert!(managed);
        command
            .env("MIXED_FIRST_JOB", job)
            .env("MIXED_FIRST_TEST_PID", test_pid.to_string())
            .env("MIXED_DATABASE", &f.database)
            .env("MIXED_RELEASE_WITNESS", path.join("committed-release"));
    }
    BuildChild::new(OwnedChild::new(command.spawn().unwrap()), f, managed)
}

pub(super) fn observe_body(child: &BuildChild, path: &Path, tool: usize, managed: bool) -> Value {
    eventually(|| marker_ready(&path.join("launches")) && marker_ready(&path.join("bodies")));
    let tool_pid = marker_pid(&path.join("launches"));
    let body_pid = marker_pid(&path.join("bodies"));
    let tool_id = identity(tool_pid);
    let body_id = identity(body_pid);
    let tool_path = executable(tool_pid);
    let body_path = executable(body_pid);
    let wrapper_path = executable(child.child.id());
    assert_eq!(tool_path.file_name().unwrap(), TOOLS[tool]);
    if managed {
        assert!(children(child.child.id()).contains(&tool_pid));
        assert_eq!(wrapper_path, fs::canonicalize(binary("arcctl")).unwrap());
    } else {
        assert_eq!(child.child.id(), tool_pid);
        assert_eq!(wrapper_path, tool_path);
    }
    if tool == 0 {
        assert_ne!(tool_pid, body_pid);
        assert!(children(tool_pid).contains(&body_pid));
    } else {
        assert_eq!(tool_pid, body_pid);
    }
    json!({"wrapper":child.owner.as_ref().unwrap(),
        "tool":tool_id, "test":body_id,
        "toolExecutable":{"path":tool_path,"sha256":sha256(&tool_path)},
        "testExecutable":{"path":body_path,"sha256":sha256(&body_path)},
        "wrapperExecutable":{"path":wrapper_path,"sha256":sha256(&wrapper_path)},
        "parentVerified":true})
}
