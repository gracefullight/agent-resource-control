use super::{isolated, Signals};
use std::{
    os::unix::process::ExitStatusExt,
    process::Command,
    time::{Duration, Instant},
};

#[test]
fn sole_waiter_returns_native_usage_and_preserves_exit17_and_signal() {
    isolated("exec::signals::tests::accounting_tests::sole_waiter_returns_native_usage_and_preserves_exit17_and_signal", || {
        for script in ["exit 0", "exit 17", "kill -TERM $$"] {
            let signals = Signals::install(false).unwrap();
            let mut command = Command::new("/bin/sh");
            command.args(["-c", script]);
            signals.prepare_child(&mut command);
            let start = Instant::now();
            let mut child = command.spawn().unwrap();
            signals.attach(child.id());
            let terminal = signals.wait(&mut child).unwrap();
            assert!(terminal.observed_at >= start);
            assert!(terminal.observed_at <= Instant::now());
            let usage = terminal.usage.unwrap();
            assert!(usage.user_core_seconds >= 0.0);
            assert!(usage.system_core_seconds >= 0.0);
            assert_eq!(usage.total_core_seconds, usage.user_core_seconds + usage.system_core_seconds);
            match script {
                "exit 0" => assert_eq!(terminal.status.code(), Some(0)),
                "exit 17" => assert_eq!(terminal.status.code(), Some(17)),
                _ => assert_eq!(terminal.status.signal(), Some(libc::SIGTERM)),
            }
            assert_eq!(child.wait().unwrap_err().raw_os_error(), Some(libc::ECHILD));
        }
    });
}

#[test]
fn inclusive_native_usage_captures_reaped_cpu_descendants_and_excludes_peer() {
    isolated("exec::signals::tests::accounting_tests::inclusive_native_usage_captures_reaped_cpu_descendants_and_excludes_peer", || {
        use std::os::unix::process::CommandExt;
        let signals = Signals::install(true).unwrap();
        // A sibling in exactly the wrapper's process group burns CPU. The owned
        // job only sleeps, so a group aggregate would visibly charge the peer.
        let mut peer = Command::new("/bin/sh").args(["-c", "while :; do :; done"]).process_group(unsafe {libc::getpgrp()}).spawn().unwrap();
        let mut sleeper = Command::new("/bin/sleep").arg("0.15").spawn().unwrap();
        signals.attach(sleeper.id());
        let idle = signals.wait(&mut sleeper).unwrap();
        peer.kill().unwrap();
        peer.wait().unwrap();
        assert!(idle.usage.unwrap().total_core_seconds < 0.08);
        let mut owned = Command::new("/bin/sh").args(["-c", "(i=0; while [ $i -lt 60000 ]; do i=$((i+1)); done) & wait; exit 17"]).spawn().unwrap();
        signals.attach(owned.id());
        let terminal = signals.wait(&mut owned).unwrap();
        assert_eq!(terminal.status.code(), Some(17));
        assert!(terminal.usage.unwrap().total_core_seconds > idle.usage.unwrap().total_core_seconds + 0.03);
        assert!(terminal.observed_at.elapsed() < Duration::from_secs(1));
    });
}
