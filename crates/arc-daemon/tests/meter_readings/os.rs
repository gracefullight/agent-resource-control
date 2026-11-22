//! The test's own readings of the operating system, taken in the test process
//! with `libc` only. Nothing here is shared with the daemon's sampler: the
//! counters are read again and the values are derived again.
use std::{ffi::CStr, io, mem, ptr};

// libc exposes mach_host_self but not its matching release operation. This is
// the SDK mach/mach_port.h ABI; no binding dependency is added for one call.
unsafe extern "C" {
    fn mach_port_deallocate(
        task: libc::mach_port_t,
        name: libc::mach_port_t,
    ) -> libc::kern_return_t;
}

struct HostPort(libc::mach_port_t);
impl HostPort {
    #[allow(deprecated)] // libc points its Mach bindings at `mach2`, not a workspace dependency.
    fn acquire() -> Self {
        // SAFETY: mach_host_self returns a send right for the current task.
        Self(unsafe { libc::mach_host_self() })
    }
}
impl Drop for HostPort {
    #[allow(deprecated)]
    fn drop(&mut self) {
        // SAFETY: this send right was acquired by `acquire` and is released once.
        unsafe { mach_port_deallocate(libc::mach_task_self(), self.0) };
    }
}

fn sysctl<T>(name: &CStr, output: &mut T) {
    let mut size = mem::size_of::<T>();
    // SAFETY: each caller supplies the SDK's exact output type for `name`,
    // writable storage of that size and a NUL-terminated name.
    let code = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            ptr::from_mut(output).cast(),
            &mut size,
            ptr::null_mut(),
            0,
        )
    };
    assert_eq!(code, 0, "sysctl {name:?}: {}", io::Error::last_os_error());
    assert_eq!(size, mem::size_of::<T>(), "sysctl {name:?} reply size");
}

/// Host CPU scheduler ticks summed over all logical CPUs since boot. The
/// kernel exports 32-bit counters that wrap, so only differences are used.
#[derive(Debug, Clone, Copy)]
pub struct CpuTicks {
    busy: [u32; 3],
    idle: u32,
}

impl CpuTicks {
    /// (busy, idle) ticks from `earlier` to `self`. Each counter may have
    /// wrapped once; a window of this test is far shorter than one wrap.
    pub fn since(&self, earlier: &Self) -> (u64, u64) {
        let busy = (0..3)
            .map(|state| u64::from(self.busy[state].wrapping_sub(earlier.busy[state])))
            .sum();
        (busy, u64::from(self.idle.wrapping_sub(earlier.idle)))
    }
}

/// One pass over the host counters of the five Meter values under test.
#[derive(Debug, Clone, Copy)]
pub struct Host {
    pub cpu: CpuTicks,
    pub memory_available_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
}

pub fn host() -> Host {
    let port = HostPort::acquire();
    let (swap_used_bytes, swap_total_bytes) = swap();
    Host {
        cpu: cpu(port.0),
        memory_available_bytes: memory_available(port.0),
        swap_used_bytes,
        swap_total_bytes,
    }
}

fn mach(code: libc::kern_return_t, label: &str) {
    assert_eq!(code, libc::KERN_SUCCESS, "{label}: kern_return {code}");
}

fn cpu(host: libc::host_t) -> CpuTicks {
    // SAFETY: the SDK struct accepts an all-zero initialized output buffer.
    let mut info: libc::host_cpu_load_info = unsafe { mem::zeroed() };
    let mut count = libc::HOST_CPU_LOAD_INFO_COUNT;
    // SAFETY: HOST_CPU_LOAD_INFO fills exactly the buffer and count supplied.
    mach(
        unsafe {
            libc::host_statistics(
                host,
                libc::HOST_CPU_LOAD_INFO,
                ptr::from_mut(&mut info).cast(),
                &mut count,
            )
        },
        "host_statistics(HOST_CPU_LOAD_INFO)",
    );
    assert_eq!(count, libc::HOST_CPU_LOAD_INFO_COUNT);
    let state = |index: libc::c_int| info.cpu_ticks[index as usize];
    CpuTicks {
        busy: [
            state(libc::CPU_STATE_USER),
            state(libc::CPU_STATE_SYSTEM),
            state(libc::CPU_STATE_NICE),
        ],
        idle: state(libc::CPU_STATE_IDLE),
    }
}

/// Memory a new allocation can use without swapping: free pages (which
/// include speculative pages) plus inactive pages, in bytes.
fn memory_available(host: libc::host_t) -> u64 {
    // SAFETY: vm_statistics64 is the SDK's output buffer for HOST_VM_INFO64.
    let mut info: libc::vm_statistics64 = unsafe { mem::zeroed() };
    let mut count = libc::HOST_VM_INFO64_COUNT;
    // SAFETY: HOST_VM_INFO64 fills at most the buffer and count supplied.
    mach(
        unsafe {
            libc::host_statistics64(
                host,
                libc::HOST_VM_INFO64,
                ptr::from_mut(&mut info).cast(),
                &mut count,
            )
        },
        "host_statistics64(HOST_VM_INFO64)",
    );
    (u64::from(info.free_count) + u64::from(info.inactive_count)) * page_size()
}

fn page_size() -> u64 {
    // SAFETY: sysconf has no preconditions.
    let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    u64::try_from(size)
        .ok()
        .filter(|size| *size > 0)
        .expect("page size")
}

/// (used, total) bytes of the swap the kernel has configured right now.
fn swap() -> (u64, u64) {
    // SAFETY: xsw_usage is a plain C output struct with a valid zero value.
    let mut usage: libc::xsw_usage = unsafe { mem::zeroed() };
    sysctl(c"vm.swapusage", &mut usage);
    (usage.xsu_used, usage.xsu_total)
}

pub fn physical_memory_bytes() -> u64 {
    let mut bytes = 0u64;
    sysctl(c"hw.memsize", &mut bytes);
    bytes
}

pub fn logical_cpu_count() -> u32 {
    let mut count: libc::c_int = 0;
    sysctl(c"hw.logicalcpu", &mut count);
    u32::try_from(count)
        .ok()
        .filter(|count| *count > 0)
        .expect("logical CPU count")
}

/// Scheduler ticks one logical CPU accumulates per second.
pub fn ticks_per_second() -> u64 {
    // SAFETY: sysconf has no preconditions.
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    u64::try_from(ticks)
        .ok()
        .filter(|ticks| *ticks > 0)
        .expect("clock ticks")
}
