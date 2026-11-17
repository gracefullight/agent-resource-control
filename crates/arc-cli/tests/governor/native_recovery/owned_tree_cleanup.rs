//! Freeze and terminate only verified fixture-owned process identities.
use super::owned_tree_process::{self, NativeOps, OwnedProcess, ProcessOps};
use arc_core::process_identity::{probe, IdentityStatus, ProcessIdentity};
use std::{
    fs, io,
    path::Path,
    process::{Child, ExitStatus},
    thread,
    time::{Duration, Instant},
};

pub(in super::super) fn info(pid: u32) -> Option<libc::proc_bsdinfo> {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of_val(&info) as i32;
    let written = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    };
    (written == size).then_some(info)
}

pub(super) fn confirmed_missing(pid: u32) -> bool {
    (unsafe { libc::kill(pid as i32, 0) }) != 0
        && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
}

pub(super) fn children(pid: u32) -> io::Result<Vec<u32>> {
    let mut capacity = 32;
    loop {
        let mut pids = vec![0i32; capacity];
        let count = unsafe {
            libc::proc_listchildpids(
                pid as i32,
                pids.as_mut_ptr().cast(),
                std::mem::size_of_val(pids.as_slice()) as i32,
            )
        };
        if count < 0 {
            return Err(io::Error::last_os_error());
        }
        if count as usize >= capacity {
            capacity *= 2;
            if capacity > 8192 {
                return Err(io::Error::other("fixture descendant limit exceeded"));
            }
            continue;
        }
        return Ok(pids[..count as usize]
            .iter()
            .copied()
            .filter(|p| *p > 0)
            .map(|p| p as u32)
            .collect());
    }
}

// A private session is created before the root can fork. Unlike ancestry,
// session membership survives reparenting and process-group changes. Freeze
// every member and repeat until no live member can fork another child.
pub(super) fn freeze_session(
    anchor: &Child,
    leader: Option<&ProcessIdentity>,
    owned: &mut Vec<OwnedProcess>,
    capture: &impl Fn(u32) -> io::Result<ProcessIdentity>,
    read_info: &impl Fn(u32) -> Option<libc::proc_bsdinfo>,
) -> io::Result<()> {
    // The caller holds the unreaped direct Child for this whole enumeration.
    // Borrowing it here prevents this code from reaping and reusing its PID.
    let session = anchor.id();
    if leader.is_some_and(|leader| leader.pid != session) {
        return Err(io::Error::other("fixture session anchor PID changed"));
    }
    for _ in 0..128 {
        if let Some(leader) = leader {
            match probe(leader) {
                IdentityStatus::VerifiedMatch => (),
                _ => {
                    return Err(io::Error::other(
                        "fixture session leader identity changed or unavailable",
                    ))
                }
            }
        }
        let mut capacity = 1024;
        let pids = loop {
            let mut pids = vec![0i32; capacity];
            let count = unsafe {
                libc::proc_listallpids(
                    pids.as_mut_ptr().cast(),
                    std::mem::size_of_val(pids.as_slice()) as i32,
                )
            };
            if count < 0 {
                return Err(io::Error::last_os_error());
            }
            if count as usize >= capacity {
                capacity *= 2;
                if capacity > 131_072 {
                    return Err(io::Error::other("fixture process inventory limit exceeded"));
                }
                continue;
            }
            pids.truncate(count as usize);
            break pids;
        };
        let mut added = false;
        for pid in pids.into_iter().filter(|pid| *pid > 0) {
            if unsafe { libc::getsid(pid) } != session as i32 {
                continue;
            }
            let identity = match capture(pid as u32) {
                Ok(identity) => identity,
                Err(_) if confirmed_missing(pid as u32) => continue,
                Err(error) => return Err(io::Error::other(format!("session pid {pid}: {error}"))),
            };
            if unsafe { libc::getsid(pid) } != session as i32 {
                continue;
            }
            if !owned.iter().any(|item| item.identity == identity) {
                added = true;
                freeze_tree_with(identity, owned, capture, read_info)?;
            }
        }
        if !added {
            return Ok(());
        }
    }
    Err(io::Error::other("fixture session did not quiesce"))
}

// Stop each live parent before discovering its children: after SIGSTOP is
// observed it cannot fork between enumeration and ownership of its children.
fn freeze_tree(identity: ProcessIdentity, owned: &mut Vec<OwnedProcess>) -> io::Result<()> {
    freeze_tree_with(identity, owned, &ProcessIdentity::capture, &info)
}

pub(super) fn freeze_tree_with(
    identity: ProcessIdentity,
    owned: &mut Vec<OwnedProcess>,
    capture: &impl Fn(u32) -> io::Result<ProcessIdentity>,
    read_info: &impl Fn(u32) -> Option<libc::proc_bsdinfo>,
) -> io::Result<()> {
    freeze_tree_with_ops(identity, owned, capture, read_info, &NativeOps)
}

fn freeze_tree_with_ops(
    identity: ProcessIdentity,
    owned: &mut Vec<OwnedProcess>,
    capture: &impl Fn(u32) -> io::Result<ProcessIdentity>,
    read_info: &impl Fn(u32) -> Option<libc::proc_bsdinfo>,
    ops: &impl ProcessOps,
) -> io::Result<()> {
    if owned.iter().any(|item| item.identity == identity) {
        return Ok(());
    }

    // An exec can change PID version between the observation and signal.
    // Retry only if the kernel unique ID still denotes the same process.
    let mut unique_id = None;
    let mut frozen = None;
    for _ in 0..8 {
        let Some((current, _)) = owned_tree_process::acquire(&identity, ops)? else {
            return Ok(());
        };
        if unique_id.is_some_and(|old| old != current.unique_id) {
            return Ok(());
        }
        unique_id = Some(current.unique_id);
        match ops.signal(&current, libc::SIGSTOP) {
            Ok(()) => {
                owned.push(current.clone());
                frozen = Some(current);
                break;
            }
            Err(error) if error.raw_os_error() == Some(libc::ESRCH) => continue,
            Err(error) => return Err(error),
        }
    }
    let process =
        frozen.ok_or_else(|| io::Error::other("fixture process generation kept changing"))?;
    let watchdog = Instant::now() + Duration::from_secs(60);
    loop {
        match read_info(identity.pid) {
            Some(state) if !owned_tree_process::matches_identity(&state, &identity) => {
                return Ok(())
            }
            Some(state) if state.pbi_status == libc::SZOMB => return Ok(()),
            Some(state) if state.pbi_status == libc::SSTOP => {
                if !owned_tree_process::still_stopped(&process, ops)? {
                    return Ok(());
                }
                break;
            }
            None => {
                let Some((current, _)) = owned_tree_process::acquire(&identity, ops)? else {
                    return Ok(());
                };
                if current.unique_id != process.unique_id
                    || current.pid_version != process.pid_version
                {
                    return Ok(());
                }
            }
            _ => (),
        }
        // The signal was sent to the verified generation. Wait for the
        // kernel's stopped state, rather than treating scheduler delay as a
        // failure or enumerating an unfrozen process tree.
        if Instant::now() >= watchdog {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("fixture process {} never stopped", identity.pid),
            ));
        }
        thread::yield_now();
    }
    let pids = ops.children(identity.pid)?;
    // Discard a PID-only child listing if its parent exited or was replaced
    // during enumeration; never use it to acquire or signal descendants.
    if !owned_tree_process::still_stopped(&process, ops)? {
        return Ok(());
    }
    for pid in pids {
        let Some(child_info) = read_info(pid) else {
            if confirmed_missing(pid) {
                continue;
            }
            return Err(io::Error::other("fixture child identity unavailable"));
        };
        if child_info.pbi_ppid != identity.pid {
            continue;
        }
        match capture(pid) {
            Ok(child) => {
                let observed_start = child_info
                    .pbi_start_tvsec
                    .checked_mul(1_000_000)
                    .and_then(|v| v.checked_add(child_info.pbi_start_tvusec));
                if child.pid != pid
                    || observed_start.is_none_or(|v| v.to_string() != child.start_token)
                {
                    return Err(io::Error::other(format!(
                        "fixture child {pid} changed during identity capture"
                    )));
                }
                if !owned_tree_process::still_stopped(&process, ops)? {
                    return Ok(());
                }
                freeze_tree_with_ops(child, owned, capture, read_info, ops)?;
            }
            Err(_) if confirmed_missing(pid) => (),
            Err(error) => {
                return Err(io::Error::other(format!(
                    "fixture child {pid} identity unavailable: {error}"
                )))
            }
        }
    }
    Ok(())
}

pub(super) fn terminate(
    child: &mut Child,
    status: &mut Option<ExitStatus>,
    owned: &[OwnedProcess],
) -> (Vec<ProcessIdentity>, Option<String>) {
    // Every enumerated parent stays stopped until discovery completes.
    let mut signal_errors = Vec::new();
    for process in owned.iter().rev() {
        if let Err(error) = owned_tree_process::terminate(process, &NativeOps) {
            signal_errors.push((process.identity.clone(), error));
        }
    }
    let mut root_error = None;
    if status.is_none() {
        root_error = child
            .kill()
            .err()
            .filter(|error| error.raw_os_error() != Some(libc::ESRCH))
            .map(|error| format!("cannot kill fixture root: {error}"));
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if status.is_none() {
            match child.try_wait() {
                Ok(Some(exit)) => *status = Some(exit),
                Ok(None) => (),
                Err(error) => root_error = Some(format!("cannot reap fixture root: {error}")),
            }
        }
        let remaining: Vec<_> = owned
            .iter()
            .filter(|process| {
                let identity = &process.identity;
                !matches!(
                    probe(identity),
                    IdentityStatus::Missing | IdentityStatus::Mismatch
                ) && !confirmed_missing(identity.pid)
            })
            .map(|process| process.identity.clone())
            .collect();
        if (remaining.is_empty() && status.is_some()) || Instant::now() >= deadline {
            if status.is_none() && root_error.is_none() {
                root_error = Some("fixture root did not exit before cleanup deadline".into());
            }
            let mut errors: Vec<_> = signal_errors
                .into_iter()
                .filter(|(identity, _)| {
                    !matches!(
                        probe(identity),
                        IdentityStatus::Missing | IdentityStatus::Mismatch
                    ) && !confirmed_missing(identity.pid)
                })
                .map(|(identity, error)| {
                    format!("cannot terminate fixture pid {}: {error}", identity.pid)
                })
                .collect();
            errors.extend(root_error);
            return (remaining, (!errors.is_empty()).then(|| errors.join("; ")));
        }
        thread::sleep(Duration::from_millis(5));
    }
}

pub(super) fn report_cleanup(
    fixture: &Path,
    errors: &[String],
    remaining: &[ProcessIdentity],
) -> io::Result<()> {
    if errors.is_empty() && remaining.is_empty() {
        return Ok(());
    }
    let message = format!("errors={errors:?}; remaining={remaining:?}");
    let _ = fs::write(fixture.join("owned-cleanup-incomplete"), &message);
    eprintln!("fixture tree cleanup incomplete: {message}");
    Err(io::Error::other(message))
}

pub(super) fn cleanup_uncaptured_root(
    child: &mut Child,
    fixture: &Path,
    capture: &impl Fn(u32) -> io::Result<ProcessIdentity>,
) -> io::Result<()> {
    let mut owned = Vec::new();
    let mut errors = Vec::new();
    // Retry independently. If identity remains unavailable, the unreaped
    // Child handle still owns this PID; stop it and discover its children.
    let frozen = match capture(child.id()) {
        Ok(root) => freeze_tree(root, &mut owned),
        Err(_) => freeze_uncaptured_root(child, &mut owned),
    };
    if let Err(error) = frozen {
        errors.push(error.to_string());
    }
    // The direct Child is still unreaped here, so its PID cannot be reused.
    if let Err(error) = freeze_session(child, None, &mut owned, &ProcessIdentity::capture, &info) {
        errors.push(error.to_string());
    }
    // This group was created for the unreaped direct child. It covers members
    // not yet enumerated; separately grouped children still require identities.
    unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
    let mut status = None;
    let (remaining, root_error) = terminate(child, &mut status, &owned);
    errors.extend(root_error);
    report_cleanup(fixture, &errors, &remaining)
}

fn freeze_uncaptured_root(anchor: &Child, owned: &mut Vec<OwnedProcess>) -> io::Result<()> {
    // The borrowed direct child remains unreaped throughout this function,
    // so the kernel cannot recycle its PID between this signal and discovery.
    let root = anchor.id();
    if unsafe { libc::kill(root as i32, libc::SIGSTOP) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let watchdog = Instant::now() + Duration::from_secs(60);
    loop {
        if Instant::now() >= watchdog {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("unidentified fixture root {root} never reported stop or exit"),
            ));
        }
        let mut event: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // The unreaped Child is ours; only its observed stop or exit can
        // complete this wait. The deadline diagnoses a hung fixture.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                root,
                &mut event,
                libc::WSTOPPED | libc::WEXITED | libc::WNOWAIT | libc::WNOHANG,
            )
        };
        if result != 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if event.si_pid == 0 {
            thread::yield_now();
            continue;
        }
        if event.si_pid != root as libc::pid_t || event.si_code != libc::CLD_STOPPED {
            return Err(io::Error::other(
                "unidentified root exited before discovery",
            ));
        }
        match info(root) {
            Some(state) if state.pbi_status == libc::SSTOP => break,
            _ => return Err(io::Error::other("unidentified root is no longer stopped")),
        }
    }
    for pid in children(root)? {
        if confirmed_missing(pid) {
            continue;
        }
        let before = info(pid).ok_or_else(|| io::Error::other("child information unavailable"))?;
        if before.pbi_ppid != root {
            continue;
        }
        let identity = ProcessIdentity::capture(pid)?;
        let start = before
            .pbi_start_tvsec
            .checked_mul(1_000_000)
            .and_then(|v| v.checked_add(before.pbi_start_tvusec));
        if start.is_none_or(|v| v.to_string() != identity.start_token) {
            return Err(io::Error::other("child identity changed during capture"));
        }
        freeze_tree(identity, owned)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "owned_tree_cleanup_tests.rs"]
mod regression_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, os::unix::process::CommandExt, process::Command};

    #[test]
    fn reused_session_id_never_enumerates_or_signals_replacement_session() {
        let mut command = Command::new("/bin/sleep");
        command.arg("300");
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = crate::native::OwnedChild::new(command.spawn().unwrap());
        let replacement = ProcessIdentity::capture(child.child.id()).unwrap();
        let mut original = replacement.clone();
        original.start_token = (replacement.start_token.parse::<u64>().unwrap() - 1).to_string();
        let captures = Cell::new(0);
        let mut owned = Vec::new();
        let result = freeze_session(
            &child.child,
            Some(&original),
            &mut owned,
            &|pid| {
                captures.set(captures.get() + 1);
                ProcessIdentity::capture(pid)
            },
            &info,
        );
        assert!(result.is_err());
        assert_eq!(captures.get(), 0, "a reused session must not be enumerated");
        assert!(owned.is_empty());
        assert_eq!(probe(&replacement), IdentityStatus::VerifiedMatch);
    }
}
