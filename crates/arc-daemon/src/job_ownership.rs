//! Lifecycle mutations require native peer and managed-child evidence.
use arc_core::process_identity::{probe, probe_live, IdentityStatus, ProcessIdentity};
use arc_ipc::jobs::{LifecycleRequest, StartRequest};
use std::{io, os::fd::AsRawFd};

pub(super) fn authorize(
    request: &LifecycleRequest,
    direct: &impl AsRawFd,
    request_bound_to_observer: bool,
) -> Result<(), &'static str> {
    let Some(observer) = observer(request) else {
        return Ok(());
    };
    if (crate::platform::peer_pid(direct).ok() != Some(observer.pid) && !request_bound_to_observer)
        || probe_live(observer) != IdentityStatus::VerifiedMatch
        || !current_user(observer)
    {
        return Err(arc_ipc::ERROR_INVALID_REQUEST);
    }
    if let LifecycleRequest::Start(StartRequest::Attach { child, .. }) = request {
        // Unavailable/stale child evidence remains an unknown lifecycle
        // observation, never an ownership proof. A matching live or retained
        // zombie identity must belong to this observer's process tree.
        if probe(child) == IdentityStatus::VerifiedMatch
            && (!current_user(child) || !owned_descendant(child, observer) || !current_user(child))
        {
            return Err(arc_ipc::ERROR_INVALID_REQUEST);
        }
    }
    if probe_live(observer) != IdentityStatus::VerifiedMatch || !current_user(observer) {
        return Err(arc_ipc::ERROR_INVALID_REQUEST);
    }
    Ok(())
}

pub(super) fn observer(request: &LifecycleRequest) -> Option<&ProcessIdentity> {
    match request {
        LifecycleRequest::Start(request) => Some(request.observer()),
        LifecycleRequest::Complete(request) => Some(&request.observer),
        LifecycleRequest::Status(_) => None,
    }
}

/// A passed socket proves its peer, not which bytes a relay forwarded. For a
/// relayed mutation the live observer confirms the entire request over a
/// separate callback whose server PID is authenticated by the kernel.
pub(super) async fn confirm_request(
    request: &arc_ipc::Request,
    observer: &ProcessIdentity,
    deadline: tokio::time::Instant,
) -> bool {
    let Some((path, nonce)) = request
        .request_id
        .strip_prefix("arc-proof:")
        .and_then(|s| s.rsplit_once('#'))
    else {
        return false;
    };
    if path.is_empty()
        || path.len() > 100
        || !path.starts_with('/')
        || path.contains('\0')
        || nonce.len() != 32
        || !nonce.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return false;
    }
    let Ok(Ok(stream)) =
        tokio::time::timeout_at(deadline, tokio::net::UnixStream::connect(path)).await
    else {
        return false;
    };
    if crate::platform::ensure_same_user(&stream).is_err()
        || crate::platform::peer_pid(&stream).ok() != Some(observer.pid)
        || probe_live(observer) != IdentityStatus::VerifiedMatch
        || !current_user(observer)
    {
        return false;
    }
    crate::transport::confirm_request(&stream, request, deadline).await
        && probe_live(observer) == IdentityStatus::VerifiedMatch
        && current_user(observer)
}

fn current_user(identity: &ProcessIdentity) -> bool {
    if probe(identity) != IdentityStatus::VerifiedMatch {
        return false;
    }
    let Ok((real, effective)) = user_ids(identity.pid) else {
        return false;
    };
    // Native UID reads must still refer to the same captured identity.
    real == unsafe { libc::getuid() }
        && effective == unsafe { libc::geteuid() }
        && probe(identity) == IdentityStatus::VerifiedMatch
}

/// Recheck ownership for the exact tuple used by a subsequent native probe.
/// An earlier indeterminate probe is never itself ownership evidence.
pub(crate) fn managed_child(child: &ProcessIdentity, observer: &ProcessIdentity) -> bool {
    current_user(observer)
        && current_user(child)
        && owned_descendant(child, observer)
        && current_user(child)
        && current_user(observer)
}

fn user_ids(pid: u32) -> io::Result<(libc::uid_t, libc::uid_t)> {
    #[cfg(target_os = "macos")]
    {
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of_val(&info) as libc::c_int;
        if unsafe {
            libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdinfo).cast(),
                size,
            )
        } == size
            && info.pbi_pid == pid
        {
            return Ok((info.pbi_ruid, info.pbi_uid));
        }
        unsafe extern "C" {
            fn arc_unreaped_user_ids(
                pid: i32,
                real_uid: *mut libc::uid_t,
                effective_uid: *mut libc::uid_t,
            ) -> i32;
        }
        let (mut real, mut effective) = (0, 0);
        if unsafe { arc_unreaped_user_ids(pid as i32, &mut real, &mut effective) } == 0 {
            return Ok((real, effective));
        }
        Err(io::Error::other("process UID unavailable"))
    }
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string(format!("/proc/{pid}/status"))?;
        let ids = status
            .lines()
            .find_map(|line| line.strip_prefix("Uid:"))
            .ok_or_else(|| io::Error::other("process UID unavailable"))?;
        let mut values = ids.split_whitespace();
        let real = values.next().and_then(|value| value.parse().ok());
        let effective = values.next().and_then(|value| value.parse().ok());
        real.zip(effective)
            .ok_or_else(|| io::Error::other("invalid process UID"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = pid;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "process UID unavailable",
        ))
    }
}

fn owned_descendant(child: &ProcessIdentity, observer: &ProcessIdentity) -> bool {
    if child == observer || probe(child) != IdentityStatus::VerifiedMatch {
        return false;
    }
    let mut current = child.clone();
    for _ in 0..128 {
        let Ok(parent) = parent_pid(current.pid) else {
            return false;
        };
        if parent == 0 || parent == current.pid || probe(&current) != IdentityStatus::VerifiedMatch
        {
            return false;
        }
        if parent == observer.pid {
            return probe_live(observer) == IdentityStatus::VerifiedMatch
                && parent_pid(current.pid).ok() == Some(parent)
                && probe(&current) == IdentityStatus::VerifiedMatch;
        }
        let Ok(identity) = ProcessIdentity::capture(parent) else {
            return false;
        };
        if probe_live(&identity) != IdentityStatus::VerifiedMatch
            || parent_pid(current.pid).ok() != Some(parent)
            || probe(&current) != IdentityStatus::VerifiedMatch
        {
            return false;
        }
        current = identity;
    }
    false
}

fn parent_pid(pid: u32) -> io::Result<u32> {
    #[cfg(target_os = "macos")]
    {
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of_val(&info) as libc::c_int;
        if unsafe {
            libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdinfo).cast(),
                size,
            )
        } == size
        {
            return Ok(info.pbi_ppid);
        }
        unsafe extern "C" {
            fn arc_unreaped_parent_pid(pid: i32, parent: *mut i32) -> i32;
        }
        let mut parent = 0;
        if unsafe { arc_unreaped_parent_pid(pid as i32, &mut parent) } == 0 && parent > 0 {
            return Ok(parent as u32);
        }
        Err(io::Error::other("parent identity unavailable"))
    }
    #[cfg(target_os = "linux")]
    {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
        let end = stat
            .rfind(')')
            .ok_or_else(|| io::Error::other("invalid process stat"))?;
        stat[end + 1..]
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| io::Error::other("parent identity unavailable"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = pid;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "parent identity unavailable",
        ))
    }
}
