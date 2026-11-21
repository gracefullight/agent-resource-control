//! Escalation and an injected evidence fault through the same production route.
use super::{
    cli, protocol,
    reaper_fixture::{orphanable, until, Reaper, SESSION_GRACE_MS},
    support,
};
use arc_core::process_identity::{probe_live, IdentityStatus};
use arc_ipc::jobs::Outcome;
use std::fs;

#[test]
fn indeterminate_identity_sends_nothing_and_ignored_sigterm_escalates_after_grace() {
    let _serial = support::serial();
    let mut reaper = Reaper::start();
    let (mut launcher, gate, job) = orphanable(&reaper, "ignored-term");
    let child = job.child.clone().unwrap();
    drop(launcher.child.stdin.take());
    cli::wait(&mut launcher.child);
    let ended = 20_000_000;
    reaper.clock(ended);
    reaper.pass();

    // Injected: this daemon cannot read the child's native evidence.
    let fault = reaper.control.join("fault-child-evidence");
    fs::write(&fault, "indeterminate").unwrap();
    let eligible = ended + SESSION_GRACE_MS;
    reaper.clock(eligible);
    reaper.pass();
    assert_eq!(
        reaper.decisions(&job.job_id).last().unwrap(),
        &(
            "RETRY_LATER".to_owned(),
            "IDENTITY_INDETERMINATE".to_owned(),
            eligible
        )
    );
    assert_eq!(probe_live(&child), IdentityStatus::VerifiedMatch);
    fs::remove_file(&fault).unwrap();

    reaper.pass();
    // The child ignores SIGTERM. Inside the grace period nothing escalates.
    reaper.clock(eligible + 4_999);
    reaper.pass();
    assert_eq!(probe_live(&child), IdentityStatus::VerifiedMatch);
    assert_eq!(protocol::status(&reaper.f, &job.job_id).outcome, None);
    assert_eq!(
        reaper.decisions(&job.job_id).last().unwrap(),
        &(
            "TERMINATING".to_owned(),
            "SIGTERM_REQUESTED".to_owned(),
            eligible
        )
    );

    let escalated = eligible + 5_000;
    reaper.clock(escalated);
    reaper.pass();
    let finished = until("outcome from the wrapper's consuming wait", || {
        let job = protocol::status(&reaper.f, &job.job_id);
        job.outcome.is_some().then_some(job)
    });
    assert_eq!(
        finished.outcome,
        Some(Outcome::Signaled {
            signal: libc::SIGKILL as u8,
            core_dumped: false
        })
    );
    drop(gate);
    reaper.clock(escalated + 1);
    reaper.pass();
    let decisions = reaper.decisions(&job.job_id);
    assert_eq!(
        decisions[decisions.len() - 2..],
        [
            (
                "TERMINATING".to_owned(),
                "SIGKILL_REQUESTED".to_owned(),
                escalated
            ),
            (
                "RECLAIMED".to_owned(),
                "TERMINATION_CONFIRMED".to_owned(),
                escalated + 1
            ),
        ],
        "{decisions:?}"
    );
    reaper.stop();
}

#[test]
fn injected_authority_and_stale_request_faults_send_no_signal_and_never_claim_reclaim() {
    let _serial = support::serial();
    let mut reaper = Reaper::start();
    let (mut launcher, mut gate, job) = orphanable(&reaper, "echo");
    let child = job.child.clone().unwrap();
    drop(launcher.child.stdin.take());
    cli::wait(&mut launcher.child);
    let ended = 20_000_000;
    reaper.clock(ended);
    reaper.pass();
    let eligible = ended + SESSION_GRACE_MS;
    reaper.clock(eligible);

    // Injected: this daemon holds no capability for the job.
    let authority = reaper.control.join("fault-authority");
    fs::write(&authority, "lost").unwrap();
    reaper.pass();
    assert_eq!(
        reaper.decisions(&job.job_id).last().unwrap(),
        &(
            "BLOCKED".to_owned(),
            "INSUFFICIENT_AUTHORITY".to_owned(),
            eligible
        )
    );
    fs::remove_file(&authority).unwrap();

    // Injected: the request carries a record with a stale start time while
    // the child it names is alive. The wrapper itself refuses to signal.
    fs::write(reaper.control.join("fault-recorded-identity"), "request").unwrap();
    reaper.pass();
    let decisions = reaper.decisions(&job.job_id);
    assert_eq!(
        decisions[decisions.len() - 2..],
        [
            (
                "TERMINATING".to_owned(),
                "SIGTERM_REQUESTED".to_owned(),
                eligible
            ),
            (
                "BLOCKED".to_owned(),
                "WRAPPER_IDENTITY_MISMATCH".to_owned(),
                eligible
            ),
        ],
        "{decisions:?}"
    );
    assert_eq!(probe_live(&child), IdentityStatus::VerifiedMatch);

    // The child then exits by its own code: it was never signalled, and a
    // claim that sent nothing is not recorded as a reclaim.
    use std::io::Write;
    gate.write_all(b"x").unwrap();
    let finished = until("the child's own exit", || {
        let job = protocol::status(&reaper.f, &job.job_id);
        job.outcome.is_some().then_some(job)
    });
    assert_eq!(finished.outcome, Some(Outcome::Exited { code: 17 }));
    reaper.clock(eligible + 1);
    reaper.pass();
    assert_eq!(
        reaper.decisions(&job.job_id).last().unwrap(),
        &(
            "SKIPPED".to_owned(),
            "JOB_COMPLETED".to_owned(),
            eligible + 1
        )
    );
    assert_eq!(
        reaper.resource(&job.job_id),
        Some(("PROVEN".to_owned(), "RELEASED".to_owned()))
    );
    reaper.stop();
}
