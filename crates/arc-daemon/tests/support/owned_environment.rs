// Environment marker for descendants that close inherited descriptors.
use std::{io, os::unix::ffi::OsStrExt, path::Path};

fn marker(path: &Path) -> Vec<u8> {
    let mut result = b"ARC_TEST_OWNERSHIP=".to_vec();
    result.extend_from_slice(path.as_os_str().as_bytes());
    result
}

#[cfg(target_os = "macos")]
fn read_environment(pid: u32, bytes: &mut [u8], require_known: bool) -> io::Result<Option<usize>> {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of_val(&info) as i32;
    if unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    } != size
        || info.pbi_uid != unsafe { libc::geteuid() }
    {
        return Ok(None);
    }
    let mut mib = [libc::CTL_KERN, 49, pid as i32]; // KERN_PROCARGS2
    let mut len = bytes.len();
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            bytes.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        let error = io::Error::last_os_error();
        if !require_known {
            return Ok(None);
        }
        // A known process can exit between enumeration and procargs2. A
        // still-live known process that cannot be rechecked is an error.
        let written = unsafe {
            libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdinfo).cast(),
                size,
            )
        };
        if written != size || info.pbi_status == 5 {
            return Ok(None);
        }
        return Err(io::Error::other(format!(
            "cannot inspect pid {pid} ownership environment: {error}"
        )));
    }
    let parsed = (|| {
        if len < 4 {
            return None;
        }
        let argc = u32::from_ne_bytes(bytes[..4].try_into().ok()?);
        let mut at = 4 + bytes[4..len].iter().position(|byte| *byte == 0)? + 1;
        while at < len && bytes[at] == 0 {
            at += 1;
        }
        for _ in 0..argc {
            at += bytes.get(at..len)?.iter().position(|byte| *byte == 0)? + 1;
        }
        bytes.copy_within(at..len, 0);
        Some(len - at)
    })();
    Ok(parsed)
}

#[cfg(target_os = "linux")]
fn read_environment(pid: u32, bytes: &mut [u8], require_known: bool) -> io::Result<Option<usize>> {
    use std::os::unix::fs::MetadataExt;
    let directory = format!("/proc/{pid}");
    let metadata = match std::fs::metadata(&directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) if !require_known => return Ok(None),
        Err(error) => return Err(error),
    };
    if metadata.uid() != unsafe { libc::geteuid() } {
        return Ok(None);
    }
    let environment = match std::fs::read(format!("{directory}/environ")) {
        Ok(environment) => environment,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) if !require_known => return Ok(None),
        Err(error) => return Err(error),
    };
    if environment.len() > bytes.len() {
        if !require_known {
            return Ok(None);
        }
        return Err(io::Error::other(format!(
            "pid {pid} ownership environment exceeds buffer"
        )));
    }
    bytes[..environment.len()].copy_from_slice(&environment);
    Ok(Some(environment.len()))
}

fn buffer() -> Vec<u8> {
    vec![0; 1024 * 1024]
}

fn contains(pid: u32, path: &Path, bytes: &mut [u8], require_known: bool) -> io::Result<bool> {
    let Some(len) = read_environment(pid, bytes, require_known)? else {
        return Ok(false);
    };
    let wanted = marker(path);
    Ok(bytes[..len]
        .split(|byte| *byte == 0)
        .any(|entry| entry == wanted))
}

pub fn matches(pid: u32, path: &Path) -> io::Result<bool> {
    contains(pid, path, &mut buffer(), true)
}

#[cfg(target_os = "macos")]
pub fn members(path: &Path) -> io::Result<Vec<u32>> {
    let mut ids = vec![0i32; 128];
    loop {
        let bytes = unsafe {
            libc::proc_listpids(
                1,
                0,
                ids.as_mut_ptr().cast(),
                std::mem::size_of_val(ids.as_slice()) as i32,
            )
        };
        if bytes < 0 {
            return Err(io::Error::last_os_error());
        }
        let count = bytes as usize / std::mem::size_of::<i32>();
        if count < ids.len() {
            let mut env = buffer();
            let mut found = Vec::new();
            for pid in ids[..count]
                .iter()
                .copied()
                .filter(|pid| *pid > 0)
                .map(|pid| pid as u32)
            {
                if pid != std::process::id() && contains(pid, path, &mut env, false)? {
                    found.push(pid);
                }
            }
            return Ok(found);
        }
        if ids.len() >= 16_384 {
            return Err(io::Error::other("ownership environment process limit"));
        }
        ids.resize(ids.len() * 2, 0);
    }
}

#[cfg(target_os = "linux")]
pub fn members(path: &Path) -> io::Result<Vec<u32>> {
    let mut env = buffer();
    let mut found = Vec::new();
    for entry in std::fs::read_dir("/proc")? {
        let entry = entry?;
        if let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        {
            if pid != std::process::id() && contains(pid, path, &mut env, false)? {
                found.push(pid);
            }
        }
    }
    Ok(found)
}
