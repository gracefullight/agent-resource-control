//! macOS process-generation checks for fixture cleanup.
use arc_core::process_identity::{probe, probe_live, IdentityStatus, ProcessIdentity};
use std::io;

// XNU's PROC_PIDT_BSDINFOWITHUNIQID (flavor 18) returns both fields from one
// process reference. Its exported ABI is described in proc_info_private.h.
// https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/proc_info_private.h
#[repr(C)]
#[derive(Clone, Copy)]
struct UniqueInfo {
    uuid: [u8; 16],
    unique_id: u64,
    parent_unique_id: u64,
    pid_version: i32,
    original_parent_version: i32,
    reserved: [u64; 2],
}

#[repr(C)]
struct CombinedInfo {
    bsd: libc::proc_bsdinfo,
    unique: UniqueInfo,
}

#[repr(C)]
struct AuditToken {
    values: [u32; 8],
}

unsafe extern "C" {
    fn proc_signal_with_audittoken(token: *const AuditToken, signal: libc::c_int) -> libc::c_int;
}

#[derive(Clone)]
pub(super) struct ObservedProcess {
    pub bsd: libc::proc_bsdinfo,
    pub unique_id: u64,
    pub pid_version: u32,
}

#[derive(Clone)]
pub(super) struct OwnedProcess {
    pub identity: ProcessIdentity,
    pub unique_id: u64,
    pub pid_version: u32,
}

pub(super) trait ProcessOps {
    fn probe(&self, identity: &ProcessIdentity) -> IdentityStatus;
    fn probe_live(&self, identity: &ProcessIdentity) -> IdentityStatus;
    fn observe(&self, pid: u32) -> io::Result<ObservedProcess>;
    fn signal(&self, process: &OwnedProcess, signal: i32) -> io::Result<()>;
    fn children(&self, pid: u32) -> io::Result<Vec<u32>>;
}

pub(super) struct NativeOps;
impl ProcessOps for NativeOps {
    fn probe(&self, identity: &ProcessIdentity) -> IdentityStatus {
        probe(identity)
    }

    fn probe_live(&self, identity: &ProcessIdentity) -> IdentityStatus {
        probe_live(identity)
    }

    fn observe(&self, pid: u32) -> io::Result<ObservedProcess> {
        // SAFETY: the repr(C) struct matches XNU's fixed-size public ABI and
        // proc_pidinfo writes no more than the supplied buffer length.
        let mut data: CombinedInfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of_val(&data) as i32;
        let written = unsafe {
            libc::proc_pidinfo(
                pid as i32,
                18,
                0,
                (&mut data as *mut CombinedInfo).cast(),
                size,
            )
        };
        if written != size {
            return Err(io::Error::last_os_error());
        }
        Ok(ObservedProcess {
            bsd: data.bsd,
            unique_id: data.unique.unique_id,
            pid_version: data.unique.pid_version as u32,
        })
    }

    fn signal(&self, process: &OwnedProcess, signal: i32) -> io::Result<()> {
        let mut token = AuditToken { values: [0; 8] };
        token.values[5] = process.identity.pid;
        token.values[7] = process.pid_version;
        // The kernel compares PID and PID version when it finds the target.
        // Unlike kill(pid), it cannot signal a replacement after PID reuse.
        let error = unsafe { proc_signal_with_audittoken(&token, signal) };
        if error == 0 {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(error))
        }
    }

    fn children(&self, pid: u32) -> io::Result<Vec<u32>> {
        super::owned_tree_cleanup::children(pid)
    }
}

pub(super) fn matches_identity(state: &libc::proc_bsdinfo, identity: &ProcessIdentity) -> bool {
    state.pbi_pid == identity.pid
        && state
            .pbi_start_tvsec
            .checked_mul(1_000_000)
            .and_then(|v| v.checked_add(state.pbi_start_tvusec))
            .is_some_and(|start| start.to_string() == identity.start_token)
}

fn check(status: IdentityStatus) -> io::Result<bool> {
    match status {
        IdentityStatus::VerifiedMatch => Ok(true),
        IdentityStatus::Missing | IdentityStatus::Mismatch => Ok(false),
        IdentityStatus::Indeterminate => Err(io::Error::other("fixture identity unavailable")),
    }
}

/// Bind the saved process identity to a kernel PID generation without ever
/// promoting a mismatch or an indeterminate observation into ownership.
pub(super) fn acquire(
    identity: &ProcessIdentity,
    ops: &impl ProcessOps,
) -> io::Result<Option<(OwnedProcess, ObservedProcess)>> {
    if !check(ops.probe(identity))? || !check(ops.probe_live(identity))? {
        return Ok(None);
    }
    let observed = match ops.observe(identity.pid) {
        Ok(observed) => observed,
        Err(error) if error.raw_os_error() == Some(libc::ESRCH) => return Ok(None),
        Err(error) => return Err(error),
    };
    if !matches_identity(&observed.bsd, identity)
        || observed.unique_id == 0
        || observed.bsd.pbi_status == libc::SZOMB
    {
        return Ok(None);
    }
    Ok(Some((
        OwnedProcess {
            identity: identity.clone(),
            unique_id: observed.unique_id,
            pid_version: observed.pid_version,
        },
        observed,
    )))
}

pub(super) fn still_stopped(process: &OwnedProcess, ops: &impl ProcessOps) -> io::Result<bool> {
    let Some((current, observed)) = acquire(&process.identity, ops)? else {
        return Ok(false);
    };
    Ok(current.unique_id == process.unique_id
        && current.pid_version == process.pid_version
        && observed.bsd.pbi_status == libc::SSTOP)
}

/// Reacquire a generation only for the same kernel unique ID. An exec may
/// change PID version, but a recycled PID cannot inherit the old unique ID.
pub(super) fn terminate(process: &OwnedProcess, ops: &impl ProcessOps) -> io::Result<()> {
    for _ in 0..8 {
        let Some((current, _)) = acquire(&process.identity, ops)? else {
            return Ok(());
        };
        if current.unique_id != process.unique_id {
            return Ok(());
        }
        match ops.signal(&current, libc::SIGKILL) {
            Ok(()) => return Ok(()),
            Err(error) if error.raw_os_error() == Some(libc::ESRCH) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other("fixture process generation kept changing"))
}
