//! Isolated native processes avoid changing the parallel libtest host's signals.
use super::{Mask, Signals, CHILD, NONTERMINAL, PENDING, TERMINAL};
use std::sync::atomic::{AtomicI32, Ordering};
use std::{
    os::fd::RawFd,
    process::Command,
    time::{Duration, Instant},
};
#[path = "diagnostic_tests.rs"]
mod diagnostic_tests;
#[path = "handoff.rs"]
mod handoff;
static EVENTS: AtomicI32 = AtomicI32::new(-1);
extern "C" fn event(signal: i32) {
    unsafe {
        libc::write(
            EVENTS.load(Ordering::SeqCst),
            (&signal as *const i32).cast(),
            4,
        )
    };
}
fn pipe() -> [RawFd; 2] {
    let mut fds = [-1; 2];
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    fds
}
fn read(fd: RawFd) -> i32 {
    let mut poll = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    assert_eq!(
        unsafe { libc::poll(&mut poll, 1, 5000) },
        1,
        "native barrier timeout"
    );
    let mut value = 0i32;
    assert_eq!(
        unsafe { libc::read(fd, (&mut value as *mut i32).cast(), 4) },
        4
    );
    value
}
fn write(fd: RawFd, value: i32) {
    assert_eq!(
        unsafe { libc::write(fd, (&value as *const i32).cast(), 4) },
        4
    );
}
#[test]
fn native_receiver() {
    let Ok(fd) = std::env::var("ARC_SIGNAL_EVENTS") else {
        return;
    };
    EVENTS.store(fd.parse().unwrap(), Ordering::SeqCst);
    if std::env::var_os("ARC_EXPECT_CHLD_BLOCKED").is_some() {
        let mut mask = unsafe { std::mem::zeroed() };
        unsafe { libc::sigprocmask(libc::SIG_SETMASK, std::ptr::null(), &mut mask) };
        assert_eq!(unsafe { libc::sigismember(&mask, libc::SIGCHLD) }, 1);
    }
    if std::env::var_os("ARC_EXPECT_CHLD_IGNORED").is_some() {
        assert_eq!(sigchld_action().sa_sigaction, libc::SIG_IGN);
    }
    if let Ok(ignored) = std::env::var("ARC_EXPECT_CONT_IGNORED") {
        assert_eq!(
            signal_action(libc::SIGCONT).sa_sigaction == libc::SIG_IGN,
            ignored == "true"
        );
        let mut mask = unsafe { std::mem::zeroed() };
        unsafe { libc::sigprocmask(libc::SIG_SETMASK, std::ptr::null(), &mut mask) };
        assert_eq!(unsafe { libc::sigismember(&mask, libc::SIGCONT) }, 1);
    }
    for signal in NONTERMINAL.into_iter().chain(TERMINAL) {
        // SIGUSR2 deliberately remains inherited SIG_IGN in the ignored case.
        if signal == libc::SIGUSR2 && std::env::var_os("ARC_SIGNAL_IGNORED").is_some() {
            continue;
        }
        unsafe { libc::signal(signal, event as *const () as usize) };
    }
    write(
        std::env::var("ARC_SIGNAL_READY").unwrap().parse().unwrap(),
        1,
    );
    let gate = std::env::var("ARC_SIGNAL_GATE").unwrap().parse().unwrap();
    assert_eq!(read(gate), 17);
    unsafe { libc::_exit(17) };
}
fn sigchld_action() -> libc::sigaction {
    signal_action(libc::SIGCHLD)
}
fn signal_action(signal: i32) -> libc::sigaction {
    let mut action = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::sigaction(signal, std::ptr::null(), &mut action) },
        0
    );
    action
}
fn isolated(name: &str, body: impl FnOnce()) {
    isolated_sigchld(name, false, body);
}
fn isolated_sigchld(name: &str, ignored: bool, body: impl FnOnce()) {
    if std::env::var("ARC_SIGNAL_CASE").as_deref() == Ok(name) {
        body();
        println!("ARC_SIGNAL_EXECUTED={name}");
        return;
    }
    let mut command = Command::new(std::env::current_exe().unwrap());
    // Block before exec so every libtest thread inherits the same mask; only
    // the guarded waiter may unblock its process-wide child notification.
    use std::os::unix::process::CommandExt;
    unsafe {
        command.pre_exec(move || {
            if ignored && libc::signal(libc::SIGCHLD, libc::SIG_IGN) == libc::SIG_ERR {
                return Err(std::io::Error::last_os_error());
            }
            let mut set = std::mem::zeroed();
            libc::sigemptyset(&mut set);
            libc::sigaddset(&mut set, libc::SIGCHLD);
            if libc::sigprocmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.process_group(0);
    let mut child = command
        .args(["--exact", name, "--nocapture"])
        .env("ARC_SIGNAL_CASE", name)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let output = child.wait_with_output().unwrap();
            assert!(
                status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .any(|line| line == format!("ARC_SIGNAL_EXECUTED={name}")),
                "isolated test body did not run: {name}\n{}",
                String::from_utf8_lossy(&output.stdout)
            );
            break;
        }
        if Instant::now() >= end {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("isolated signal case timeout");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
struct Target(std::process::Child, bool);
impl Drop for Target {
    fn drop(&mut self) {
        if !self.1 && self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
fn target(signals: &Signals, ignored: bool) -> (Target, RawFd, RawFd) {
    let ready = pipe();
    let events = pipe();
    let gate = pipe();
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        "exec::signals::tests::native_receiver",
        "--nocapture",
    ])
    .env("ARC_SIGNAL_READY", ready[1].to_string())
    .env("ARC_SIGNAL_EVENTS", events[1].to_string())
    .env("ARC_SIGNAL_GATE", gate[0].to_string());
    if ignored {
        cmd.env("ARC_SIGNAL_IGNORED", "1");
    }
    signals.prepare_child(&mut cmd);
    let child = Target(cmd.spawn().unwrap(), false);
    eprintln!(
        "owned target: {:?}",
        arc_core::process_identity::ProcessIdentity::capture(child.0.id()).unwrap()
    );
    assert_eq!(read(ready[0]), 1);
    for fd in [ready[0], ready[1], events[1], gate[0]] {
        unsafe { libc::close(fd) };
    }
    (child, events[0], gate[1])
}
#[test]
fn distinct_preattachment_signals_ignored_disposition_and_detachment() {
    isolated(
        "exec::signals::tests::distinct_preattachment_signals_ignored_disposition_and_detachment",
        || {
            unsafe { libc::signal(libc::SIGUSR2, libc::SIG_IGN) };
            let signals = Signals::install(false).unwrap();
            let (mut child, events, gate) = target(&signals, true);
            for signal in [
                libc::SIGTERM,
                libc::SIGHUP,
                libc::SIGUSR1,
                libc::SIGINT,
                libc::SIGUSR2,
            ] {
                unsafe { libc::raise(signal) };
            }
            assert!(signals.interrupted());
            signals.attach(child.0.id());
            let mut actual = [read(events), read(events), read(events), read(events)];
            actual.sort();
            let mut expected = [libc::SIGTERM, libc::SIGHUP, libc::SIGUSR1, libc::SIGINT];
            expected.sort();
            assert_eq!(actual, expected);
            assert_eq!(
                unsafe { libc::signal(libc::SIGUSR2, libc::SIG_IGN) },
                libc::SIG_IGN
            );
            signals.reaped();
            unsafe { libc::raise(libc::SIGTERM) };
            assert_eq!(PENDING.load(Ordering::SeqCst), 0);
            let mut p = libc::pollfd {
                fd: events,
                events: libc::POLLIN,
                revents: 0,
            };
            assert_eq!(unsafe { libc::poll(&mut p, 1, 0) }, 0);
            write(gate, 17);
            assert_eq!(signals.wait(&mut child.0).unwrap().code(), Some(17));
            child.1 = true;
            for fd in [events, gate] {
                unsafe { libc::close(fd) };
            }
        },
    );
}
#[test]
fn shared_job_terminal_signals_are_not_forwarded_and_reap_disables_ownership() {
    isolated("exec::signals::tests::shared_job_terminal_signals_are_not_forwarded_and_reap_disables_ownership", || {
        let signals = Signals::install(true).unwrap();
        let (mut child, events, gate) = target(&signals, false);
        signals.attach(child.0.id());
        unsafe { libc::raise(libc::SIGINT); libc::raise(libc::SIGQUIT); libc::raise(libc::SIGUSR1); }
        assert_eq!(read(events), libc::SIGUSR1);
        let mask = Mask::block();
        // Queue a wrapper-only TERM while completion is ready; it must never
        // reach a recycled identity after the consuming native wait.
        write(gate, 17);
        unsafe {
            libc::raise(libc::SIGTERM);
            let mut info = std::mem::zeroed();
            assert_eq!(libc::waitid(libc::P_PID, child.0.id(), &mut info, libc::WEXITED | libc::WNOWAIT), 0);
        };
        assert_eq!(signals.wait(&mut child.0).unwrap().code(), Some(17));
        child.1 = true;
        assert_eq!(CHILD.load(Ordering::SeqCst), -1);
        drop(mask);
        assert_eq!(PENDING.load(Ordering::SeqCst), 0);
        for fd in [events, gate] { unsafe { libc::close(fd) }; }
    });
}

#[test]
fn inherited_blocked_sigchld_wakes_after_exit_and_preserves_native_child_mask() {
    isolated("exec::signals::tests::inherited_blocked_sigchld_wakes_after_exit_and_preserves_native_child_mask", blocked_child_exit);
}
#[test]
fn inherited_ignored_blocked_sigchld_preserves_target_state_and_observes_exit17() {
    isolated_sigchld("exec::signals::tests::inherited_ignored_blocked_sigchld_preserves_target_state_and_observes_exit17", true, || {
        assert_eq!(sigchld_action().sa_sigaction, libc::SIG_IGN);
        std::env::set_var("ARC_EXPECT_CHLD_IGNORED", "1");
        blocked_child_exit();
        assert_eq!(sigchld_action().sa_sigaction, libc::SIG_IGN);
    });
}
fn blocked_child_exit() {
    let mut blocked = unsafe { std::mem::zeroed() };
    unsafe {
        libc::sigemptyset(&mut blocked);
        libc::sigaddset(&mut blocked, libc::SIGCHLD);
        libc::sigprocmask(libc::SIG_BLOCK, &blocked, std::ptr::null_mut());
    }
    std::env::set_var("ARC_EXPECT_CHLD_BLOCKED", "1");
    let signals = Signals::install(false).unwrap();
    let (mut child, events, gate) = target(&signals, false);
    let identity = arc_core::process_identity::ProcessIdentity::capture(child.0.id()).unwrap();
    signals.attach(child.0.id());
    let mut released = false;
    let status = signals
        .wait_with(&mut child.0, || {
            assert!(!released, "exit notification must wake the waiter");
            released = true;
            // Called after WNOHANG found no exit, before atomic suspension.
            write(gate, 17);
        })
        .unwrap();
    child.1 = true;
    assert_eq!(status.code(), Some(17));
    assert_eq!(CHILD.load(Ordering::SeqCst), -1);
    unsafe {
        libc::raise(libc::SIGTERM);
    }
    assert_eq!(PENDING.load(Ordering::SeqCst), 0);
    assert_eq!(
        arc_core::process_identity::probe_live(&identity),
        arc_core::process_identity::IdentityStatus::Missing
    );
    let mut after = unsafe { std::mem::zeroed() };
    unsafe { libc::sigprocmask(libc::SIG_SETMASK, std::ptr::null(), &mut after) };
    assert_eq!(unsafe { libc::sigismember(&after, libc::SIGCHLD) }, 1);
    for fd in [events, gate] {
        unsafe { libc::close(fd) };
    }
}
#[test]
fn sigchld_guard_restores_ignored_action_flags_after_spawn_failure_and_early_return() {
    isolated_sigchld("exec::signals::tests::sigchld_guard_restores_ignored_action_flags_after_spawn_failure_and_early_return", true, || {
        let mut original = sigchld_action();
        original.sa_flags |= libc::SA_NOCLDWAIT;
        assert_eq!(unsafe { libc::sigaction(libc::SIGCHLD, &original, std::ptr::null_mut()) }, 0);
        let original = sigchld_action();
        for spawn in [false, true] {
            let signals = Signals::install(false).unwrap();
            assert_ne!(sigchld_action().sa_sigaction, libc::SIG_IGN);
            assert_eq!(sigchld_action().sa_flags & libc::SA_NOCLDWAIT, 0);
            if spawn {
                let mut cmd = Command::new("/missing/arc-signal-fixture");
                signals.prepare_child(&mut cmd);
                assert_eq!(cmd.spawn().unwrap_err().kind(), std::io::ErrorKind::NotFound);
            }
            drop(signals);
            let restored = sigchld_action();
            assert_eq!(restored.sa_sigaction, original.sa_sigaction);
            assert_eq!(restored.sa_flags, original.sa_flags);
            assert_eq!(CHILD.load(Ordering::SeqCst), -1);
        }
    });
}

#[test]
fn private_sigcont_preserves_target_disposition_mask_and_restores_guard_on_every_exit() {
    isolated("exec::signals::tests::private_sigcont_preserves_target_disposition_mask_and_restores_guard_on_every_exit", || {
        let mut block = unsafe {std::mem::zeroed()};
        unsafe {
            libc::sigemptyset(&mut block);
            libc::sigaddset(&mut block, libc::SIGCONT);
            assert_eq!(libc::sigprocmask(libc::SIG_BLOCK, &block, std::ptr::null_mut()), 0);
        }
        for ignored in [false, true] {
            let mut original: libc::sigaction = unsafe {std::mem::zeroed()};
            original.sa_sigaction = if ignored {libc::SIG_IGN} else {libc::SIG_DFL};
            original.sa_flags = libc::SA_RESTART;
            unsafe {
                libc::sigemptyset(&mut original.sa_mask);
                libc::sigaddset(&mut original.sa_mask, libc::SIGUSR1);
                assert_eq!(libc::sigaction(libc::SIGCONT, &original, std::ptr::null_mut()), 0);
            }
            let original = signal_action(libc::SIGCONT);
            std::env::set_var("ARC_EXPECT_CONT_IGNORED", ignored.to_string());
            for shared_job in [false, true] {
                for mode in ["early-return", "spawn-failure", "completion"] {
                    let signals = Signals::install(shared_job).unwrap();
                    assert_ne!(signal_action(libc::SIGCONT).sa_sigaction, libc::SIG_IGN);
                    assert_ne!(signal_action(libc::SIGCONT).sa_sigaction, libc::SIG_DFL);
                    if mode == "completion" {
                        let (mut child, events, gate) = target(&signals, false);
                        signals.attach(child.0.id());
                        write(gate, 17);
                        assert_eq!(signals.wait(&mut child.0).unwrap().code(), Some(17));
                        child.1 = true;
                        assert_eq!(CHILD.load(Ordering::SeqCst), -1);
                        super::receive(libc::SIGCONT);
                        assert_eq!(super::CONTINUE_CHILD.load(Ordering::SeqCst), 0);
                        for fd in [events, gate] {unsafe {libc::close(fd)};}
                    } else if mode == "spawn-failure" {
                        let mut cmd = Command::new("/missing/arc-cont-fixture");
                        signals.prepare_child(&mut cmd);
                        assert_eq!(cmd.spawn().unwrap_err().kind(), std::io::ErrorKind::NotFound);
                    }
                    drop(signals);
                    let restored = signal_action(libc::SIGCONT);
                    assert_eq!(restored.sa_sigaction, original.sa_sigaction);
                    assert_eq!(restored.sa_flags, original.sa_flags);
                    assert_eq!(unsafe {libc::sigismember(&restored.sa_mask, libc::SIGUSR1)}, 1);
                    let mut mask = unsafe {std::mem::zeroed()};
                    unsafe {libc::sigprocmask(libc::SIG_SETMASK, std::ptr::null(), &mut mask)};
                    assert_eq!(unsafe {libc::sigismember(&mask, libc::SIGCONT)}, 1);
                    assert_eq!(CHILD.load(Ordering::SeqCst), -1);
                }
            }
        }
    });
}

#[test]
fn shared_job_terminal_cancellation_before_attachment_blocks_spawn_admission() {
    isolated(
        "exec::signals::tests::shared_job_terminal_cancellation_before_attachment_blocks_spawn_admission",
        || {
            for signal in TERMINAL {
                let guard = Signals::install(true).unwrap();
                assert_eq!(unsafe { libc::raise(signal) }, 0);
                assert!(guard.interrupted());
                let mut invoked = false;
                assert!(super::super::preparation::admit(
                    || !guard.admit_spawn(),
                    || {
                        invoked = true;
                    }
                )
                .is_err());
                assert!(!invoked);
                drop(guard);
            }
        },
    );
}

#[test]
#[should_panic(expected = "isolated test body did not run")]
fn isolated_rejects_an_empty_exact_selection() {
    isolated("unregistered_signal_case", || panic!("must not run"));
}

#[test]
fn shared_job_terminal_signals_between_spawn_and_attach_arrive_once() {
    isolated(
        "exec::signals::tests::shared_job_terminal_signals_between_spawn_and_attach_arrive_once",
        || {
            let signals = Signals::install(true).unwrap();
            assert!(signals.admit_spawn());
            // target() blocks on the executed child's ready pipe. Both are in the
            // isolated foreground job's process group, but attachment has not run.
            let (mut child, events, gate) = target(&signals, false);
            for signal in TERMINAL {
                assert_eq!(unsafe { libc::kill(-libc::getpgrp(), signal) }, 0);
                assert_eq!(read(events), signal);
            }
            // Parent sightings after fork are retained until attach, but must
            // never be forwarded into this shared foreground job a second time.
            assert!(signals.interrupted());
            signals.attach(child.0.id());
            // An acknowledged directed signal is a second barrier after attachment.
            assert_eq!(unsafe { libc::kill(child.0.id() as i32, libc::SIGUSR1) }, 0);
            assert_eq!(read(events), libc::SIGUSR1);
            write(gate, 17);
            assert_eq!(signals.wait(&mut child.0).unwrap().code(), Some(17));
            child.1 = true;
            let mut extra = 0i32;
            assert_eq!(
                unsafe { libc::read(events, (&mut extra as *mut i32).cast(), 4) },
                0,
                "duplicate terminal delivery"
            );
            for fd in [events, gate] {
                unsafe { libc::close(fd) };
            }
        },
    );
}
