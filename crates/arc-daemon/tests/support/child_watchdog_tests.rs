use super::child_watchdog::{output, output_for, wait_child_for, Owned};
use std::{
    io,
    process::Command,
    thread,
    time::{Duration, Instant},
};

#[test]
fn stalled_owned_child_is_killed_and_reaped_by_the_watchdog() {
    let mut child = Owned::spawn(Command::new("/bin/sleep").arg("600")).unwrap();
    assert!(child.child.try_wait().unwrap().is_none());
    let error = child
        .wait_for("controlled stalled child", Duration::ZERO)
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(error.to_string().contains("controlled stalled child"));
    assert!(child.child.try_wait().unwrap().is_some());
}

#[test]
fn stalled_borrowed_child_is_reaped_before_the_wait_returns() {
    let mut child = Command::new("/bin/sleep").arg("600").spawn().unwrap();
    assert!(child.try_wait().unwrap().is_none());
    let error = wait_child_for(&mut child, "borrowed stalled child", Duration::ZERO).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(child.try_wait().unwrap().is_some());
}

#[test]
fn output_preserves_full_streams_and_the_native_exit() {
    let mut command = Command::new("python3");
    command.args([
        "-c",
        "import os,sys; os.write(1,b'o'*262144); os.write(2,b'e'*262144); sys.exit(17)",
    ]);
    let output = output(&mut command, "large output fixture").unwrap();
    assert_eq!(output.status.code(), Some(17));
    assert_eq!(output.stdout, vec![b'o'; 262144]);
    assert_eq!(output.stderr, vec![b'e'; 262144]);
}

#[test]
fn inherited_pipe_holder_is_terminated_after_the_output_watchdog() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "/bin/sleep 600 & printf 'holder:%s' \"$!\"; exit 0"]);
    let error = output_for(
        &mut command,
        "controlled inherited pipe",
        Duration::from_secs(2),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    let diagnostic = error.to_string();
    let pid: libc::pid_t = diagnostic
        .split("stdout=\"holder:")
        .nth(1)
        .expect("pipe holder PID in diagnostics")
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if unsafe { libc::kill(pid, 0) } == -1
            && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "pipe holder {pid} survived cleanup"
        );
        thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn guard_reaps_an_owned_child_when_the_caller_unwinds() {
    let mut child = Owned::spawn(Command::new("/bin/sleep").arg("600")).unwrap();
    assert!(child.child.try_wait().unwrap().is_none());
    let pid = child.child.id() as libc::pid_t;
    let result = std::panic::catch_unwind(move || {
        let _child = child;
        panic!("controlled unwind");
    });
    assert!(result.is_err());
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
}
