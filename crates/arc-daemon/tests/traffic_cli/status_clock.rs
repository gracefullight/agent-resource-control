//! Control only the production CLI process's monotonic clock in macOS tests.
use super::*;

pub(super) fn spawn(f: &Fixture, socket: &Path, clock: &Path) -> OwnedChild {
    let library = f.root.join("status-clock.dylib");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/traffic_cli/status_clock.c");
    let mut compiler = Command::new("/usr/bin/clang");
    compiler
        .env_clear()
        .args(["-dynamiclib", "-Wall", "-Wextra", "-Werror"])
        .arg(source)
        .arg("-o")
        .arg(&library)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut compiler = OwnedChild::new(compiler.spawn().unwrap());
    let output = compiler
        .wait_with_output()
        .expect("status clock compiler watchdog");
    assert!(output.status.success(), "clock compiler failed: {output:?}");

    let mut command = Command::new(binary("arcctl"));
    command
        .env_clear()
        .args(["job", "status"])
        .env(arc_core::ARC_SOCKET_PATH, socket)
        .env("DYLD_INSERT_LIBRARIES", library)
        .env("ARC_STATUS_TEST_CLOCK", clock)
        .env("ARC_STATUS_CLOCK_INITIAL", f.root.join("clock-initial"))
        .env("ARC_STATUS_CLOCK_ADVANCED", f.root.join("clock-advanced"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    forward_profile(&mut command);
    OwnedChild::new(command.spawn().unwrap())
}
