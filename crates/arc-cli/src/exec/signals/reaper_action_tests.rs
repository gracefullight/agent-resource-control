//! The wait gate and fresh identity decide every Reaper signal.
use super::super::reaper_action::{act, wire};
use super::{isolated, read, target, write, Signals};
use arc_core::process_identity::ProcessIdentity;
use arc_ipc::reaper::{ActionOutcome, Refusal, Signal};
use arc_platform::reaper_identity::{self, ChildIdentity};
use std::os::unix::process::ExitStatusExt;

fn refused(reason: Refusal) -> ActionOutcome {
    ActionOutcome::Refused { reason }
}
fn owned(pid: u32) -> ChildIdentity {
    reaper_identity::capture(&ProcessIdentity::capture(pid).unwrap()).unwrap()
}

#[test]
fn mismatched_identity_is_refused_and_each_accepted_request_sends_one_signal() {
    isolated(
        "exec::signals::tests::reaper_action_tests::mismatched_identity_is_refused_and_each_accepted_request_sends_one_signal",
        || {
            let signals = Signals::install(false).unwrap();
            let (mut child, events, gate) = target(&signals, false);
            let own = owned(child.0.id());
            let evidence = wire(&own).unwrap();
            // Not yet attached: this wrapper does not supervise the PID.
            assert_eq!(
                act(&own, &evidence, Signal::Term),
                refused(Refusal::NotSupervised)
            );
            signals.attach(child.0.id());

            // The daemon's record names a different birth for the live PID.
            let mut stale = evidence.clone();
            stale.process.start_token =
                (stale.process.start_token.parse::<u64>().unwrap() - 1).to_string();
            assert_eq!(
                act(&own, &stale, Signal::Term),
                refused(Refusal::IdentityMismatch)
            );
            // Both records are stale relative to the live child: the shape of
            // a reused PID, exercised without claiming the OS reused one.
            let mut reused = own.clone();
            reused.process.start_token = stale.process.start_token.clone();
            assert_eq!(
                act(&reused, &wire(&reused).unwrap(), Signal::Term),
                refused(Refusal::IdentityMismatch)
            );
            for change in [
                |c: &mut ChildIdentity| c.executable_inode ^= 1,
                |c: &mut ChildIdentity| c.executable.push("-other"),
                |c: &mut ChildIdentity| c.effective_uid ^= 1,
                |c: &mut ChildIdentity| c.real_uid ^= 1,
                |c: &mut ChildIdentity| c.parent_pid += 1,
            ] {
                let mut changed = own.clone();
                change(&mut changed);
                assert_eq!(
                    act(&changed, &wire(&changed).unwrap(), Signal::Term),
                    refused(Refusal::IdentityMismatch)
                );
                assert_eq!(
                    act(&own, &wire(&changed).unwrap(), Signal::Term),
                    refused(Refusal::IdentityMismatch)
                );
            }
            let mut other = evidence.clone();
            other.process.pid = std::process::id();
            assert_eq!(
                act(&own, &other, Signal::Term),
                refused(Refusal::NotSupervised)
            );
            // Pending signals are delivered lowest number first, so a refused
            // TERM that had been sent would be observed before this marker.
            unsafe { libc::raise(libc::SIGUSR1) };
            assert_eq!(read(events), libc::SIGUSR1);

            assert_eq!(
                act(&own, &evidence, Signal::Term),
                ActionOutcome::Signaled {}
            );
            assert_eq!(read(events), libc::SIGTERM);
            // Escalation is a separate request with the same full check.
            assert_eq!(
                act(&own, &evidence, Signal::Kill),
                ActionOutcome::Signaled {}
            );
            let status = signals.wait(&mut child.0).unwrap().status;
            assert_eq!(status.signal(), Some(libc::SIGKILL));
            child.1 = true;
            // The consuming wait cleared ownership under the gate.
            assert_eq!(
                act(&own, &evidence, Signal::Kill),
                refused(Refusal::NotSupervised)
            );
            for fd in [events, gate] {
                unsafe { libc::close(fd) };
            }
        },
    );
}

#[test]
fn no_signal_is_sent_after_reaped_clears_supervision() {
    isolated(
        "exec::signals::tests::reaper_action_tests::no_signal_is_sent_after_reaped_clears_supervision",
        || {
            let signals = Signals::install(false).unwrap();
            let (mut child, events, gate) = target(&signals, false);
            let own = owned(child.0.id());
            let evidence = wire(&own).unwrap();
            signals.attach(child.0.id());
            signals.reaped();
            for signal in [Signal::Term, Signal::Kill] {
                assert_eq!(
                    act(&own, &evidence, signal),
                    refused(Refusal::NotSupervised)
                );
            }
            // The fixture still owns its unreaped child and may mark the stream.
            assert_eq!(
                unsafe { libc::kill(child.0.id() as i32, libc::SIGUSR1) },
                0
            );
            assert_eq!(read(events), libc::SIGUSR1);
            write(gate, 17);
            assert_eq!(signals.wait(&mut child.0).unwrap().status.code(), Some(17));
            child.1 = true;
            for fd in [events, gate] {
                unsafe { libc::close(fd) };
            }
        },
    );
}
