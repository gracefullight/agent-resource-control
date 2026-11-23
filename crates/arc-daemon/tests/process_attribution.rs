//! Native process attribution through a real daemon, built `arcctl exec`,
//! the installed hook with a recorded Claude Code payload and offline fixtures.
//! Every wait is on observed state; the 60 s deadlines are watchdogs only.
#[path = "process_attribution/first_baseline.rs"]
mod first_baseline;
#[path = "process_attribution/fixture.rs"]
mod fixture;
#[allow(dead_code)] // Only the installed hook and its returned command are used here.
#[path = "hook_event_identity/hook.rs"]
mod hook;
#[path = "support/journey.rs"]
mod journey;
#[path = "process_attribution/lease.rs"]
mod lease;
#[path = "process_attribution/ownership.rs"]
mod ownership;
#[path = "process_attribution/phases.rs"]
mod phases;
#[allow(dead_code)] // The shared fixture also serves unrelated workloads.
#[path = "support/runtime.rs"]
mod runtime;
#[path = "process_attribution/summary.rs"]
mod summary;
#[path = "process_attribution/serial.rs"]
mod target_serial;
#[allow(dead_code)] // Only the sleeping root and its samples are used here.
#[path = "job_process_meter/tree.rs"]
mod tree;

use arc_core::process_identity::{probe_live, IdentityStatus};
use arc_ipc::meter::quality::Availability;
use arc_storage::attribution::IntervalStore;
use journey::Sandbox;
use runtime::{child_watchdog, Fixture};
use std::{path::Path, process::Stdio, time::Duration};
use summary::*;

/// The one Meter hook now composes the job-tree sampler with the
/// attribution scan: a RUNNING managed job is still sampled from its
/// recorded child identity on the daemon's own cadence, across at least two
/// passes observed in daemon state.
#[test]
fn seam_composed_meter_cadence_still_samples_the_managed_job_tree() {
    let _serial = target_serial::acquire();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    tree::compile(&f, &s);
    let mut daemon = fixture::daemon(&f, "self.sock");
    let db = journey::database(&f);

    let directory = tree::directory(&f, "sleeper");
    let mut command = s.arcctl(&f, &project);
    command
        .args(["exec", "--", tree::PROGRAM, "sleeper"])
        .arg(&directory)
        .stdin(std::process::Stdio::null());
    let mut exec = child_watchdog::Owned::spawn(&mut command).unwrap();
    let pid = tree::published(&directory, "sleeper.pid");
    let job = tree::running_job(&db, pid);
    journey::eventually("two job-tree samples of the managed root", || {
        let rows = tree::rows(&db, &job);
        let timestamps: std::collections::BTreeSet<_> =
            rows.iter().map(|row| row.timestamp_ns).collect();
        timestamps.len() >= 2
    });
    tree::release(&directory);
    assert!(exec.wait("managed sleeper").unwrap().success());
    daemon.stop(libc::SIGTERM);
    f.report();
}

/// Real managed and outside fixtures establish each class, scan boundaries,
/// identity reuse, clock failures and first-baseline recovery. Each fixture
/// observes only itself and belongs to an owned, reaped process set.
#[test]
fn native_process_attribution_separates_proven_estimated_and_unattributed() {
    let _serial = target_serial::acquire();
    let reuse_deadline = std::time::Instant::now() + Duration::from_secs(35);
    let started_ms = now_ms();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    let programs = fixture::compile(&f, &s);
    let mut daemon = fixture::daemon(&f, "self.sock");
    let mut db = journey::database(&f);
    db.busy_timeout(Duration::from_secs(60)).unwrap();
    let store = IntervalStore::beside(&f.database).unwrap();
    let mut lifeline = fixture::Lifeline::new(&f);
    // (d) first: its PID cycle runs while the other phases proceed.
    let respawn_dir = tree::directory(&f, "respawn");
    let respawn_args = [Path::new("respawn"), &respawn_dir, &programs.burner];
    fixture::detach(
        &f,
        &s,
        &project,
        &programs,
        &lifeline,
        &programs.burner,
        &respawn_args,
    );
    let respawner = identity(tree::published(&respawn_dir, "respawner.pid"));
    let first_life = identity(tree::published(&respawn_dir, "respawn-1.pid"));

    let installed = hook::Vendor::Claude.install(&f, &s, &project);
    let event = hook::claude_event(&project, "cargo build");
    let command = hook::Vendor::Claude.returned_command(&f, &s, &project, &installed, &event);
    assert!(
        command.contains(" hook-exec --arc-event ") && command.ends_with(" -- cargo build"),
        "{command}"
    );
    // (a) The vendor's shell runs the hook's returned managed command.
    let managed_dir = tree::directory(&f, "managed");
    let mut shell = s.shell(&f, &project, &command);
    lifeline
        .arm(&mut shell)
        .env("ARC_ATTRIBUTION_DIR", &managed_dir)
        .env("ARC_ATTRIBUTION_SELF_OBSERVE", f.root.join("self.sock"))
        .stdin(Stdio::null());
    let mut managed = child_watchdog::Owned::spawn(&mut shell).unwrap();
    let root = identity(tree::published(&managed_dir, "root.pid"));
    let child = identity(tree::published(&managed_dir, "child.pid"));
    let job = tree::running_job(&db, u64::from(root.pid));
    let observer = phases::observer_with_sent_target(&f, &root);
    let vendor: Option<String> = db
        .query_row("SELECT vendor FROM jobs WHERE id=?1", [&job], |r| r.get(0))
        .unwrap();
    assert_eq!(vendor.as_deref(), Some("claude-code"));
    // (b) An agent-named executable outside ARC starts a burner.
    let agent_dir = tree::directory(&f, "agent");
    let burn_args = [Path::new("agent"), &agent_dir, &programs.burner];
    fixture::detach(
        &f,
        &s,
        &project,
        &programs,
        &lifeline,
        &programs.agent,
        &burn_args,
    );
    let agent = identity(tree::published(&agent_dir, "agent.pid"));
    let agent_burner = identity(tree::published(&agent_dir, "agent-burner.pid"));
    // (c) A burner outside ARC with no agent ancestry.
    let outside_dir = tree::directory(&f, "outside");
    let outside_args = [Path::new("burn"), Path::new("outside"), &outside_dir];
    fixture::detach(
        &f,
        &s,
        &project,
        &programs,
        &lifeline,
        &programs.burner,
        &outside_args,
    );
    let outside = identity(tree::published(&outside_dir, "outside.pid"));
    let mut reaped: Vec<_> = [&root, &child, &agent, &agent_burner, &outside, &respawner]
        .into_iter()
        .chain([&first_life])
        .map(|id| tree::Reaped(id.clone()))
        .collect();

    let fixtures = [&root, &child, &agent_burner, &outside];
    let requests = [
        fixture::observation(&managed_dir, "root"),
        fixture::observation(&managed_dir, "child"),
        fixture::observation(&agent_dir, "agent-burner"),
        fixture::observation(&outside_dir, "outside"),
    ];
    // At least two counted scans of each, observed while the job is RUNNING.
    let observed: Vec<_> = requests.iter().map(|r| fixture::scanned(r, 2)).collect();
    let state: String = db
        .query_row("SELECT state FROM jobs WHERE id=?1", [&job], |r| r.get(0))
        .unwrap();
    assert_eq!(state, "RUNNING", "the active cadence held throughout");
    assert_eq!(
        observed[0].only(),
        ("proven", "claude-code"),
        "managed root"
    );
    assert_eq!(
        observed[1].only(),
        ("proven", "claude-code"),
        "managed descendant"
    );
    assert_eq!(
        observed[2].only(),
        ("estimated", "claude-code"),
        "agent descendant"
    );
    assert_eq!(
        observed[3].only(),
        ("unattributed", ""),
        "unknown outside process"
    );
    for (probe, role) in observed
        .iter()
        .zip(["root", "child", "agent-burner", "outside"])
    {
        assert_counted_only_after_first_sighting(probe, role);
    }

    phases::assert_observer_ignores_a_sent_target(observer);
    // Native oracle: what the daemon counted for a process is positive and at
    // most the CPU that process had used when read afterwards.
    for (probe, id) in observed.iter().zip(fixtures) {
        let cpu = fixture::cpu_seconds(id).expect("still running");
        assert!(
            probe.total() > 0.0 && probe.total() <= cpu,
            "{probe:?} > {cpu}"
        );
    }
    // The job-tree sampler still advances beside the scan.
    journey::eventually("two job-tree samples of the managed root", || {
        let rows = tree::rows(&db, &job);
        let stamps: std::collections::BTreeSet<_> = rows.iter().map(|r| r.timestamp_ns).collect();
        stamps.len() >= 2
    });

    // A recorded child identity that no longer names the live root proves
    // nothing: root and descendant leave Proven until it is restored.
    let (token, _) = tree::recycle(&mut db, &job);
    let before: Vec<_> = requests[..2]
        .iter()
        .map(|r| fixture::probe(r).unwrap())
        .collect();
    journey::eventually("managed members counted outside Proven", || {
        requests[..2].iter().zip(&before).all(|(request, before)| {
            fixture::probe(request).is_some_and(|p| {
                p.scans > before.scans && p.classes.iter().any(|c| c.confidence != "proven")
            })
        })
    });
    tree::restore(&db, &job, &token);
    let restored: Vec<_> = requests[..2]
        .iter()
        .map(|r| fixture::probe(r).unwrap())
        .collect();
    journey::eventually("managed members Proven again", || {
        requests[..2]
            .iter()
            .zip(&restored)
            .all(|(r, b)| fixture::probe(r).is_some_and(|p| proven(&p) > proven(b)))
    });

    // The whole run so far, read after the probes: every probe line was sent
    // after its interval was committed, so the summary includes it.
    let probes: Vec<_> = requests
        .iter()
        .map(|r| fixture::probe(r).unwrap())
        .collect();
    let run_to = now_ms() + 1;
    let run = attribution(&f, &s, &project, started_ms, run_to);
    assert!(run.window.scans >= 2);
    assert!(run.window.first_scan_ms.unwrap() >= started_ms);
    assert!(run.window.last_scan_ms.unwrap() <= run_to);
    assert_eq!(vendors(&run.proven), ["claude-code"]);
    let run_proven = group(&run.proven, "claude-code");
    assert!(run_proven + ROUNDING >= proven(&probes[0]) + proven(&probes[1]));
    assert!(vendors(&run.estimated)
        .iter()
        .all(|v| v == "claude-code" || v == "cursor"));
    assert!(group(&run.estimated, "claude-code") + ROUNDING >= probes[2].total());
    assert!(run.unattributed.value.unwrap() + ROUNDING >= probes[3].total());
    assert_own_ratio(&run);
    assert_matches_store(&run, &store);
    assert_default_vendor_summary(&f, &s, &project, &store);
    // Missing samples: a window before the daemon ran is unavailable, not 0.
    let early = attribution(
        &f,
        &s,
        &project,
        started_ms - 20 * 60_000,
        started_ms - 19 * 60_000,
    );
    assert_eq!(early.window.scans, 0);
    assert!(early.proven.is_empty() && early.estimated.is_empty());
    assert_eq!(early.unattributed.value, None);
    assert_eq!(early.unattributed.reason.as_deref(), Some("no_scan"));
    assert_eq!(
        early.unattributed_ratio.availability,
        Availability::Unavailable
    );

    // Window bounds tied to the outside burner's own reads.
    phases::assert_envelopes(&f, &s, &project, &store, &requests[3]);
    // One PID, two lifetimes, consecutive scans.
    let second_life =
        phases::assert_same_pid_replacement(&respawn_dir, &first_life, reuse_deadline);
    assert_ne!(probe_live(&first_life), IdentityStatus::VerifiedMatch);
    reaped.push(tree::Reaped(second_life.clone()));

    // A real failed pass: the write-protected store refuses the commit and
    // the scanner's mark alike; nothing durable records the failure.
    let failure = phases::unresolved_failure(&store, &requests[3]);
    assert_eq!(
        phases::minute(failure.earlier.previous.start_ms),
        phases::minute(failure.at_ms),
        "two committed scans and the failure share one calendar minute: {failure:?}"
    );
    assert_eq!(
        failure.seq,
        failure.last.scan.seq + 1,
        "the next pass failed"
    );
    phases::assert_unresolved(&f, &s, &project, &store, &failure, true, "before recovery");
    // The wall clock steps back over the earlier interval while nothing can
    // be recorded: only this daemon knows, and its reads stay unresolved.
    let reversal = phases::reverse_clock(&f, &requests[3], &failure);
    phases::assert_reversed(
        &f,
        &s,
        &project,
        &store,
        &reversal,
        "pending_scan",
        "before recovery",
    );
    phases::assert_unresolved(&f, &s, &project, &store, &failure, false, "reversed");
    // The clock is corrected while writes stay blocked: a pass on the
    // corrected clock fails too, and the reversed window stays unresolved.
    phases::correct_clock(&f, &requests[3], &reversal);
    phases::assert_reversed(
        &f,
        &s,
        &project,
        &store,
        &reversal,
        "pending_scan",
        "corrected",
    );
    phases::assert_unresolved(&f, &s, &project, &store, &failure, false, "corrected");
    // The managed job ends, then the daemon restarts before any recovery.
    tree::release(&managed_dir);
    assert!(managed.wait("managed hook job").unwrap().success());
    let waited = waited_cpu(&f, &s, &project, &job);
    let old_scanner = intervals(&store)
        .iter()
        .find(|i| i.measured)
        .unwrap()
        .scanner
        .clone();
    daemon.stop(libc::SIGTERM);
    // Its clean stop could not seal the frontier: the fence stays.
    assert_eq!(sealed(&store, &old_scanner), Some(false));
    // The first daemon's own report; each daemon creates its report anew.
    f.report();
    std::fs::remove_file(f.root.join("runtime.json")).unwrap();
    let restarted_ms = now_ms();
    daemon = fixture::daemon(&f, "self2.sock");
    // The corrected clock shows the restarted scanner no step, and nothing
    // was written since: the old frontier's fence keeps every earlier window
    // unresolved, the reversed one included.
    phases::assert_unresolved(&f, &s, &project, &store, &failure, false, "after restart");
    phases::assert_reversed(
        &f,
        &s,
        &project,
        &store,
        &reversal,
        "pending_scan",
        "after restart",
    );
    // Recovery: the store is writable again; the new scanner's baseline
    // resolves the old frontier's fence and the reversed range into one
    // failed interval from 0, on the active cadence of a second managed job.
    phases::protect(&store, false);
    let (second_dir, mut second, second_job) = lease::start(&f, &s, &project, &db, &lifeline);
    let mut resolved = None;
    journey::eventually("the old frontier resolved into a failed interval", || {
        let stored = intervals(&store);
        resolved = stored
            .iter()
            .position(|i| !i.measured && i.scanner != old_scanner)
            .filter(|&at| {
                stored[at + 1..]
                    .iter()
                    .filter(|i| i.measured && i.start >= stored[at].end)
                    .count()
                    >= 2
            })
            .map(|at| (stored[at].clone(), stored.clone()));
        resolved.is_some()
    });
    let (gap, stored) = resolved.unwrap();
    assert_eq!(
        gap.start, 0,
        "the unsealed fence reaches back over the reversed range: {reversal:?} {stored:?}"
    );
    assert!(gap.end > restarted_ms, "{stored:?}");
    assert_eq!(
        stored
            .iter()
            .filter(|i| !i.measured && i.scanner != old_scanner)
            .count(),
        1,
        "one failed interval resolves the restart: {stored:?}"
    );
    let after = stored
        .iter()
        .find(|i| i.measured && i.start >= gap.end)
        .unwrap();
    let after_window = (after.start, after.end);
    let across = attribution(&f, &s, &project, failure.earlier.interval.0, after.end);
    assert_eq!(
        across.unattributed_ratio.reason.as_deref(),
        Some("failed_scan")
    );
    assert_eq!(across.unattributed.availability, Availability::Partial);
    assert_matches_store(&across, &store);
    let clear = attribution(&f, &s, &project, after.start, after.end);
    assert_eq!(clear.window.scans, 1);
    assert_own_ratio(&clear);
    phases::assert_reversed(
        &f,
        &s,
        &project,
        &store,
        &reversal,
        "failed_scan",
        "after recovery",
    );
    let child_proven = lease::check(lease::Context {
        f: &f,
        s: &s,
        db: &db,
        project: &project,
        programs: &programs,
        lifeline: &lifeline,
        directory: &second_dir,
        job: &second_job,
        second: &mut second,
    });
    let waited = waited + waited_cpu(&f, &s, &project, &second_job);

    for directory in [&agent_dir, &outside_dir, &respawn_dir] {
        tree::release(directory);
    }
    journey::eventually("detached fixtures exited", || {
        [&agent, &agent_burner, &outside, &respawner, &first_life]
            .iter()
            .copied()
            .chain(std::iter::once(&second_life))
            .all(|id| probe_live(id) != IdentityStatus::VerifiedMatch)
    });
    let finals: Vec<_> = requests
        .iter()
        .map(|r| fixture::probe(r).unwrap())
        .collect();
    for (last, first) in finals.iter().zip(&observed) {
        assert!(last.scans >= first.scans);
    }
    assert_eq!(finals[2].only(), ("estimated", "claude-code"));
    assert_eq!(finals[3].only(), ("unattributed", ""));

    // The whole run including the gap: values are lower bounds of every
    // measured interval, the ratio is unavailable, and Proven is at most
    // what wait4 charged to the two managed jobs.
    let whole = attribution(&f, &s, &project, started_ms, now_ms() + 1);
    assert_eq!(whole.unattributed_ratio.value, None);
    assert_eq!(
        whole.unattributed_ratio.reason.as_deref(),
        Some("failed_scan")
    );
    assert!(vendors(&whole.proven)
        .iter()
        .all(|v| v == "claude-code" || v == "none"));
    let whole_proven: f64 = whole
        .proven
        .iter()
        .map(|g| g.cpu_core_seconds.value.unwrap())
        .sum();
    assert!(
        group(&whole.proven, "claude-code") + ROUNDING >= proven(&finals[0]) + proven(&finals[1])
    );
    assert!(
        whole_proven <= waited + child_proven + ROUNDING,
        "{whole_proven} > waited {waited} plus unwaited child {child_proven}"
    );
    assert!(group(&whole.estimated, "claude-code") + ROUNDING >= finals[2].total());
    assert!(whole.unattributed.value.unwrap() + ROUNDING >= finals[3].total());
    assert_matches_store(&whole, &store);

    assert_aggregates_only(&f, &s, &project, &store);
    drop(reaped);
    drop(db);
    let new_scanner = intervals(&store).last().unwrap().scanner.clone();
    daemon.stop(libc::SIGTERM);
    assert_eq!(sealed(&store, &new_scanner), Some(true));
    f.report();
    first_baseline::failed_first_baseline_after_clean_stop_keeps_the_earlier_window_unavailable(
        &f,
        &s,
        &project,
        &lifeline,
        &store,
        &new_scanner,
        after_window,
    );
    lifeline.cancel().unwrap();
}
