//! Bounded Reaper through the built CLI, the real daemon route and a real child.
//!
//! `inject` sets a deterministic fault of the Reaper fixture daemon; each use
//! is labelled. A live child paired with a stale record exercises
//! stale-identity handling: no test here observes the OS reusing a PID.
use super::{
    cli, protocol,
    reaper_fixture::{
        isolate, managed, orphanable, running, until, ManagedCli, Reaper, SESSION_GRACE_MS,
    },
    runtime, support,
};
use arc_core::process_identity::{probe, probe_live, IdentityStatus, ProcessIdentity};
use arc_ipc::jobs::{JobId, JobSnapshot, Outcome, StartRequest};
use std::{
    fs,
    io::Write,
    os::unix::net::{UnixListener, UnixStream},
    process::Command,
    time::Duration,
};

/// When the fixtures below observe the ended owner session, on the Reaper clock.
const ENDED: u64 = 20_000_000;
const TERM_GRACE_MS: u64 = 5_000;
const ALIVE: IdentityStatus = IdentityStatus::VerifiedMatch;
/// A recorded decision: (state, reason, observation time).
type Row = (String, String, u64);

fn row(state: &str, reason: &str, at: u64) -> Row {
    (state.to_owned(), reason.to_owned(), at)
}
fn proven(state: &str) -> Option<(String, String)> {
    Some(("PROVEN".to_owned(), state.to_owned()))
}
fn signaled(signal: i32) -> Outcome {
    Outcome::Signaled {
        signal: signal as u8,
        core_dumped: false,
    }
}
/// The outcome that only the wrapper's consuming wait records.
fn outcome(reaper: &Reaper, job: &JobId) -> Outcome {
    until("outcome from the wrapper's consuming wait", || {
        protocol::status(&reaper.f, job).outcome
    })
}
/// Run exactly one pass at this time on the Reaper clock.
fn pass_at(reaper: &mut Reaper, now: u64) {
    reaper.clock(now);
    reaper.pass();
}
/// A registered child whose owner session has ended and whose grace period
/// has elapsed: (child control stream, job, the clock value now set).
fn eligible(reaper: &mut Reaper, mode: &str) -> (UnixStream, JobSnapshot, u64) {
    let (mut launcher, gate, job) = orphanable(reaper, mode);
    drop(launcher.child.stdin.take());
    cli::wait(&mut launcher.child);
    pass_at(reaper, ENDED);
    reaper.clock(ENDED + SESSION_GRACE_MS);
    (gate, job, ENDED + SESSION_GRACE_MS)
}
/// A wrapper started by this test process, which is therefore its live owner
/// session: (wrapper, child control stream, child identity, running job).
fn wrapped(f: &runtime::Fixture) -> (ManagedCli, UnixStream, ProcessIdentity, JobSnapshot) {
    let control = f.root.join("managed-child.sock");
    let listener = UnixListener::bind(&control).unwrap();
    let command = managed(f, &control, &f.root.join("cli-home"))
        .spawn()
        .unwrap();
    let wrapper = ManagedCli {
        child: runtime::OwnedChild::new(command),
        completed: false,
    };
    let (gate, message) = cli::accept(&listener);
    let child: ProcessIdentity = serde_json::from_value(message["identity"].clone()).unwrap();
    let parent = wrapper.child.child.id() as u64;
    assert_eq!(message["ppid"].as_u64(), Some(parent));
    let job = running(f, &child);
    (wrapper, gate, child, job)
}
/// Set or clear one injected fault of the fixture daemon.
fn inject(reaper: &Reaper, name: &str, value: Option<&str>) {
    let path = reaper.control.join(format!("fault-{name}"));
    match value {
        Some(value) => fs::write(path, value).unwrap(),
        None => fs::remove_file(path).unwrap(),
    }
}
/// Impose or lift one condition. `lease` and `protected` are written to the
/// ownership tables as another holder or a policy would write them; every
/// other name is an injected fault of the fixture daemon.
fn impose(reaper: &Reaper, job: &JobId, name: &str, value: &str, on: bool) {
    let resource = format!("process:{}", job.as_str());
    let sql = match (name, on) {
        ("lease", true) => "INSERT INTO resource_leases(id,resource_id,acquired_at,state) VALUES('other',?1,1,'ACTIVE')",
        ("lease", false) => "DELETE FROM resource_leases WHERE id='other' AND resource_id=?1",
        ("protected", true) => "UPDATE resource_objects SET protected=1 WHERE id=?1",
        ("protected", false) => "UPDATE resource_objects SET protected=0 WHERE id=?1",
        _ => return inject(reaper, name, on.then_some(value)),
    };
    let database = reaper.database();
    database.busy_timeout(Duration::from_secs(60)).unwrap();
    assert_eq!(database.execute(sql, [&resource]).unwrap(), 1, "{name}");
}
/// Restart the fixture daemon on the same database: no capability survives.
fn restart(reaper: &mut Reaper) {
    reaper.daemon.stop(libc::SIGTERM);
    let root = &reaper.f.root;
    fs::rename(root.join("runtime.json"), root.join("prior-runtime.json")).unwrap();
    // The restarted daemon runs no pass until this test requests the next one.
    fs::remove_file(reaper.control.join("pass")).unwrap();
    let mut command = reaper.f.isolated_command("test");
    command
        .env("ARC_REAPER", "process")
        .arg("--fixture-control")
        .arg(&reaper.control);
    reaper.daemon = runtime::OwnedChild::new(command.spawn().unwrap());
    let Reaper { f, daemon, .. } = reaper;
    f.wait_for(daemon, "Sweeper off. Process Reaper on.");
}

#[test]
fn managed_route_tracks_owned_child() {
    let _serial = support::serial();
    let f = runtime::Fixture::new();
    let mut daemon = f.spawn_isolated("test");
    fs::create_dir(f.root.join("cli-home")).unwrap();
    // The job row names the wrapper and the child's PID, start time and boot,
    // and a fresh native capture of that PID is the same identity.
    let (mut wrapper, mut gate, child, job) = wrapped(&f);
    assert_eq!(job.observer.pid, wrapper.child.child.id());
    assert_eq!(ProcessIdentity::capture(child.pid).unwrap(), child);
    assert_eq!(probe_live(&child), ALIVE);
    let mut earlier = child.clone();
    earlier.start_token = (child.start_token.parse::<u64>().unwrap() - 1).to_string();
    assert_eq!(probe_live(&earlier), IdentityStatus::Mismatch);
    gate.write_all(b"x").unwrap();
    let status = cli::wait(&mut wrapper.child.child);
    wrapper.completed = true;
    assert_eq!(status.code(), Some(17));
    assert_eq!(probe(&child), IdentityStatus::Missing);
    // The resident daemon runs no Reaper unless it is explicitly configured:
    // it accepts no registration and records no ownership or GC row.
    let database = rusqlite::Connection::open(&f.database).unwrap();
    for table in ["resource_objects", "resource_leases", "gc_operations"] {
        let count = format!("SELECT count(*) FROM {table}");
        let rows: i64 = database.query_row(&count, [], |row| row.get(0)).unwrap();
        assert_eq!(rows, 0, "{table}");
    }
    drop(database);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn owned_inactive_child_reclaimed_and_recorded() {
    let _serial = support::serial();
    let mut reaper = Reaper::start();
    // The launcher is the owner session: it starts the wrapper and stays alive
    // until this test closes its standard input.
    let (mut launcher, gate, job) = orphanable(&reaper, "echo");
    let (id, child, wrapper) = (&job.job_id, job.child.clone().unwrap(), &job.observer);
    // Long past every grace period, a live owner session still protects it.
    pass_at(&mut reaper, 10_000_000);
    assert_eq!(probe_live(&child), ALIVE);
    // Observed session end: the launcher exits and is reaped by this test.
    drop(launcher.child.stdin.take());
    cli::wait(&mut launcher.child);
    pass_at(&mut reaper, ENDED);
    // One millisecond short of the grace period nothing is requested.
    let requested = ENDED + SESSION_GRACE_MS;
    pass_at(&mut reaper, requested - 1);
    assert_eq!(probe_live(&child), ALIVE);
    let mut expected = vec![
        row("SKIPPED", "SESSION_ACTIVE", 10_000_000),
        row("RETRY_LATER", "SESSION_GRACE_PENDING", ENDED),
    ];
    assert_eq!(reaper.decisions(id), expected);

    pass_at(&mut reaper, requested);
    // The wrapper signalled its own child, reaped it and recorded the outcome.
    assert_eq!(outcome(&reaper, id), signaled(libc::SIGTERM));
    for (label, identity) in [("reaped child", &child), ("wrapper exit", wrapper)] {
        until(label, || {
            (probe(identity) == IdentityStatus::Missing).then_some(())
        });
    }
    drop(gate);
    let confirmed = requested + 1;
    pass_at(&mut reaper, confirmed);
    expected.extend([
        row("TERMINATING", "SIGTERM_REQUESTED", requested),
        row("RECLAIMED", "TERMINATION_CONFIRMED", confirmed),
    ]);
    assert_eq!(reaper.decisions(id), expected);
    assert_eq!(reaper.resource(id), proven("RECLAIMED"));
    // Every decision is audited; the last one carries the confirmed reclaim.
    let (audits, event, payload, at): (i64, String, String, i64) = reaper
        .database()
        .query_row(
            "SELECT (SELECT count(*) FROM audit_events WHERE entity_id=?1),event_type,\
             payload_redacted_json,created_at FROM audit_events WHERE entity_id=?1 \
             ORDER BY rowid DESC LIMIT 1",
            [format!("process:{}", id.as_str())],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        (audits, event.as_str(), at as u64),
        (4, "reaper.decision", confirmed)
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&payload).unwrap(),
        serde_json::json!({"job_id": id.as_str(), "state": "RECLAIMED",
            "reason": "TERMINATION_CONFIRMED", "observed_at_ms": confirmed,
            "ownership_level": "PROVEN"})
    );
    reaper.stop();
}

#[test]
fn protected_candidates_receive_no_signal() {
    let _serial = support::serial();
    let mut reaper = Reaper::start();
    let home = reaper.f.root.join("cli-home");
    fs::create_dir(&home).unwrap();
    // Unknown ownership: a real child of a client that never registers a
    // Reaper capability, as a legacy client would not.
    let unknown_control = reaper.f.root.join("unknown-child.sock");
    let unknown_listener = UnixListener::bind(&unknown_control).unwrap();
    let mut direct = Command::new(cli::binary("arc-exec-fixture"));
    isolate(&mut direct, &reaper.f, &unknown_control, &home);
    let mut unknown = runtime::OwnedChild::new(direct.spawn().unwrap());
    let (mut unknown_gate, message) = cli::accept(&unknown_listener);
    let unknown_child: ProcessIdentity =
        serde_json::from_value(message["identity"].clone()).unwrap();
    let unknown_job = protocol::id(0x5afe);
    protocol::start(&reaper.f, &protocol::register(0x5afe));
    for request in [
        StartRequest::Claim {
            job_id: unknown_job.clone(),
            observer: protocol::observer(),
        },
        StartRequest::Attach {
            job_id: unknown_job.clone(),
            observer: protocol::observer(),
            child: unknown_child.clone(),
        },
    ] {
        protocol::start(&reaper.f, &request);
    }
    // A forged registration cannot create ownership: the peer is not the named
    // observer, or this daemon's fresh native evidence disagrees.
    let evidence = arc_platform::reaper_identity::capture(&unknown_child).unwrap();
    let registration = |observer: &ProcessIdentity, inode: u64| {
        serde_json::json!({
            "job_id": unknown_job.as_str(), "observer": observer,
            "child": {"process": unknown_child, "parent_pid": observer.pid,
                "executable": evidence.executable.to_str().unwrap(),
                "executable_device": evidence.executable_device, "executable_inode": inode,
                "real_uid": evidence.real_uid, "effective_uid": evidence.effective_uid},
            "session": ProcessIdentity::capture(reaper.daemon.child.id()).unwrap(),
            "endpoint": unknown_control.to_str().unwrap(),
            "token": "0123456789abcdef0123456789abcdef",
        })
    };
    let wrong_peer = ProcessIdentity::capture(unsafe { libc::getppid() } as u32).unwrap();
    for forged in [
        registration(&wrong_peer, evidence.executable_inode),
        registration(&protocol::observer(), evidence.executable_inode ^ 1),
    ] {
        let response = protocol::call(&reaper.f, arc_ipc::reaper::OPERATION_REGISTER, forged);
        assert_eq!(
            response.error.map(|error| error.code).as_deref(),
            Some(arc_ipc::ERROR_INVALID_REQUEST)
        );
    }
    assert_eq!(reaper.resource(&unknown_job), None);
    // Active reference: a registered job whose owner session, this test
    // process, is still alive and so still holds its lease.
    let (mut wrapper, mut gate, child, job) = wrapped(&reaper.f);
    until("verified registration", || {
        (reaper.resource(&job.job_id) == proven("TRACKED")).then_some(())
    });
    let late = 10_000_000 + 10 * SESSION_GRACE_MS;
    for now_ms in [10_000_000, late] {
        pass_at(&mut reaper, now_ms);
    }
    let unknown_rows = [row("BLOCKED", "UNKNOWN_OWNERSHIP", 10_000_000)];
    assert_eq!(reaper.decisions(&unknown_job), unknown_rows);
    let mut expected = vec![row("SKIPPED", "SESSION_ACTIVE", 10_000_000)];
    assert_eq!(reaper.decisions(&job.job_id), expected);

    // Insufficient authority, without any injected fault: the daemon restarts
    // on the same database and holds no capability for the job it recorded.
    restart(&mut reaper);
    pass_at(&mut reaper, late + 1);
    expected.push(row("BLOCKED", "INSUFFICIENT_AUTHORITY", late + 1));
    // These complete lists hold no claim: nothing was requested for either.
    assert_eq!(reaper.decisions(&job.job_id), expected);
    assert_eq!(reaper.decisions(&unknown_job), unknown_rows);
    let unowned = Some(("UNKNOWN".to_owned(), "PROTECTED".to_owned()));
    assert_eq!(reaper.resource(&unknown_job), unowned);
    assert_eq!(reaper.resource(&job.job_id), proven("TRACKED"));
    // Both children are alive, and each then exits by its own code: neither
    // received a terminating signal.
    for identity in [&unknown_child, &child] {
        assert_eq!(probe_live(identity), ALIVE);
    }
    unknown_gate.write_all(b"x").unwrap();
    assert_eq!(cli::wait(&mut unknown.child).code(), Some(17));
    gate.write_all(b"x").unwrap();
    assert_eq!(cli::wait(&mut wrapper.child.child).code(), Some(17));
    wrapper.completed = true;
    assert_eq!(outcome(&reaper, &job.job_id), Outcome::Exited { code: 17 });
    reaper.stop();
}

#[test]
fn stale_recorded_identity_keeps_live_child_alive() {
    let _serial = support::serial();
    let mut reaper = Reaper::start();
    let (mut gate, job, eligible) = eligible(&mut reaper, "echo");
    let (id, child) = (&job.job_id, job.child.clone().unwrap());
    let mut expected = reaper.decisions(id);

    // Injected: the record this daemon validates names the live child's PID
    // with an earlier start time, as a record would after that PID had passed
    // to another process. Fresh native evidence disagrees: nothing is claimed.
    inject(&reaper, "recorded-identity", Some("daemon"));
    reaper.pass();
    expected.push(row("BLOCKED", "IDENTITY_MISMATCH", eligible));
    assert_eq!(reaper.decisions(id), expected);

    // Injected: only the record sent with the request is stale, so the daemon
    // validates and claims. The wrapper, holding the child's wait gate,
    // compares it with the birth evidence and refuses. Far later, the same.
    inject(&reaper, "recorded-identity", Some("request"));
    for now in [eligible + 1, eligible + 10 * SESSION_GRACE_MS] {
        pass_at(&mut reaper, now);
        expected.extend([
            row("TERMINATING", "SIGTERM_REQUESTED", now),
            row("BLOCKED", "WRAPPER_IDENTITY_MISMATCH", now),
        ]);
        assert_eq!(reaper.decisions(id), expected);
        // The claim that sent nothing is withdrawn; the child is untouched.
        assert_eq!(reaper.resource(id), proven("TRACKED"));
        assert_eq!(probe_live(&child), ALIVE);
        assert_eq!(protocol::status(&reaper.f, id).outcome, None);
    }

    // SIGTERM would have ended this child. It exits by its own code instead.
    gate.write_all(b"x").unwrap();
    assert_eq!(outcome(&reaper, id), Outcome::Exited { code: 17 });
    let done = eligible + 11 * SESSION_GRACE_MS;
    pass_at(&mut reaper, done);
    let completed = row("SKIPPED", "JOB_COMPLETED", done);
    assert_eq!(reaper.decisions(id).last(), Some(&completed));
    assert_eq!(reaper.resource(id), proven("RELEASED"));
    reaper.stop();
}

/// One condition per entry that must stop a signal for an otherwise eligible
/// child: "condition value state reason". `recorded-identity` faults are
/// injected stale or altered records (start time, PID, executable file, UID).
const CONDITIONS: [&str; 10] = [
    "child-evidence unreadable RETRY_LATER IDENTITY_INDETERMINATE",
    "recorded-identity daemon BLOCKED IDENTITY_MISMATCH",
    "session unreadable RETRY_LATER SESSION_INDETERMINATE",
    "recorded-identity pid BLOCKED IDENTITY_MISMATCH",
    "lease held SKIPPED ACTIVE_REFERENCE",
    "recorded-identity executable BLOCKED IDENTITY_MISMATCH",
    "protected set BLOCKED PROTECTED",
    "recorded-identity uid BLOCKED IDENTITY_MISMATCH",
    "foreground held BLOCKED FOREGROUND",
    "foreground unreadable RETRY_LATER FOREGROUND_INDETERMINATE",
];

/// Run one pass per failing condition, then one in which only the wrapper's
/// own check under its wait gate fails. No pass may deliver `claim`'s signal.
fn refused(reaper: &mut Reaper, job: &JobSnapshot, now: &mut u64, claim: &str) -> Vec<Row> {
    let id = &job.job_id;
    let mut rows = Vec::new();
    for condition in CONDITIONS {
        let field: Vec<&str> = condition.split(' ').collect();
        impose(reaper, id, field[0], field[1], true);
        *now += 1;
        pass_at(reaper, *now);
        impose(reaper, id, field[0], field[1], false);
        rows.push(row(field[2], field[3], *now));
    }
    inject(reaper, "recorded-identity", Some("request"));
    *now += 1;
    pass_at(reaper, *now);
    inject(reaper, "recorded-identity", None);
    rows.push(row("TERMINATING", claim, *now));
    rows.push(row("BLOCKED", "WRAPPER_IDENTITY_MISMATCH", *now));
    assert_eq!(probe_live(job.child.as_ref().unwrap()), ALIVE);
    assert_eq!(protocol::status(&reaper.f, id).outcome, None);
    rows
}

#[test]
fn revalidation_precedes_every_signal() {
    let _serial = support::serial();
    let mut reaper = Reaper::start();
    // This child ignores SIGTERM, so the escalation is reached.
    let (gate, job, mut now) = eligible(&mut reaper, "ignored-term");
    let id = &job.job_id;
    let mut rows = reaper.decisions(id);

    // Before the first signal every condition is evaluated on fresh evidence.
    rows.extend(refused(&mut reaper, &job, &mut now, "SIGTERM_REQUESTED"));
    assert_eq!(reaper.decisions(id), rows);
    now += 1;
    pass_at(&mut reaper, now);
    rows.push(row("TERMINATING", "SIGTERM_REQUESTED", now));
    // Inside the grace period nothing further is decided or requested.
    now += TERM_GRACE_MS - 1;
    pass_at(&mut reaper, now);
    assert_eq!(reaper.decisions(id), rows);
    assert_eq!(reaper.resource(id), proven("TERMINATING"));

    // Before the escalation the same conditions are all evaluated again: an
    // accepted SIGTERM carries no authority over to the SIGKILL.
    rows.extend(refused(&mut reaper, &job, &mut now, "SIGKILL_REQUESTED"));
    assert_eq!(reaper.decisions(id), rows);
    assert_eq!(reaper.resource(id), proven("TERMINATING"));
    now += 1;
    pass_at(&mut reaper, now);
    assert_eq!(outcome(&reaper, id), signaled(libc::SIGKILL));
    drop(gate);
    pass_at(&mut reaper, now + 1);
    rows.push(row("TERMINATING", "SIGKILL_REQUESTED", now));
    rows.push(row("RECLAIMED", "TERMINATION_CONFIRMED", now + 1));
    assert_eq!(reaper.decisions(id), rows);
    assert_eq!(reaper.resource(id), proven("RECLAIMED"));
    reaper.stop();
}

#[test]
fn unconfirmed_actions_are_never_recorded_as_reclaimed() {
    let _serial = support::serial();
    // Injected: the wrapper acts and answers, and the daemon never reads the
    // answer. The outcome that follows is not attributed to a confirmed action.
    let mut reaper = Reaper::start();
    let (gate, job, eligible_at) = eligible(&mut reaper, "echo");
    let mut rows = reaper.decisions(&job.job_id);
    inject(&reaper, "action-reply", Some("lost"));
    reaper.pass();
    inject(&reaper, "action-reply", None);
    assert_eq!(outcome(&reaper, &job.job_id), signaled(libc::SIGTERM));
    drop(gate);
    pass_at(&mut reaper, eligible_at + 1);
    rows.extend([
        row("TERMINATING", "SIGTERM_REQUESTED", eligible_at),
        row("RETRY_LATER", "ACTION_UNCONFIRMED", eligible_at),
        row(
            "PARTIAL_FAILURE",
            "TERMINATION_UNATTRIBUTED",
            eligible_at + 1,
        ),
    ]);
    assert_eq!(reaper.decisions(&job.job_id), rows);
    assert_eq!(reaper.resource(&job.job_id), proven("UNCONFIRMED"));
    reaper.stop();

    // Without any injected fault: an accepted SIGTERM is ignored and the
    // daemon restarts. The capability is lost, so nothing escalates.
    let mut reaper = Reaper::start();
    let (mut gate, job, eligible_at) = eligible(&mut reaper, "ignored-term");
    let (id, child) = (&job.job_id, job.child.clone().unwrap());
    let mut rows = reaper.decisions(id);
    reaper.pass();
    restart(&mut reaper);
    let late = eligible_at + 10 * SESSION_GRACE_MS;
    for now in [late, late + 1] {
        pass_at(&mut reaper, now);
    }
    rows.extend([
        row("TERMINATING", "SIGTERM_REQUESTED", eligible_at),
        row("PARTIAL_FAILURE", "TERMINATION_UNCONFIRMED", late),
    ]);
    assert_eq!(reaper.decisions(id), rows);
    assert_eq!(reaper.resource(id), proven("UNCONFIRMED"));
    assert_eq!(probe_live(&child), ALIVE);
    // Never killed: the child ends by its own code once this test lets it.
    gate.write_all(b"x").unwrap();
    assert_eq!(outcome(&reaper, id), Outcome::Exited { code: 17 });
    reaper.stop();
}
