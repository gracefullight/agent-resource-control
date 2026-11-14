use super::{isolated, Signals};
use crate::native_fixture as native;
use std::{os::unix::process::ExitStatusExt, process::Command, time::Instant};

#[test]
fn sole_waiter_returns_native_usage_and_preserves_exit17_and_signal() {
    isolated("exec::signals::tests::accounting_tests::sole_waiter_returns_native_usage_and_preserves_exit17_and_signal", || {
        let epoch = Instant::now();
        for (sequence, script) in ["exit 0", "exit 17", "kill -TERM $$"].into_iter().enumerate() {
            let signals = Signals::install(false).unwrap();
            let mut command = Command::new("/bin/sh");
            command.args(["-c", script]);
            signals.prepare_child(&mut command);
            let mut child = native::OwnedChild::new(command.spawn().unwrap());
            signals.attach(child.id());
            let pid = child.id();
            let event_time = epoch + std::time::Duration::from_secs(sequence as u64 + 1);
            let samples = std::cell::Cell::new(0);
            let terminal = signals.wait_with_clock(&mut child,
                || assert_eq!(samples.get(), 0, "clock sampled before terminal event"),
                || {
                    // The real consuming wait must precede this timestamp. A
                    // pre-wait/stale sample fails even if it is <= wall now.
                    assert_eq!(unsafe { libc::waitpid(pid as i32, std::ptr::null_mut(), libc::WNOHANG) }, -1);
                    assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(libc::ECHILD));
                    samples.set(samples.get() + 1);
                    event_time
                }).unwrap();
            assert_eq!(samples.get(), 1);
            assert_eq!(terminal.observed_at, event_time);
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
        let mut peer = native::OwnedChild::new(Command::new("/bin/sh").args(["-c", "while :; do :; done"]).process_group(unsafe {libc::getpgrp()}).spawn().unwrap());
        let mut sleeper = native::OwnedChild::new(Command::new("/bin/sleep").arg("0.15").spawn().unwrap());
        signals.attach(sleeper.id());
        let idle = signals.wait(&mut sleeper).unwrap();
        peer.kill().unwrap();
        peer.wait().unwrap();
        assert!(idle.usage.unwrap().total_core_seconds < 0.08);
        let mut owned = native::OwnedChild::new(Command::new("/bin/sh").args(["-c", "(i=0; while [ $i -lt 60000 ]; do i=$((i+1)); done) & wait; exit 17"]).spawn().unwrap());
        signals.attach(owned.id());
        let terminal = signals.wait(&mut owned).unwrap();
        assert_eq!(terminal.status.code(), Some(17));
        assert!(terminal.usage.unwrap().total_core_seconds > idle.usage.unwrap().total_core_seconds + 0.03);
        // Native CPU totals and peer exclusion above are the accuracy checks;
        // timestamp attribution/order is proved by the injected event clock.
    });
}
