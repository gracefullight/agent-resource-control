//! Native process RSS/thread/CPU-time snapshots and owned-child wait4 lifetime peaks.
#![allow(dead_code)] // Each integration binary uses the subset it needs.
use std::io;
#[path = "child_watchdog.rs"]
#[allow(clippy::duplicate_mod)] // Included by independently reusable fixture modules.
mod child_watchdog;

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Default)]
struct TaskInfo {
    virtual_size: u64,
    resident_size: u64,
    total_user: u64,
    total_system: u64,
    threads_user: u64,
    threads_system: u64,
    policy: i32,
    faults: i32,
    pageins: i32,
    cow_faults: i32,
    messages_sent: i32,
    messages_received: i32,
    syscalls_mach: i32,
    syscalls_unix: i32,
    csw: i32,
    threadnum: i32,
    numrunning: i32,
    priority: i32,
}

#[cfg(target_os = "macos")]
#[link(name = "proc")]
unsafe extern "C" {
    fn proc_pidinfo(pid: i32, flavor: i32, arg: u64, buffer: *mut libc::c_void, size: i32) -> i32;
}

#[cfg(target_os = "macos")]
fn task_info(pid: u32) -> io::Result<TaskInfo> {
    let mut info = TaskInfo::default();
    let size = std::mem::size_of::<TaskInfo>() as i32;
    let actual = unsafe {
        proc_pidinfo(
            pid as i32,
            4,
            0,
            &mut info as *mut _ as *mut libc::c_void,
            size,
        )
    };
    if actual != size {
        return Err(io::Error::last_os_error());
    }
    Ok(info)
}

#[cfg(target_os = "macos")]
pub fn sample(pid: u32) -> io::Result<(u64, u64)> {
    let info = task_info(pid)?;
    Ok((info.resident_size, info.threadnum as u64))
}

/// Cumulative CPU time (user plus system) the OS has charged to a live
/// process, in nanoseconds. The kernel reports these two counters in Mach
/// absolute-time units, which are not nanoseconds on Apple Silicon, so they
/// are converted with the host timebase.
#[cfg(target_os = "macos")]
#[allow(deprecated)] // libc points its Mach bindings at `mach2`, which is not a workspace dependency.
pub fn cpu_time_ns(pid: u32) -> io::Result<u64> {
    let info = task_info(pid)?;
    let mut timebase = libc::mach_timebase_info { numer: 0, denom: 0 };
    // SAFETY: the call only fills the supplied two-field structure.
    if unsafe { libc::mach_timebase_info(&mut timebase) } != 0 || timebase.denom == 0 {
        return Err(io::Error::other("mach_timebase_info failed"));
    }
    let units = u128::from(info.total_user) + u128::from(info.total_system);
    u64::try_from(units * u128::from(timebase.numer) / u128::from(timebase.denom))
        .map_err(io::Error::other)
}

#[cfg(target_os = "linux")]
pub fn sample(pid: u32) -> io::Result<(u64, u64)> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/status"))?;
    let field = |key: &str| -> io::Result<u64> {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| io::Error::other(format!("missing {key}")))
    };
    Ok((field("VmRSS:")? * 1024, field("Threads:")?))
}

/// Cumulative CPU time (user plus system) of a live process in nanoseconds,
/// from the scheduler tick counters after the parenthesised command name.
#[cfg(target_os = "linux")]
pub fn cpu_time_ns(pid: u32) -> io::Result<u64> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let fields: Vec<&str> = text
        .rsplit_once(')')
        .map(|(_, rest)| rest.split_whitespace().collect())
        .unwrap_or_default();
    // After the name: state is field 0, utime field 11 and stime field 12.
    let ticks = |index: usize| -> io::Result<u64> {
        fields
            .get(index)
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| io::Error::other("missing /proc stat CPU field"))
    };
    // SAFETY: sysconf has no preconditions.
    let per_second = u64::try_from(unsafe { libc::sysconf(libc::_SC_CLK_TCK) })
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| io::Error::other("sysconf(_SC_CLK_TCK) failed"))?;
    Ok((ticks(11)? + ticks(12)?) * 1_000_000_000 / per_second)
}

/// The measurement host as recorded beside the numbers: kernel and machine
/// type, hardware size and build profile. Each probe gets a cleared
/// environment; no host name, account, toolchain or repository state is read.
pub fn host() -> serde_json::Value {
    fn output(program: &str, args: &[&str]) -> String {
        let mut command = std::process::Command::new(program);
        command
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
        let out = child_watchdog::output(&mut command, program).unwrap();
        assert!(out.status.success(), "{out:?}");
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    }
    let hardware = if cfg!(target_os = "macos") {
        output(
            "sysctl",
            &["-n", "hw.memsize", "hw.ncpu", "machdep.cpu.brand_string"],
        )
    } else {
        std::fs::read_to_string("/proc/meminfo").unwrap()
    };
    serde_json::json!({"uname":output("uname", &["-srm"]),"hardware":hardware,
        "profile":if cfg!(debug_assertions) {"debug"} else {"release"},
        "instrumentation":"proc_pidinfo(PROC_PIDTASKINFO) on macOS; /proc/PID/status on Linux; wait4 child ru_maxrss lifetime peak; sampler is in test parent"})
}
