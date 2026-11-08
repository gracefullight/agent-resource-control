//! Native identity evidence shared by locks and lifecycle clients.
use serde::{Deserialize, Serialize};
use std::io;

#[cfg(target_os = "linux")]
mod linux {
    use std::io;
    pub(super) fn process_start_time(pid: u32) -> io::Result<u64> {
        start_time(pid, true)
    }
    pub(super) fn live_start_time(pid: u32) -> io::Result<u64> {
        start_time(pid, false)
    }
    fn start_time(pid: u32, include_zombie: bool) -> io::Result<u64> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
        let closing = stat.rfind(')').ok_or_else(super::unavailable)?;
        if !include_zombie && stat[closing + 1..].split_whitespace().next() == Some("Z") {
            return Err(io::Error::from_raw_os_error(libc::ESRCH));
        }
        stat[closing + 1..]
            .split_whitespace()
            .nth(19)
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .ok_or_else(super::unavailable)
    }
    pub(super) fn boot_id() -> io::Result<String> {
        let value = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?
            .trim()
            .to_owned();
        if !super::valid_boot_id(&value) {
            return Err(super::unavailable());
        }
        Ok(value)
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::io;

    pub(super) fn live_start_time(pid: u32) -> io::Result<u64> {
        // SAFETY: zeroed proc_bsdinfo is a valid output buffer; size is exact.
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
        let written = unsafe {
            libc::proc_pidinfo(
                pid as libc::c_int,
                libc::PROC_PIDTBSDINFO,
                0,
                &mut info as *mut _ as *mut libc::c_void,
                size,
            )
        };
        if written != size {
            let error = io::Error::last_os_error();
            return Err(error);
        }
        info.pbi_start_tvsec
            .checked_mul(1_000_000)
            .and_then(|v| v.checked_add(info.pbi_start_tvusec))
            .filter(|v| *v > 0)
            .ok_or_else(super::unavailable)
    }
    pub(super) fn boot_id() -> io::Result<String> {
        let mut bytes = [0u8; 37];
        let mut length = bytes.len();
        // SAFETY: name is NUL terminated; bytes and length are valid writable buffers.
        let result = unsafe {
            libc::sysctlbyname(
                c"kern.bootsessionuuid".as_ptr(),
                bytes.as_mut_ptr().cast(),
                &mut length,
                std::ptr::null_mut(),
                0,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        if length != 37 || bytes[36] != 0 {
            return Err(super::unavailable());
        }
        let value = std::str::from_utf8(&bytes[..36])
            .map_err(|_| super::unavailable())?
            .to_ascii_lowercase();
        if !super::valid_boot_id(&value) {
            return Err(super::unavailable());
        }
        Ok(value)
    }

    // Darwin exports kinfo_proc.kp_proc beginning with this LP64 extern_proc prefix
    // (SDK sys/proc.h). Read only the prefix, never command/credential fields.
    // XNU kern_sysctl.c fills the same p_start for normal and zombie processes.
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct ProcPrefix {
        start: libc::timeval,
        vmspace: *mut libc::c_void,
        sigacts: *mut libc::c_void,
        flags: libc::c_int,
        status: libc::c_char,
        pid: libc::pid_t,
    }
    fn zombie_start_time(pid: u32) -> io::Result<u64> {
        let mut mib = [
            libc::CTL_KERN,
            libc::KERN_PROC,
            libc::KERN_PROC_PID,
            pid as i32,
        ];
        let mut bytes = [0u8; 4096];
        let mut length = bytes.len();
        // SAFETY: bounded initialized output storage and a four-element native MIB.
        let result = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                mib.len() as u32,
                bytes.as_mut_ptr().cast(),
                &mut length,
                std::ptr::null_mut(),
                0,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        if length == 0 {
            return Err(io::Error::from_raw_os_error(libc::ESRCH));
        }
        if length < std::mem::size_of::<ProcPrefix>() || length > bytes.len() {
            return Err(super::unavailable());
        }
        // SAFETY: the returned bytes include the complete documented prefix; the
        // byte buffer may be unaligned. All prefix field bit patterns are valid.
        let prefix = unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast::<ProcPrefix>()) };
        if prefix.pid != pid as i32
            || prefix.status as u32 != libc::SZOMB
            || prefix.start.tv_sec <= 0
            || !(0..1_000_000).contains(&prefix.start.tv_usec)
        {
            return Err(super::unavailable());
        }
        (prefix.start.tv_sec as u64)
            .checked_mul(1_000_000)
            .and_then(|v| v.checked_add(prefix.start.tv_usec as u64))
            .ok_or_else(super::unavailable)
    }

    pub(super) fn process_start_time(pid: u32) -> io::Result<u64> {
        match live_start_time(pid) {
            Err(error) if error.raw_os_error() == Some(libc::ESRCH) => zombie_start_time(pid),
            result => result,
        }
    }
}

#[cfg(target_os = "linux")]
use linux as native;
#[cfg(target_os = "macos")]
use macos as native;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub start_token: String,
    pub boot_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIdentity {
    pid: u32,
    start_token: String,
    boot_id: String,
}
impl<'de> Deserialize<'de> for ProcessIdentity {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Object;
        impl<'de> serde::de::Visitor<'de> for Object {
            type Value = ProcessIdentity;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a process identity object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<Self::Value, A::Error> {
                let raw =
                    RawIdentity::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                ProcessIdentity::try_from(raw).map_err(serde::de::Error::custom)
            }
        }
        deserializer.deserialize_map(Object)
    }
}
impl TryFrom<RawIdentity> for ProcessIdentity {
    type Error = &'static str;
    fn try_from(raw: RawIdentity) -> Result<Self, Self::Error> {
        let identity = Self {
            pid: raw.pid,
            start_token: raw.start_token,
            boot_id: raw.boot_id,
        };
        identity
            .is_valid()
            .then_some(identity)
            .ok_or("invalid process identity")
    }
}
impl ProcessIdentity {
    pub fn is_valid(&self) -> bool {
        self.pid > 0
            && self.pid <= i32::MAX as u32
            && self
                .start_token
                .parse::<u64>()
                .is_ok_and(|v| v > 0 && v.to_string() == self.start_token)
            && valid_boot_id(&self.boot_id)
    }
    pub fn capture(pid: u32) -> io::Result<Self> {
        if pid == 0 || pid > i32::MAX as u32 {
            return Err(unavailable());
        }
        let boot_id = boot_id()?;
        let start_token = process_start_time(pid)?.to_string();
        let identity = Self {
            pid,
            start_token,
            boot_id,
        };
        if !identity.is_valid() || identity.boot_id != self::boot_id()? {
            return Err(unavailable());
        }
        Ok(identity)
    }
}
pub fn valid_boot_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityStatus {
    VerifiedMatch,
    Missing,
    Mismatch,
    Indeterminate,
}

/// A permission failure is indeterminate; existence alone never verifies identity.
pub fn probe(identity: &ProcessIdentity) -> IdentityStatus {
    probe_with(identity, process_start_time)
}
/// Observer admission requires a live process, not just a retained zombie tuple.
pub fn probe_live(identity: &ProcessIdentity) -> IdentityStatus {
    probe_with(identity, |pid| {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            native::live_start_time(pid)
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = pid;
            Err(unavailable())
        }
    })
}
fn probe_with(
    identity: &ProcessIdentity,
    read: impl FnOnce(u32) -> io::Result<u64>,
) -> IdentityStatus {
    if !identity.is_valid() {
        return IdentityStatus::Indeterminate;
    }
    let Ok(boot) = boot_id() else {
        return IdentityStatus::Indeterminate;
    };
    if boot != identity.boot_id {
        return IdentityStatus::Mismatch;
    }
    match read(identity.pid) {
        Ok(token) if token.to_string() == identity.start_token => IdentityStatus::VerifiedMatch,
        Ok(_) => IdentityStatus::Mismatch,
        Err(error) if error.kind() == io::ErrorKind::NotFound || is_missing(&error) => {
            IdentityStatus::Missing
        }
        Err(_) => IdentityStatus::Indeterminate,
    }
}
fn is_missing(error: &io::Error) -> bool {
    #[cfg(unix)]
    {
        error.raw_os_error() == Some(libc::ESRCH)
    }
    #[cfg(not(unix))]
    {
        let _ = error;
        false
    }
}
fn unavailable() -> io::Error {
    io::Error::other("process identity unavailable")
}

pub fn process_start_time(pid: u32) -> io::Result<u64> {
    if pid == 0 || pid > i32::MAX as u32 {
        return Err(unavailable());
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        native::process_start_time(pid)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native identity unsupported",
        ))
    }
}
pub fn boot_id() -> io::Result<String> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        native::boot_id()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native identity unsupported",
        ))
    }
}
/// Conservative compatibility helper for the existing daemon lock.
#[cfg(unix)]
pub fn process_is_alive(pid: u32) -> bool {
    // SAFETY: signal zero only checks existence; callers supply a process id.
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    result == 0 || io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}
/// Generate a lifecycle key from OS randomness, without command-derived input.
pub fn random_job_id() -> io::Result<String> {
    #[cfg(unix)]
    {
        use std::io::Read;
        let mut bytes = [0u8; 16];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }
    #[cfg(not(unix))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native randomness unsupported",
        ))
    }
}
