//! Darwin APIs only; no subprocesses, scheduling or attribution. Available RAM
//! is (free_count + inactive_count) * vm_page_size. free_count already includes
//! speculative pages. Compressed/purgeable overlap is deliberately excluded.
use crate::{CpuCounters, NativeFilesystem, NativeSample};
use arc_core::meter::{Diagnostic, FilesystemIdentity, MemoryPressure, Reading, MAX_SAFE_INTEGER};
use std::{ffi::CStr, io, mem, ptr};

// libc exposes mach_host_self but not its matching release operation. This is
// the SDK mach/mach_port.h ABI; no additional binding dependency is required.
unsafe extern "C" {
    fn mach_port_deallocate(
        task: libc::mach_port_t,
        name: libc::mach_port_t,
    ) -> libc::kern_return_t;
}
struct HostPort(libc::mach_port_t);
impl Drop for HostPort {
    #[allow(deprecated)] // Retain the existing libc binding, matching the SDK ABI.
    fn drop(&mut self) {
        // SAFETY: this send right was acquired by mach_host_self in this pass.
        unsafe { mach_port_deallocate(libc::mach_task_self(), self.0) };
    }
}

#[allow(deprecated)] // No new binding dependency for libc's supported host call.
pub(super) fn sample(observed_at_ms: Option<u64>, monotonic_ms: u64) -> NativeSample {
    // SAFETY: mach_host_self returns a send right for the current task.
    let host = HostPort(unsafe { libc::mach_host_self() });
    NativeSample {
        observed_at_ms,
        monotonic_ms,
        host_cpu: reading(cpu(host.0)),
        memory_available_bytes: reading(memory(host.0)),
        memory_pressure: reading(pressure()),
        swap_used_bytes: reading(swap()),
        filesystem: filesystem(),
        daemon_cpu_core_seconds: reading(process_cpu()),
        daemon_rss_bytes: reading(rss()),
    }
}

fn failure(label: &'static str) -> io::Error {
    io::Error::other(label)
}
fn reading<T>(result: io::Result<T>) -> Reading<T> {
    match result {
        Ok(value) => Reading::Available { value },
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => Reading::Denied {},
        Err(error)
            if error.kind() == io::ErrorKind::Unsupported
                || matches!(
                    error.raw_os_error(),
                    Some(libc::ENOENT | libc::ENOSYS | libc::ENOTSUP)
                ) =>
        {
            Reading::Unsupported {}
        }
        // Keep native/private data and OS-supplied strings out of diagnostics.
        Err(_) => Reading::Failed {
            diagnostic: Diagnostic::bounded("native_measurement_failed"),
        },
    }
}
fn mach_result(code: libc::kern_return_t) -> io::Result<()> {
    match code {
        libc::KERN_SUCCESS => Ok(()),
        libc::KERN_PROTECTION_FAILURE | libc::KERN_NO_ACCESS => {
            Err(io::ErrorKind::PermissionDenied.into())
        }
        _ => Err(failure("mach_statistics_failed")),
    }
}
fn sysctl<T>(name: &CStr, output: &mut T) -> io::Result<()> {
    let mut size = mem::size_of::<T>();
    // SAFETY: the private callers supply the SDK's exact output type, writable
    // storage and a NUL-terminated name. No new value is written by this call.
    if unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            ptr::from_mut(output).cast(),
            &mut size,
            ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if size != mem::size_of::<T>() {
        return Err(failure("short_sysctl_reply"));
    }
    Ok(())
}
fn cpu(host: libc::host_t) -> io::Result<CpuCounters> {
    // SAFETY: these SDK structs accept an all-zero initialized output buffer.
    let mut info: libc::host_cpu_load_info = unsafe { mem::zeroed() };
    let mut count = libc::HOST_CPU_LOAD_INFO_COUNT;
    // SAFETY: HOST_CPU_LOAD_INFO uses the exact buffer/count supplied here.
    mach_result(unsafe {
        libc::host_statistics(
            host,
            libc::HOST_CPU_LOAD_INFO,
            ptr::from_mut(&mut info).cast(),
            &mut count,
        )
    })?;
    if count != libc::HOST_CPU_LOAD_INFO_COUNT {
        return Err(failure("short_cpu_reply"));
    }
    let mut logical = 0i32;
    sysctl(c"hw.logicalcpu", &mut logical)?;
    if logical <= 0 {
        return Err(failure("invalid_cpu_topology"));
    }
    Ok(CpuCounters {
        user: u64::from(info.cpu_ticks[libc::CPU_STATE_USER as usize]),
        system: u64::from(info.cpu_ticks[libc::CPU_STATE_SYSTEM as usize]),
        nice: u64::from(info.cpu_ticks[libc::CPU_STATE_NICE as usize]),
        idle: u64::from(info.cpu_ticks[libc::CPU_STATE_IDLE as usize]),
        counter_bits: 32,
        logical_cpu_count: logical as u32,
    })
}
fn available_bytes(free: u32, inactive: u32, page_size: u64) -> io::Result<u64> {
    (u64::from(free) + u64::from(inactive))
        .checked_mul(page_size)
        .filter(|value| page_size > 0 && *value <= MAX_SAFE_INTEGER)
        .ok_or_else(|| failure("invalid_vm_counters"))
}
fn memory(host: libc::host_t) -> io::Result<u64> {
    // SAFETY: vm_statistics64 is the SDK's output buffer for HOST_VM_INFO64.
    let mut info: libc::vm_statistics64 = unsafe { mem::zeroed() };
    let mut count = libc::HOST_VM_INFO64_COUNT;
    mach_result(unsafe {
        libc::host_statistics64(
            host,
            libc::HOST_VM_INFO64,
            ptr::from_mut(&mut info).cast(),
            &mut count,
        )
    })?;
    // Only the stable revision-0 prefix through inactive_count is required.
    if count < 3 {
        return Err(failure("short_vm_reply"));
    }
    // SAFETY: libSystem initializes vm_page_size before application entry.
    available_bytes(
        info.free_count,
        info.inactive_count,
        unsafe { libc::vm_page_size } as u64,
    )
}
fn pressure_level(level: i32) -> io::Result<MemoryPressure> {
    // kern.memorystatus_vm_pressure_level exports NOTE_MEMORYSTATUS_* flags,
    // rather than the kernel's internal kVMPressure* ordinal values.
    match level {
        1 => Ok(MemoryPressure::Normal),
        2 => Ok(MemoryPressure::Warning),
        4 => Ok(MemoryPressure::Critical),
        _ => Err(failure("unrecognized_native_pressure")),
    }
}
fn pressure() -> io::Result<MemoryPressure> {
    let mut level = 0i32;
    sysctl(c"kern.memorystatus_vm_pressure_level", &mut level)?;
    pressure_level(level)
}
fn swap() -> io::Result<u64> {
    // SAFETY: xsw_usage is a plain C output struct with a valid zero value.
    let mut usage: libc::xsw_usage = unsafe { mem::zeroed() };
    sysctl(c"vm.swapusage", &mut usage)?;
    if usage.xsu_used > usage.xsu_total {
        return Err(failure("invalid_swap_usage"));
    }
    Ok(usage.xsu_used)
}
fn process_cpu() -> io::Result<f64> {
    // SAFETY: rusage is a plain writable C struct, with an exact SELF selector.
    let mut usage: libc::rusage = unsafe { mem::zeroed() };
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let time = |t: libc::timeval| {
        if t.tv_sec < 0 || !(0..1_000_000).contains(&t.tv_usec) {
            return Err(failure("invalid_process_time"));
        }
        Ok(t.tv_sec as f64 + t.tv_usec as f64 / 1_000_000.0)
    };
    Ok(time(usage.ru_utime)? + time(usage.ru_stime)?)
}
fn rss() -> io::Result<u64> {
    // SAFETY: exact SDK proc_taskinfo buffer, observing only the caller's PID.
    let mut info: libc::proc_taskinfo = unsafe { mem::zeroed() };
    let size = mem::size_of_val(&info) as i32;
    if unsafe {
        libc::proc_pidinfo(
            std::process::id() as i32,
            libc::PROC_PIDTASKINFO,
            0,
            ptr::from_mut(&mut info).cast(),
            size,
        )
    } != size
    {
        return Err(io::Error::last_os_error());
    }
    // Do not substitute rusage.ru_maxrss (a native lifetime high-water mark).
    Ok(info.pti_resident_size)
}
fn native_text(bytes: &[libc::c_char]) -> io::Result<String> {
    let end = bytes
        .iter()
        .position(|b| *b == 0)
        .ok_or_else(|| failure("unterminated_native_path"))?;
    let value: Vec<u8> = bytes[..end].iter().map(|b| *b as u8).collect();
    String::from_utf8(value).map_err(|_| failure("non_utf8_filesystem"))
}
fn filesystem() -> NativeFilesystem {
    let result = (|| -> io::Result<_> {
        // SAFETY: statfs writes the exact initialized SDK output struct for /.
        let mut info: libc::statfs = unsafe { mem::zeroed() };
        if unsafe { libc::statfs(c"/".as_ptr(), &mut info) } != 0 {
            return Err(io::Error::last_os_error());
        }
        if info.f_flags & libc::MNT_LOCAL as u32 == 0 {
            return Err(io::ErrorKind::Unsupported.into());
        }
        let identity = FilesystemIdentity {
            path: "/".into(),
            mount_point: native_text(&info.f_mntonname)?,
            filesystem_type: native_text(&info.f_fstypename)?,
            // fsid_t has opaque private fields in libc. Its exact SDK ABI is
            // two signed 32-bit words; copy its bytes without field access.
            volume_id: {
                let mut words = [0i32; 2];
                // SAFETY: fsid_t and this initialized buffer have equal sizes.
                unsafe {
                    ptr::copy_nonoverlapping(
                        ptr::from_ref(&info.f_fsid).cast::<u8>(),
                        words.as_mut_ptr().cast(),
                        mem::size_of_val(&words),
                    )
                };
                format!("{:08x}:{:08x}", words[0], words[1])
            },
            is_local: true,
        };
        if !identity.is_valid()
            || info.f_bsize == 0
            || info.f_bfree > info.f_blocks
            || info.f_bavail > info.f_bfree
        {
            return Err(failure("invalid_filesystem_counters"));
        }
        let bytes = |blocks: u64| {
            blocks
                .checked_mul(u64::from(info.f_bsize))
                .filter(|n| *n <= MAX_SAFE_INTEGER)
                .ok_or_else(|| failure("filesystem_overflow"))
        };
        Ok((
            identity,
            bytes(info.f_blocks)?,
            bytes(info.f_bfree)?,
            bytes(info.f_bavail)?,
        ))
    })();
    match result {
        Ok((identity, capacity, free, available)) => NativeFilesystem {
            identity: Some(identity),
            capacity_bytes: Reading::Available { value: capacity },
            free_bytes: Reading::Available { value: free },
            available_bytes: Reading::Available { value: available },
        },
        Err(error) => {
            let unknown = reading::<u64>(Err(error));
            NativeFilesystem {
                identity: None,
                capacity_bytes: unknown.clone(),
                free_bytes: unknown.clone(),
                available_bytes: unknown,
            }
        }
    }
}

#[cfg(test)]
#[path = "macos_tests.rs"]
mod tests;
