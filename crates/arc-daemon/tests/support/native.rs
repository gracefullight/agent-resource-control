//! Native process RSS/thread snapshots and owned-child wait4 lifetime peaks.
use std::io;

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
pub fn sample(pid: u32) -> io::Result<(u64, u64)> {
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
    Ok((info.resident_size, info.threadnum as u64))
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

pub fn host() -> serde_json::Value {
    fn output(program: &str, args: &[&str]) -> String {
        let out = std::process::Command::new(program)
            .args(args)
            .output()
            .unwrap();
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
    serde_json::json!({"uname":output("uname", &["-a"]),"hardware":hardware,
        "rustc":output("rustc", &["-Vv"]),"cargo":output("cargo", &["-V"]),
        "profile":if cfg!(debug_assertions) {"debug"} else {"release"},
        "local_head":output("git", &["rev-parse","HEAD"]),
        "instrumentation":"proc_pidinfo(PROC_PIDTASKINFO) on macOS; /proc/PID/status on Linux; wait4 child ru_maxrss lifetime peak; sampler is in test parent"})
}
