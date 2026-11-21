//! Fresh native observations. Nothing here is cached between passes, and an
//! unreadable observation is reported as such: it never becomes eligibility.
use super::{fixture, registry::Capability};
use arc_core::process_identity::{probe, probe_live, IdentityStatus, ProcessIdentity};
use arc_ipc::reaper::ChildEvidence;
use arc_platform::reaper_identity::{self, ChildIdentity};
use std::path::PathBuf;

pub(super) fn native(evidence: &ChildEvidence) -> ChildIdentity {
    ChildIdentity {
        process: evidence.process.clone(),
        parent_pid: evidence.parent_pid,
        executable: PathBuf::from(&evidence.executable),
        executable_device: evidence.executable_device,
        executable_inode: evidence.executable_inode,
        real_uid: evidence.real_uid,
        effective_uid: evidence.effective_uid,
    }
}

/// A recorded identity whose start time no longer names the live process.
/// Used only by the injected `recorded-identity` fault.
pub(super) fn stale(evidence: &ChildEvidence) -> ChildEvidence {
    let mut stale = evidence.clone();
    // Always a different valid start time for the same PID and boot.
    let token = match stale.process.start_token.parse::<u64>() {
        Ok(token) if token > 1 => token - 1,
        _ => 2,
    };
    stale.process.start_token = token.to_string();
    stale
}

/// The record the daemon validates. Injected fault `recorded-identity` pairs
/// the live child with a record that is stale (`daemon`: another start time)
/// or names another PID, executable file or user (`pid`, `executable`, `uid`).
pub(super) fn recorded(capability: &Capability) -> ChildEvidence {
    let mut record = capability.child.clone();
    match fixture::fault("recorded-identity").as_deref() {
        Some("daemon") => return stale(&record),
        Some("pid") => record.process = capability.observer.clone(),
        Some("executable") => record.executable_inode ^= 1,
        Some("uid") => record.effective_uid ^= 1,
        _ => (),
    }
    record
}

/// The record sent to the wrapper. Injected fault `recorded-identity=request`
/// leaves daemon validation intact so the wrapper's own refusal is observed.
pub(super) fn requested(capability: &Capability) -> ChildEvidence {
    match fixture::fault("recorded-identity").as_deref() {
        Some("request") => stale(&capability.child),
        _ => recorded(capability),
    }
}

pub(super) enum Child {
    Matches,
    Missing,
    Mismatch,
    Indeterminate,
}

/// PID, start time, boot, direct parent, executable path and file identity,
/// and both UIDs, all read fresh and compared with the record.
pub(super) fn child(recorded: &ChildEvidence) -> Child {
    if fixture::fault("child-evidence").is_some() {
        return Child::Indeterminate;
    }
    let expected = native(recorded);
    if expected.real_uid != unsafe { libc::getuid() }
        || expected.effective_uid != unsafe { libc::geteuid() }
    {
        return Child::Mismatch;
    }
    match reaper_identity::capture(&expected.process) {
        Ok(current) if current == expected => Child::Matches,
        Ok(_) => Child::Mismatch,
        Err(_) => match probe(&expected.process) {
            IdentityStatus::Missing => Child::Missing,
            IdentityStatus::Mismatch => Child::Mismatch,
            // A retained zombie or an unreadable process proves nothing.
            _ => Child::Indeterminate,
        },
    }
}

pub(super) fn live(identity: &ProcessIdentity) -> IdentityStatus {
    probe_live(identity)
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Session {
    Active,
    Ended,
    Indeterminate,
}

/// The owner session has ended only when its recorded process is observed
/// gone and the wrapper is observed reparented away from it.
pub(super) fn session(capability: &Capability) -> Session {
    if fixture::fault("session").is_some() {
        return Session::Indeterminate;
    }
    match probe(&capability.session) {
        IdentityStatus::VerifiedMatch => Session::Active,
        IdentityStatus::Indeterminate => Session::Indeterminate,
        IdentityStatus::Missing | IdentityStatus::Mismatch => {
            match reaper_identity::capture(&capability.observer) {
                Ok(observer) if observer.parent_pid != capability.session.pid => Session::Ended,
                _ => Session::Indeterminate,
            }
        }
    }
}

/// Injected fault `foreground`: `held` reports a terminal foreground, any
/// other value an unreadable terminal state.
pub(super) fn foreground(process: &ProcessIdentity) -> std::io::Result<bool> {
    match fixture::fault("foreground").as_deref() {
        Some("held") => Ok(true),
        Some(_) => Err(std::io::Error::other("injected terminal state")),
        None => reaper_identity::foreground(process),
    }
}
