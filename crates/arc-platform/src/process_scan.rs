//! Enumeration of the current user's processes with their CPU time (lane A:
//! MTR-007, MET-027).
//!
//! One bounded native pass: `proc_listpids(PROC_UID_ONLY, euid)` names the
//! effective user's processes; for each, `proc_pidinfo` BSD info (parent,
//! process group, start time, effective uid and OS process name) and
//! `proc_pid_rusage` (own user plus system CPU time) are read, and the BSD
//! info is read again so a reading belongs to one unchanged start time.
//! Read on the Meter thread only; nothing read here is logged or stored
//! except through aggregates. Read-only: no signal, no subprocess.
use arc_core::process_identity::ProcessIdentity;

/// At most this many processes are read in one scan.
pub const MAX_SCAN_PROCESSES: usize = 8192;

/// One native reading of one process of the current user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedProcess {
    pub identity: ProcessIdentity,
    pub parent_pid: u32,
    pub process_group: u32,
    /// The OS process name (`p_name`, else `p_comm`): the executable's base
    /// name as the kernel recorded it at exec. Kept in memory for one scan.
    pub name: String,
    /// The process's own user plus system CPU time.
    pub cpu_time_ns: u64,
}
impl ScannedProcess {
    /// The start time in microseconds since the epoch.
    pub fn start_us(&self) -> Option<u64> {
        self.identity.start_token.parse().ok()
    }
}

/// One scan. `unreadable` counts listed processes that could not be read
/// (exited, a zombie, denied, another user, or over the bound).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scan {
    pub processes: Vec<ScannedProcess>,
    pub unreadable: u64,
}

/// The current user's processes. An empty scan means nothing could be read.
pub fn scan() -> Scan {
    #[cfg(target_os = "macos")]
    {
        native::scan()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Scan::default()
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::{Scan, ScannedProcess, MAX_SCAN_PROCESSES};
    use arc_core::process_identity::{boot_id, ProcessIdentity};
    use std::{ffi::CStr, mem, ptr, sync::OnceLock};

    /// `PROC_UID_ONLY` of `<sys/proc_info.h>`; libc does not declare it.
    const PROC_UID_ONLY: u32 = 4;

    struct Bsd {
        parent: u32,
        group: u32,
        start: u64,
        uid: u32,
        name: String,
    }

    pub(super) fn scan() -> Scan {
        let Ok(boot) = boot_id() else {
            return Scan::default();
        };
        // SAFETY: geteuid has no preconditions.
        let uid = unsafe { libc::geteuid() };
        let (pids, over) = list(uid);
        let mut scan = Scan {
            processes: Vec::with_capacity(pids.len()),
            unreadable: over,
        };
        for pid in pids {
            match read(pid, uid, &boot) {
                Some(process) => scan.processes.push(process),
                None => scan.unreadable += 1,
            }
        }
        scan
    }

    fn read(pid: u32, uid: u32, boot: &str) -> Option<ScannedProcess> {
        let info = bsd(pid).filter(|info| info.uid == uid)?;
        let usage = rusage(pid)?;
        // The CPU time belongs to this start time only if it is unchanged.
        bsd(pid).filter(|again| {
            again.start == info.start
                && again.uid == uid
                && again.parent == info.parent
                && again.name == info.name
        })?;
        Some(ScannedProcess {
            identity: ProcessIdentity {
                pid,
                start_token: info.start.to_string(),
                boot_id: boot.to_owned(),
            },
            parent_pid: info.parent,
            process_group: info.group,
            name: info.name,
            cpu_time_ns: ticks_to_ns(usage.ri_user_time)
                .saturating_add(ticks_to_ns(usage.ri_system_time)),
        })
    }

    /// The effective user's PIDs, at most the bound, and how many were over it.
    fn list(uid: u32) -> (Vec<u32>, u64) {
        // SAFETY: a null buffer asks only for the required size in bytes.
        let needed = unsafe { libc::proc_listpids(PROC_UID_ONLY, uid, ptr::null_mut(), 0) };
        let Ok(needed) = usize::try_from(needed) else {
            return (Vec::new(), 0);
        };
        let wanted = needed / mem::size_of::<libc::pid_t>();
        // Headroom for processes started between the two calls.
        let capacity = (wanted + 64).min(MAX_SCAN_PROCESSES);
        let mut pids = vec![0 as libc::pid_t; capacity];
        let bytes = mem::size_of_val(pids.as_slice()) as libc::c_int;
        // SAFETY: the buffer is writable for exactly `bytes` bytes.
        let written =
            unsafe { libc::proc_listpids(PROC_UID_ONLY, uid, pids.as_mut_ptr().cast(), bytes) };
        let Ok(written) = usize::try_from(written) else {
            return (Vec::new(), 0);
        };
        let entries = written / mem::size_of::<libc::pid_t>();
        pids.truncate(entries.min(capacity));
        // A full bounded buffer can also mean the process list grew between
        // the size and data calls. Treat that scan as incomplete.
        let over = (wanted.saturating_sub(MAX_SCAN_PROCESSES) as u64).max(u64::from(
            capacity == MAX_SCAN_PROCESSES && entries >= capacity,
        ));
        let pids = pids
            .into_iter()
            .filter_map(|pid| u32::try_from(pid).ok().filter(|pid| *pid > 0))
            .collect();
        (pids, over)
    }

    fn bsd(pid: u32) -> Option<Bsd> {
        if pid == 0 || pid > i32::MAX as u32 {
            return None;
        }
        // SAFETY: a zeroed proc_bsdinfo is a valid output buffer of exact size.
        let mut info: libc::proc_bsdinfo = unsafe { mem::zeroed() };
        let size = mem::size_of_val(&info) as libc::c_int;
        let written = unsafe {
            libc::proc_pidinfo(
                pid as libc::c_int,
                libc::PROC_PIDTBSDINFO,
                0,
                ptr::from_mut(&mut info).cast(),
                size,
            )
        };
        if written != size || info.pbi_pid != pid {
            return None;
        }
        let start = info
            .pbi_start_tvsec
            .checked_mul(1_000_000)?
            .checked_add(info.pbi_start_tvusec)
            .filter(|v| *v > 0)?;
        let name = text(&info.pbi_name).filter(|n| !n.is_empty());
        Some(Bsd {
            parent: info.pbi_ppid,
            group: info.pbi_pgid,
            start,
            uid: info.pbi_uid,
            name: name.or_else(|| text(&info.pbi_comm)).unwrap_or_default(),
        })
    }

    /// A NUL-terminated fixed-size name; `None` without a terminator.
    fn text(name: &[libc::c_char]) -> Option<String> {
        // SAFETY: c_char and u8 have the same size and alignment.
        let bytes = unsafe { std::slice::from_raw_parts(name.as_ptr().cast::<u8>(), name.len()) };
        CStr::from_bytes_until_nul(bytes)
            .ok()
            .map(|name| name.to_string_lossy().into_owned())
    }

    fn rusage(pid: u32) -> Option<libc::rusage_info_v4> {
        // SAFETY: a zeroed rusage_info_v4 is the exact V4 flavor output buffer.
        let mut usage: libc::rusage_info_v4 = unsafe { mem::zeroed() };
        let result = unsafe {
            libc::proc_pid_rusage(
                pid as libc::c_int,
                libc::RUSAGE_INFO_V4,
                ptr::from_mut(&mut usage).cast(),
            )
        };
        (result == 0).then_some(usage)
    }

    /// `rusage_info` CPU times are Mach absolute-time units, converted as
    /// `process_tree` converts them. libc marks its Mach timebase items
    /// deprecated in favour of another crate; no dependency is added for it.
    #[allow(deprecated)]
    fn ticks_to_ns(ticks: u64) -> u64 {
        static TIMEBASE: OnceLock<(u32, u32)> = OnceLock::new();
        let (numer, denom) = *TIMEBASE.get_or_init(|| {
            let mut info = libc::mach_timebase_info { numer: 0, denom: 0 };
            // SAFETY: a writable mach_timebase_info output struct.
            let result = unsafe { libc::mach_timebase_info(&mut info) };
            if result == 0 && info.numer > 0 && info.denom > 0 {
                (info.numer, info.denom)
            } else {
                (1, 1)
            }
        });
        let ns = u128::from(ticks) * u128::from(numer) / u128::from(denom);
        u64::try_from(ns).unwrap_or(u64::MAX)
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        process::{Command, Stdio},
    };

    #[test]
    fn the_scan_reads_this_process_and_a_child_with_its_parent_and_name() {
        let me = std::process::id();
        let mut child = Command::new("/bin/cat")
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let scan = scan();
        let own = scan
            .processes
            .iter()
            .find(|p| p.identity.pid == me)
            .expect("this test process is the current user's");
        assert!(own.cpu_time_ns > 0);
        assert!(own.identity.is_valid());
        assert_eq!(
            ProcessIdentity::capture(me).unwrap(),
            own.identity,
            "the same identity the lifecycle records"
        );
        let cat = scan
            .processes
            .iter()
            .find(|p| p.identity.pid == child.id())
            .expect("a child of the current user");
        assert_eq!(cat.parent_pid, me);
        assert_eq!(cat.name, "cat");
        assert!(cat.start_us().unwrap() >= own.start_us().unwrap());
        let pids: std::collections::BTreeSet<_> =
            scan.processes.iter().map(|p| p.identity.pid).collect();
        assert_eq!(pids.len(), scan.processes.len(), "one reading per PID");
        assert!(scan.processes.len() <= MAX_SCAN_PROCESSES);
        child.stdin.take().unwrap().write_all(b"x").unwrap();
        let mut out = String::new();
        child
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut out)
            .unwrap();
        assert_eq!(out, "x");
        assert!(child.wait().unwrap().success());
    }
}
