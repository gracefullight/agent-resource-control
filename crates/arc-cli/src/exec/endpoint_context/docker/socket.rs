//! Inspect a Unix socket's listener without connecting to it.
use std::{
    fs,
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::Path,
};

pub(super) fn is_local_engine(endpoint: &str) -> bool {
    let Some(path) = endpoint.strip_prefix("unix://") else {
        return false;
    };
    let path = Path::new(path);
    if !path.is_absolute() {
        return false;
    }
    let Ok(before) = fs::symlink_metadata(path) else {
        return false;
    };
    if !before.file_type().is_socket()
        || (before.uid() != 0 && before.uid() != unsafe { libc::getuid() })
    {
        return false;
    }
    let Some(listeners) = platform::listeners(path) else {
        return false;
    };
    let Ok(after) = fs::symlink_metadata(path) else {
        return false;
    };
    before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.uid() == after.uid()
        && after.file_type().is_socket()
        && matches!(listeners.as_slice(), [name] if known_engine(name))
}

fn known_engine(path: &str) -> bool {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                "dockerd"
                    | "com.docker.backend"
                    | "com.docker.vpnkit"
                    | "colima"
                    | "limactl"
                    | "OrbStack"
                    | "podman"
            )
        })
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_the_named_local_engine_executables_are_accepted() {
        for name in [
            "dockerd",
            "com.docker.backend",
            "com.docker.vpnkit",
            "colima",
            "limactl",
            "OrbStack",
            "podman",
        ] {
            assert!(super::known_engine(&format!("/fixture/{name}")), "{name}");
        }
        for name in ["ssh", "dockerd-forwarder", "other", ""] {
            assert!(!super::known_engine(&format!("/fixture/{name}")), "{name}");
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::{ffi::CStr, mem::size_of, os::unix::ffi::OsStrExt, path::Path};

    // sys/proc_info.h: proc_fileinfo, socket_info and its largest union member.
    // The socket address begins with un_sockinfo's two handles and sockaddr_un.
    #[repr(C)]
    struct SocketFdInfo {
        open_flags: u32,
        status: u32,
        offset: i64,
        file_type: i32,
        guard_flags: u32,
        stat: libc::vinfo_stat,
        socket: u64,
        pcb: u64,
        socket_type: i32,
        protocol: i32,
        family: i32,
        options: i16,
        linger: i16,
        state: i16,
        qlen: i16,
        incqlen: i16,
        qlimit: i16,
        timeout: i16,
        error: u16,
        oobmark: u32,
        buffers: [u8; 48],
        kind: i32,
        reserved: u32,
        protocol_data: [u8; 528],
    }

    fn process_info(pid: i32) -> Option<libc::proc_bsdinfo> {
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = size_of::<libc::proc_bsdinfo>() as i32;
        (unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdinfo).cast(),
                size,
            )
        } == size)
            .then_some(info)
    }

    fn process_is_gone(pid: i32) -> bool {
        (unsafe { libc::kill(pid, 0) }) < 0
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
    }

    fn socket_info(pid: i32, fd: i32) -> Option<SocketFdInfo> {
        let mut info: SocketFdInfo = unsafe { std::mem::zeroed() };
        let size = size_of::<SocketFdInfo>() as i32;
        (unsafe { libc::proc_pidfdinfo(pid, fd, 3, (&mut info as *mut SocketFdInfo).cast(), size) }
            == size)
            .then_some(info)
    }

    fn matches_listener(info: &SocketFdInfo, path: &Path) -> bool {
        if info.family != libc::AF_UNIX
            || info.socket_type != libc::SOCK_STREAM
            || info.kind != 3
            || (info.options as i32 & libc::SO_ACCEPTCONN) == 0
        {
            return false;
        }
        let address = &info.protocol_data[18..18 + 104];
        let Some(end) = address.iter().position(|byte| *byte == 0) else {
            return false;
        };
        &address[..end] == path.as_os_str().as_bytes()
    }

    pub(super) fn inventory() -> Option<Vec<i32>> {
        let mut pids = vec![0i32; 16_384];
        let size = (pids.len() * size_of::<i32>()) as i32;
        let bytes = unsafe { libc::proc_listpids(1, 0, pids.as_mut_ptr().cast(), size) };
        if bytes <= 0 || bytes >= size || !(bytes as usize).is_multiple_of(size_of::<i32>()) {
            return None;
        }
        pids.truncate(bytes as usize / size_of::<i32>());
        // PID 1 may also own the selected listener. Omitting it can turn an
        // ambiguous socket into a false local-engine proof.
        pids.retain(|pid| *pid > 0);
        pids.sort_unstable();
        pids.dedup();
        Some(pids)
    }

    pub(super) fn listeners(path: &Path) -> Option<Vec<String>> {
        let mut listeners = Vec::new();
        for pid in inventory()? {
            let Some(first) = process_info(pid) else {
                if process_is_gone(pid) {
                    continue;
                }
                return None;
            };
            let mut fds: Vec<libc::proc_fdinfo> =
                (0..4096).map(|_| unsafe { std::mem::zeroed() }).collect();
            let size = (fds.len() * size_of::<libc::proc_fdinfo>()) as i32;
            let bytes = unsafe { libc::proc_pidinfo(pid, 1, 0, fds.as_mut_ptr().cast(), size) };
            if bytes <= 0
                || bytes >= size
                || !(bytes as usize).is_multiple_of(size_of::<libc::proc_fdinfo>())
            {
                if process_is_gone(pid) {
                    continue;
                }
                return None;
            }
            fds.truncate(bytes as usize / size_of::<libc::proc_fdinfo>());
            let mut matched = false;
            for fd in fds.into_iter().filter(|fd| fd.proc_fdtype == 2) {
                let Some(info) = socket_info(pid, fd.proc_fd) else {
                    if process_is_gone(pid) {
                        break;
                    }
                    return None;
                };
                if matches_listener(&info, path) {
                    let mut executable = [0u8; 4096];
                    let n = unsafe {
                        libc::proc_pidpath(
                            pid,
                            executable.as_mut_ptr().cast(),
                            executable.len() as u32,
                        )
                    };
                    if n <= 0 || n as usize >= executable.len() {
                        return None;
                    }
                    let executable = CStr::from_bytes_until_nul(&executable)
                        .ok()?
                        .to_str()
                        .ok()?
                        .to_owned();
                    let second = process_info(pid)?;
                    let again = socket_info(pid, fd.proc_fd)?;
                    if first.pbi_start_tvsec != second.pbi_start_tvsec
                        || first.pbi_start_tvusec != second.pbi_start_tvusec
                        || first.pbi_uid != second.pbi_uid
                        || info.socket != again.socket
                        || !matches_listener(&again, path)
                    {
                        return None;
                    }
                    if !matched {
                        listeners.push(executable);
                        matched = true;
                    }
                }
            }
        }
        Some(listeners)
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn system_process_table_is_readable_without_contacting_a_socket() {
            use std::os::{fd::AsRawFd, unix::net::UnixListener};
            let path = std::env::temp_dir()
                .join(format!("arc-listener-table-{}.sock", std::process::id()));
            let listener = UnixListener::bind(&path).unwrap();
            let pid = std::process::id() as i32;
            assert!(super::inventory().unwrap().contains(&pid));
            assert!(super::process_info(pid).is_some());
            let info = super::socket_info(pid, listener.as_raw_fd()).unwrap();
            assert!(super::matches_listener(&info, &path));
            drop(listener);
            std::fs::remove_file(path).unwrap();
            assert_eq!(std::mem::size_of::<super::SocketFdInfo>(), 792);
            assert_eq!(
                std::mem::offset_of!(super::SocketFdInfo, protocol_data),
                264
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use std::path::Path;
    pub(super) fn listeners(_: &Path) -> Option<Vec<String>> {
        None
    }
}
