// Private inherited markers identify fixture descendants after reparenting.
use arc_core::process_identity::ProcessIdentity;
use std::{
    fs::{File, OpenOptions},
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{fs::OpenOptionsExt, process::CommandExt},
    },
    path::PathBuf,
    process::Command,
};

pub struct Marker {
    path: PathBuf,
    file: Option<File>,
    retain: bool,
}

impl Marker {
    pub fn attach(command: &mut Command) -> io::Result<Self> {
        // A paired comparison can explicitly use the same marker path in
        // sequential commands, preserving their exact child environments.
        let configured = command
            .get_envs()
            .find(|(key, _)| *key == "ARC_TEST_OWNERSHIP")
            .and_then(|(_, value)| value)
            .map(PathBuf::from);
        let path = match configured {
            Some(path) => path,
            None => std::env::temp_dir().join(format!(
                "arc-owned-{}",
                arc_core::process_identity::random_job_id()?
            )),
        };
        let initial = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        // Stay clear of the hook worker's reserved descriptor and ordinary
        // fixture shell descriptors without changing the parent process flags.
        let marker_fd = unsafe { libc::fcntl(initial.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 64) };
        if marker_fd < 0 {
            let error = io::Error::last_os_error();
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
        let file = unsafe { File::from_raw_fd(marker_fd) };
        drop(initial);
        let fd = file.as_raw_fd();
        if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = io::Error::last_os_error();
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
        // Only the forked child clears close-on-exec, so unrelated concurrent
        // spawns from the test process do not inherit this ownership marker.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        command.env("ARC_TEST_OWNERSHIP", &path);
        Ok(Self {
            path,
            file: Some(file),
            retain: false,
        })
    }

    pub fn close_parent(&mut self) {
        self.file.take();
    }

    pub fn identities(&self) -> io::Result<Vec<ProcessIdentity>> {
        // An inherited exclusive flock disappears with the last inherited
        // descriptor. The environment marker covers a child that closed it.
        let probe = File::open(&self.path)?;
        let unlocked =
            unsafe { libc::flock(probe.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
        if !unlocked {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::EWOULDBLOCK) {
                return Err(error);
            }
        }
        let mut pids = if unlocked {
            Vec::new()
        } else {
            self.members()?
        };
        pids.extend(environment::members(&self.path)?);
        pids.sort_unstable();
        pids.dedup();
        let mut captured = Vec::new();
        for pid in pids.into_iter().filter(|pid| *pid != std::process::id()) {
            match ProcessIdentity::capture(pid) {
                Ok(identity) => captured.push(identity),
                Err(_)
                    if unsafe { libc::kill(pid as i32, 0) } != 0
                        && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) => {}
                Err(error) => return Err(error),
            }
        }
        // Recheck both the marker and native start identity before trusting a
        // PID; neither an exited owner nor a reused PID authorizes signalling.
        let members = if unlocked {
            Vec::new()
        } else {
            self.members()?
        };
        let mut verified = Vec::new();
        for identity in captured {
            if (members.contains(&identity.pid) || environment::matches(identity.pid, &self.path)?)
                && ProcessIdentity::capture(identity.pid).ok().as_ref() == Some(&identity)
            {
                verified.push(identity);
            }
        }
        Ok(verified)
    }

    pub fn retain_on_drop(&mut self) {
        self.retain = true;
    }

    pub fn remove_on_drop(&mut self) {
        self.retain = false;
    }

    #[cfg(test)]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    #[cfg(test)]
    pub fn is_retained(&self) -> bool {
        self.retain
    }

    #[cfg(target_os = "macos")]
    fn members(&self) -> io::Result<Vec<u32>> {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        unsafe extern "C" {
            fn proc_listpidspath(
                kind: u32,
                info: u32,
                path: *const libc::c_char,
                flags: u32,
                buffer: *mut libc::c_void,
                size: libc::c_int,
            ) -> libc::c_int;
        }
        let path = CString::new(self.path.as_os_str().as_bytes()).map_err(io::Error::other)?;
        let mut ids = vec![0i32; 128];
        loop {
            let bytes = unsafe {
                proc_listpidspath(
                    1,
                    0,
                    path.as_ptr(),
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
                return Ok(ids[..count]
                    .iter()
                    .copied()
                    .filter(|pid| *pid > 0)
                    .map(|pid| pid as u32)
                    .collect());
            }
            if ids.len() >= 16_384 {
                return Err(io::Error::other("owned marker process limit"));
            }
            ids.resize(ids.len() * 2, 0);
        }
    }

    #[cfg(target_os = "linux")]
    fn members(&self) -> io::Result<Vec<u32>> {
        use std::os::unix::fs::MetadataExt;
        let marker = std::fs::metadata(&self.path)?;
        let mut found = Vec::new();
        for entry in std::fs::read_dir("/proc")? {
            let entry = entry?;
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<u32>().ok())
            else {
                continue;
            };
            let Ok(fds) = std::fs::read_dir(entry.path().join("fd")) else {
                continue;
            };
            if fds.flatten().any(|fd| {
                std::fs::metadata(fd.path())
                    .is_ok_and(|m| m.dev() == marker.dev() && m.ino() == marker.ino())
            }) {
                found.push(pid);
            }
        }
        Ok(found)
    }
}

impl Drop for Marker {
    fn drop(&mut self) {
        if self.retain {
            eprintln!(
                "fixture ownership evidence retained at {}",
                self.path.display()
            );
        } else {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

mod environment {
    include!("owned_environment.rs");
}
