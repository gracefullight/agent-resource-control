//! macOS process-tree reader for job metering (lane A, MTR-002, DET-012).
//!
//! Enumerates a job's process group (`proc_listpgrppids`) and ppid
//! descendants (`proc_listchildpids`) from its recorded child identity and
//! reads `proc_pid_rusage` (`rusage_info_v4`: CPU time, reaped-children CPU
//! time, disk I/O bytes) and `proc_pidinfo` (BSD info for parent, group and
//! start time; task info for resident size). The process group counts only
//! when the recorded child leads it, so the wrapper's own group is never
//! read. Each reading carries the identity it was read under; the caller
//! revalidates it with `arc_core::process_identity::probe_live` before it
//! counts. Read-only: no signal, no subprocess.
use arc_core::process_identity::ProcessIdentity;

/// At most this many processes are read for one tree.
pub const MAX_TREE_PROCESSES: usize = 4096;

/// One native reading of one process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeProcess {
    pub identity: ProcessIdentity,
    pub parent_pid: u32,
    pub process_group: u32,
    /// The process's own user plus system CPU time.
    pub cpu_time_ns: u64,
    /// User plus system CPU time of the children it has reaped.
    pub child_cpu_time_ns: u64,
    pub rss_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
}

/// The process group and ppid descendants of `root`, including `root`.
/// An empty result means nothing could be read, never a zero-usage tree:
/// a root whose recorded identity no longer matches a live process (for
/// example a recycled PID) yields nothing, and so do its descendants.
pub fn read_tree(root: &ProcessIdentity) -> Vec<NativeProcess> {
    #[cfg(target_os = "macos")]
    {
        native::read_tree(root)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = root;
        Vec::new()
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::{NativeProcess, MAX_TREE_PROCESSES};
    use arc_core::process_identity::{boot_id, ProcessIdentity};
    use std::{collections::BTreeMap, mem, ptr, sync::OnceLock};

    struct Bsd {
        parent: u32,
        group: u32,
        start: u64,
    }

    pub(super) fn read_tree(root: &ProcessIdentity) -> Vec<NativeProcess> {
        let Ok(boot) = boot_id() else {
            return Vec::new();
        };
        if !root.is_valid() || root.boot_id != boot {
            return Vec::new();
        }
        let Some(info) = bsd(root.pid).filter(|i| i.start.to_string() == root.start_token) else {
            return Vec::new();
        };
        let leads_group = info.group == root.pid;
        let mut members = BTreeMap::from([(root.pid, info)]);
        if leads_group {
            for pid in list(libc::proc_listpgrppids, root.pid) {
                if let Some(info) = bsd(pid).filter(|i| i.group == root.pid) {
                    members.entry(pid).or_insert(info);
                }
            }
        }
        let mut frontier: Vec<u32> = members.keys().copied().collect();
        // Over ppid edges from every member, bounded.
        while let Some(parent) = frontier.pop() {
            for pid in list(libc::proc_listchildpids, parent) {
                if members.len() >= MAX_TREE_PROCESSES || members.contains_key(&pid) {
                    continue;
                }
                if let Some(info) = bsd(pid).filter(|i| i.parent == parent) {
                    members.insert(pid, info);
                    frontier.push(pid);
                }
            }
        }
        members
            .into_iter()
            .filter_map(|(pid, info)| read(pid, info, &boot))
            .collect()
    }

    fn read(pid: u32, info: Bsd, boot: &str) -> Option<NativeProcess> {
        let usage = rusage(pid)?;
        let rss = resident(pid)?;
        // The readings belong to this start time only if it is unchanged.
        bsd(pid).filter(|again| again.start == info.start)?;
        let ns = |ticks: u64| ticks_to_ns(ticks);
        Some(NativeProcess {
            identity: ProcessIdentity {
                pid,
                start_token: info.start.to_string(),
                boot_id: boot.to_owned(),
            },
            parent_pid: info.parent,
            process_group: info.group,
            cpu_time_ns: ns(usage.ri_user_time).saturating_add(ns(usage.ri_system_time)),
            child_cpu_time_ns: ns(usage.ri_child_user_time)
                .saturating_add(ns(usage.ri_child_system_time)),
            rss_bytes: rss,
            disk_read_bytes: usage.ri_diskio_bytesread,
            disk_write_bytes: usage.ri_diskio_byteswritten,
        })
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
        Some(Bsd {
            parent: info.pbi_ppid,
            group: info.pbi_pgid,
            start,
        })
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

    fn resident(pid: u32) -> Option<u64> {
        // SAFETY: exact SDK proc_taskinfo output buffer for this PID.
        let mut info: libc::proc_taskinfo = unsafe { mem::zeroed() };
        let size = mem::size_of_val(&info) as libc::c_int;
        let written = unsafe {
            libc::proc_pidinfo(
                pid as libc::c_int,
                libc::PROC_PIDTASKINFO,
                0,
                ptr::from_mut(&mut info).cast(),
                size,
            )
        };
        (written == size).then_some(info.pti_resident_size)
    }

    type Lister = unsafe extern "C" fn(libc::pid_t, *mut libc::c_void, libc::c_int) -> libc::c_int;

    /// Live PIDs named by one bounded native listing; zero entries are unused.
    fn list(lister: Lister, of: u32) -> Vec<u32> {
        let mut pids = vec![0 as libc::pid_t; MAX_TREE_PROCESSES];
        let bytes = mem::size_of_val(pids.as_slice()) as libc::c_int;
        // SAFETY: the buffer is writable for exactly `bytes` bytes.
        let count = unsafe { lister(of as libc::pid_t, pids.as_mut_ptr().cast(), bytes) };
        let Ok(count) = usize::try_from(count) else {
            return Vec::new();
        };
        pids.truncate(count.min(MAX_TREE_PROCESSES));
        pids.into_iter()
            .filter_map(|pid| u32::try_from(pid).ok().filter(|pid| *pid > 0))
            .collect()
    }

    /// `rusage_info` CPU times are Mach absolute-time units. libc marks its
    /// Mach timebase items deprecated in favour of another crate; the
    /// declaration is the SDK's and no dependency is added for it.
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
#[path = "process_tree/tests.rs"]
mod tests;
