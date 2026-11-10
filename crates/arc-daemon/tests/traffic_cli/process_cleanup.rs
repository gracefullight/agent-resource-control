use arc_core::process_identity::ProcessIdentity;
use std::{cell::RefCell, io};

thread_local! {
    static UNWIND_ERRORS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

// A failed identity read is not proof that a process has disappeared. Probe
// existence independently, and retain the original error if it still exists or
// the existence check is also unavailable.
pub(super) fn capture_with(
    pid: u32,
    capture: impl FnOnce(u32) -> io::Result<ProcessIdentity>,
    disappeared: impl FnOnce(u32) -> io::Result<bool>,
) -> io::Result<Option<ProcessIdentity>> {
    match capture(pid) {
        Ok(identity) => Ok(Some(identity)),
        Err(error) => match disappeared(pid) {
            Ok(true) => Ok(None),
            Ok(false) => Err(io::Error::new(
                error.kind(),
                format!("cannot capture identity for pid {pid}: {error}"),
            )),
            Err(probe_error) => Err(io::Error::new(
                error.kind(),
                format!(
                    "cannot capture identity for pid {pid}: {error}; disappearance probe: {probe_error}"
                ),
            )),
        },
    }
}

fn disappeared(pid: u32) -> io::Result<bool> {
    // SAFETY: signal zero observes existence without sending a signal.
    if unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 {
        return Ok(false);
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(true)
    } else {
        Err(error)
    }
}

pub(super) fn capture(pid: u32) -> io::Result<Option<ProcessIdentity>> {
    capture_with(pid, ProcessIdentity::capture, disappeared)
}

pub(super) fn children_with(
    parent: u32,
    enumerate: impl FnOnce(u32) -> io::Result<Vec<u32>>,
) -> io::Result<Vec<u32>> {
    enumerate(parent).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("cannot enumerate descendants of pid {parent}: {error}"),
        )
    })
}

pub(super) fn children(parent: u32) -> io::Result<Vec<u32>> {
    children_with(parent, |parent| {
        let mut ids = vec![0i32; 128];
        loop {
            // Darwin's proc_listpids returns zero for an empty result as well
            // as on failure, so clear errno before observing both values.
            let bytes = unsafe {
                *libc::__error() = 0;
                libc::proc_listpids(
                    6,
                    parent,
                    ids.as_mut_ptr().cast(),
                    std::mem::size_of_val(ids.as_slice()) as i32,
                )
            };
            let error = io::Error::last_os_error();
            if bytes < 0 || (bytes == 0 && error.raw_os_error() != Some(0)) {
                return Err(error);
            }
            let count = bytes as usize / std::mem::size_of::<i32>();
            if count < ids.len() {
                return Ok(ids[..count]
                    .iter()
                    .copied()
                    .filter(|pid| *pid > 0)
                    .map(|pid| pid as u32)
                    .collect());
            }
            if ids.len() >= 16_384 {
                return Err(io::Error::other("descendant list exceeds fixture limit"));
            }
            ids.resize(ids.len() * 2, 0);
        }
    })
}

fn child_identity(parent: u32, pid: u32) -> io::Result<Option<ProcessIdentity>> {
    let Some(identity) = capture(pid)? else {
        return Ok(None);
    };
    // SAFETY: proc_bsdinfo is an initialized output buffer of the required size.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    let written = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    };
    if written != size {
        let error = io::Error::last_os_error();
        return match disappeared(pid) {
            Ok(true) => Ok(None),
            Ok(false) => Err(io::Error::new(
                error.kind(),
                format!("cannot verify parent of pid {pid}: {error}"),
            )),
            Err(probe_error) => Err(io::Error::new(
                error.kind(),
                format!("cannot verify parent of pid {pid}: {error}; disappearance probe: {probe_error}"),
            )),
        };
    }
    if info.pbi_ppid != parent {
        return Err(io::Error::other(format!(
            "pid {pid} is no longer a child of pid {parent}"
        )));
    }
    match capture(pid)? {
        Some(current) if current == identity => Ok(Some(identity)),
        _ => Ok(None),
    }
}

// Continue through all known children even if one observation is unavailable.
// Only signal a PID after its current identity matches the recorded identity.
pub(super) fn kill_tree(identity: &ProcessIdentity, depth: usize) -> Vec<String> {
    if depth >= 128 {
        return vec![format!(
            "descendant depth limit reached for pid {}",
            identity.pid
        )];
    }
    let mut errors = Vec::new();
    match capture(identity.pid) {
        Ok(Some(current)) if current == *identity => {}
        Ok(_) => return errors,
        Err(error) => return vec![error.to_string()],
    }
    match children(identity.pid) {
        Ok(children) => {
            for pid in children {
                match child_identity(identity.pid, pid) {
                    Ok(Some(child)) => errors.extend(kill_tree(&child, depth + 1)),
                    Ok(None) => {}
                    Err(error) => errors.push(error.to_string()),
                }
            }
        }
        Err(error) => errors.push(error.to_string()),
    }
    match capture(identity.pid) {
        Ok(Some(current)) if current == *identity => {
            // SAFETY: the current native identity matches this test-owned PID.
            if unsafe { libc::kill(identity.pid as libc::pid_t, libc::SIGKILL) } != 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    errors.push(format!("cannot terminate pid {}: {error}", identity.pid));
                }
            }
        }
        Ok(_) => {}
        Err(error) => errors.push(error.to_string()),
    }
    errors
}

pub(super) fn report_errors(errors: Vec<String>) {
    if errors.is_empty() {
        return;
    }
    let message = format!("fixture cleanup observation failed: {}", errors.join("; "));
    if std::thread::panicking() {
        use std::io::Write;
        let _ = writeln!(io::stderr().lock(), "{message}");
        UNWIND_ERRORS.with(|errors| errors.borrow_mut().push(message));
    } else {
        panic!("{message}");
    }
}

pub(super) fn take_unwind_errors() -> Vec<String> {
    UNWIND_ERRORS.with(|errors| std::mem::take(&mut *errors.borrow_mut()))
}
