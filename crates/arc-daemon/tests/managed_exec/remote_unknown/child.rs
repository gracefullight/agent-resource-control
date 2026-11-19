//! Execution fixture for comparing direct and managed custom shell programs.
use super::super::{cli::command, retries::unknown_owned::SessionChild, runtime::Fixture};
use super::{reservations, ReservationSnapshot};
use arc_ipc::jobs::ProcessIdentity;
use std::{
    ffi::OsString,
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

pub(super) const LLVM_MARKER_KEY: &str = "__LLVM_PROFILE_RT_INIT_ONCE";
pub(super) const CALLER_LLVM_MARKER: &str = "caller-supplied-value";

pub(super) fn install_script(path: &Path) {
    fs::write(
        path,
        r##"#!/bin/sh
printf 'invoked\n' >> "$MARKER"
printf '%s\n' "$$" > "$PIDFILE"
IFS= read -r gate || exit 126
if [ -t 0 ] || [ -t 1 ] || [ -t 2 ]; then
  printf 'unexpected terminal\n' >&2; exit 125
fi
if [ "${MIXED:-}" = 1 ]; then cargo test && gh workflow run build.yml; fi
printf 'program=<%s>\n' "$0"
printf 'argc=%s\n' "$#"
for arg do printf '<%s>\n' "$arg"; done
printf 'cwd='; /bin/pwd -P
printf 'environment:\n'; /usr/bin/env | /usr/bin/sort
printf 'stdin='; /bin/cat
printf 'child-stderr\n' >&2
exit 37
"##,
    )
    .unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

pub(super) struct Scenario<'a> {
    pub(super) name: &'a str,
    pub(super) executable: &'a Path,
    pub(super) args: &'a [OsString],
    pub(super) socket: &'a Path,
    pub(super) kubecfg: Option<&'a str>,
    pub(super) expected_reason: Option<&'a str>,
    pub(super) mixed: bool,
    pub(super) caller_llvm_marker: bool,
}

pub(super) fn execute(
    f: &Fixture,
    scenario: &Scenario<'_>,
    marker: &Path,
    managed: bool,
    held: Option<&ReservationSnapshot>,
) -> (Output, ProcessIdentity, ProcessIdentity) {
    let pidfile = f.root.join("custom-child-pid");
    let mut invocation = if managed {
        let mut command = command(f);
        command.args(["exec", "--"]).arg(scenario.executable);
        command
    } else {
        Command::new(scenario.executable)
    };
    invocation
        .args(scenario.args)
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", f.root.display()))
        .env(arc_core::ARC_SOCKET_PATH, scenario.socket)
        .env("MARKER", marker)
        .env("PIDFILE", &pidfile)
        .env("STAGES", f.root.join("mixed-stages"))
        .env("FIXTURE_ENV", "spaces 한글 ; $(literal)")
        .current_dir(&f.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(value) = scenario.kubecfg {
        invocation.env("KUBECONFIG", value);
    }
    if scenario.mixed {
        invocation.env("MIXED", "1");
    }
    // The instrumented test process has this runtime marker in its own
    // environment. Pass the same input to both children after env_clear, so
    // their complete output can still be compared without normalization.
    if let Some(value) = std::env::var_os(LLVM_MARKER_KEY) {
        invocation.env(LLVM_MARKER_KEY, value);
    }
    if scenario.caller_llvm_marker {
        invocation.env(LLVM_MARKER_KEY, CALLER_LLVM_MARKER);
    }
    let mut child = SessionChild::spawn(invocation, Some(&pidfile)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let target_pid = loop {
        if let Some(pid) = fs::read_to_string(&pidfile)
            .ok()
            .and_then(|value| value.strip_suffix('\n')?.parse::<u32>().ok())
        {
            break pid;
        }
        assert!(
            !child.exited().unwrap(),
            "custom program exited before publishing its identity"
        );
        assert!(
            Instant::now() < deadline,
            "custom program did not reach its input barrier"
        );
        thread::sleep(Duration::from_millis(1));
    };
    let target = ProcessIdentity::capture(target_pid).unwrap();
    let observer = ProcessIdentity::capture(child.id()).unwrap();
    assert_eq!(
        unsafe { libc::getsid(target_pid as i32) },
        child.id() as i32
    );
    assert_eq!(
        unsafe { libc::getpgid(target_pid as i32) },
        target_pid as i32
    );
    if managed {
        assert_ne!(
            target_pid,
            child.id(),
            "ARC must use a separate child group"
        );
        child.track_target(target.clone());
    }
    if let Some(held) = held {
        assert_eq!(
            &reservations(f),
            held,
            "Traffic changed while the child was running"
        );
    }
    child
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"release\ninput\0bytes\n")
        .unwrap();
    let status = child
        .wait()
        .expect("custom shell program exceeded its 60 second watchdog");
    let output = child.output(status);
    fs::remove_file(pidfile).unwrap();
    (output, observer, target)
}
