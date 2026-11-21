//! Native evidence for a retained, direct child. This is not an authority to signal.
//!
//! A caller must additionally hold exclusive ownership of the child's wait,
//! establish an ended owner session, fence leases and policy, and check the
//! native evidence again immediately before every signal.
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use std::{io, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildIdentity {
    pub process: ProcessIdentity,
    pub parent_pid: u32,
    pub executable: PathBuf,
    pub executable_device: u64,
    pub executable_inode: u64,
    pub real_uid: u32,
    pub effective_uid: u32,
}

/// Capture executable, credentials and direct parent for a live identity.
/// A missing or inaccessible native observation never becomes ownership.
pub fn capture(process: &ProcessIdentity) -> io::Result<ChildIdentity> {
    native::capture(process)
}

/// Whether the process group holds its controlling terminal's foreground.
/// An unreadable observation is an error: callers must not treat it as `false`.
pub fn foreground(process: &ProcessIdentity) -> io::Result<bool> {
    native::foreground(process)
}

/// Freshly check all birth evidence, including the current-user boundary.
pub fn matches(recorded: &ChildIdentity, parent_pid: u32) -> bool {
    recorded.parent_pid == parent_pid
        && recorded.real_uid == unsafe { libc::getuid() }
        && recorded.effective_uid == unsafe { libc::geteuid() }
        && probe_live(&recorded.process) == IdentityStatus::VerifiedMatch
        && capture(&recorded.process).is_ok_and(|current| current == *recorded)
        && probe_live(&recorded.process) == IdentityStatus::VerifiedMatch
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use std::{
        ffi::OsString,
        os::unix::{ffi::OsStringExt, fs::MetadataExt},
    };

    pub(super) fn capture(process: &ProcessIdentity) -> io::Result<ChildIdentity> {
        if probe_live(process) != IdentityStatus::VerifiedMatch {
            return Err(io::Error::other("live process identity unavailable"));
        }
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of_val(&info) as i32;
        if unsafe {
            libc::proc_pidinfo(
                process.pid as i32,
                libc::PROC_PIDTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdinfo).cast(),
                size,
            )
        } != size
            || info.pbi_pid != process.pid
            || info.pbi_ppid == 0
        {
            return Err(io::Error::other(
                "process credentials or parent unavailable",
            ));
        }
        let mut path = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        let length = unsafe {
            libc::proc_pidpath(
                process.pid as i32,
                path.as_mut_ptr().cast(),
                path.len() as u32,
            )
        };
        if length <= 0 || length as usize >= path.len() {
            return Err(io::Error::other("process executable unavailable"));
        }
        path.truncate(length as usize);
        if path.last() == Some(&0) {
            path.pop();
        }
        if path.is_empty() || path.contains(&0) {
            return Err(io::Error::other("invalid process executable"));
        }
        let executable = PathBuf::from(OsString::from_vec(path));
        let metadata = std::fs::metadata(&executable)?;
        if !metadata.is_file() || probe_live(process) != IdentityStatus::VerifiedMatch {
            return Err(io::Error::other("process executable changed"));
        }
        Ok(ChildIdentity {
            process: process.clone(),
            parent_pid: info.pbi_ppid,
            executable,
            executable_device: metadata.dev(),
            executable_inode: metadata.ino(),
            real_uid: info.pbi_ruid,
            effective_uid: info.pbi_uid,
        })
    }

    pub(super) fn foreground(process: &ProcessIdentity) -> io::Result<bool> {
        if probe_live(process) != IdentityStatus::VerifiedMatch {
            return Err(io::Error::other("live process identity unavailable"));
        }
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of_val(&info) as i32;
        let read = unsafe {
            libc::proc_pidinfo(
                process.pid as i32,
                libc::PROC_PIDTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdinfo).cast(),
                size,
            )
        };
        if read != size
            || info.pbi_pid != process.pid
            || probe_live(process) != IdentityStatus::VerifiedMatch
        {
            return Err(io::Error::other("process terminal state unavailable"));
        }
        // NODEV means no controlling terminal; otherwise compare its
        // foreground process group with this process's own group.
        Ok(info.e_tdev != u32::MAX && info.e_tpgid != 0 && info.e_tpgid == info.pbi_pgid)
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "../../arc-daemon/tests/support/instrumentation.rs"]
mod instrumentation;

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };

    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }

    #[test]
    fn fresh_direct_child_evidence_rejects_stale_identity_and_changed_fields() {
        let home = std::env::temp_dir().join(format!(
            "arc-reaper-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        std::fs::create_dir(&home).unwrap();
        let mut command = Command::new("/bin/sleep");
        command
            .arg("120")
            .env_clear()
            .env("HOME", home)
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        instrumentation::forward(&mut command);
        let mut child = Child(command.spawn().unwrap());
        let deadline = Instant::now() + Duration::from_secs(60);
        let process = loop {
            match ProcessIdentity::capture(child.0.id()) {
                Ok(identity) => break identity,
                Err(error) => {
                    assert!(
                        Instant::now() < deadline,
                        "child identity unavailable: {error}"
                    );
                    std::thread::yield_now();
                }
            }
        };
        let captured = capture(&process).unwrap();
        assert!(matches(&captured, std::process::id()));
        assert!(!matches(&captured, std::process::id().saturating_add(1)));
        let mut changed = captured.clone();
        changed.executable_inode ^= 1;
        assert!(!matches(&changed, std::process::id()));
        let mut changed = captured.clone();
        changed.effective_uid ^= 1;
        assert!(!matches(&changed, std::process::id()));
        let mut changed = process.clone();
        changed.start_token = (changed.start_token.parse::<u64>().unwrap() + 1).to_string();
        assert!(capture(&changed).is_err());
        assert!(foreground(&changed).is_err());
        // The fixture has no terminal on any standard stream and may share the
        // runner's group; only a readable observation is asserted here.
        assert!(foreground(&process).is_ok());
        assert!(child.0.try_wait().unwrap().is_none());
    }
}

#[cfg(not(target_os = "macos"))]
mod native {
    use super::*;
    pub(super) fn capture(_: &ProcessIdentity) -> io::Result<ChildIdentity> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native reaper identity unavailable",
        ))
    }
    pub(super) fn foreground(_: &ProcessIdentity) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native terminal state unavailable",
        ))
    }
}
