//! Durable evidence written by the child before its native exec.
use arc_ipc::{
    jobs::{JobId, Outcome, ProcessIdentity, SpawnReason},
    traffic::LineageContext,
};
use std::{
    cell::Cell,
    fs::{self, File, OpenOptions},
    io,
    os::{
        fd::AsRawFd,
        unix::{
            fs::{FileExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::Command,
};

const MAGIC: &[u8; 8] = b"ARCWIT03";
const UNSPAWNED: &[u8; 8] = b"ARCNOS03";
const SPAWN_FAILED: &[u8; 8] = b"ARCERR03";
const RECORD_SIZE: usize = 136;

pub(super) struct Witness {
    file: File,
    path: PathBuf,
    initial: [u8; RECORD_SIZE],
    armed: Cell<bool>,
}

impl Witness {
    pub(super) fn create(
        socket: &Path,
        id: &JobId,
        key: &LineageContext,
        owner: &ProcessIdentity,
    ) -> io::Result<Self> {
        let parent = socket
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "socket has no parent"))?;
        let directory = fs::symlink_metadata(parent)?;
        if !directory.is_dir()
            || directory.uid() != unsafe { libc::geteuid() }
            || directory.mode() & 0o7777 != 0o700
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "socket directory is not private",
            ));
        }
        let path = parent.join(format!(".traffic-witness-{}", id.as_str()));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)?;
        let mut initial = [0; RECORD_SIZE];
        initial[..8].copy_from_slice(MAGIC);
        initial[8..16].copy_from_slice(&(owner.pid as u64).to_be_bytes());
        let start: u64 = owner
            .start_token
            .parse()
            .map_err(|_| io::ErrorKind::InvalidInput)?;
        initial[16..24].copy_from_slice(&start.to_be_bytes());
        initial[40..104].copy_from_slice(key.as_str().as_bytes());
        initial[104..].copy_from_slice(id.as_str().as_bytes());
        let witness = Self {
            file,
            path,
            initial,
            armed: Cell::new(false),
        };
        witness
            .file
            .set_permissions(fs::Permissions::from_mode(0o600))?;
        // The inherited lock keeps recovery from reading a partial pre-exec
        // update if the wrapper dies between fork and exec.
        if unsafe { libc::flock(witness.file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::last_os_error());
        }
        witness.file.write_all_at(&witness.initial, 0)?;
        witness.file.sync_all()?;
        File::open(parent)?.sync_all()?;
        Ok(witness)
    }

    pub(super) fn arm(&self) {
        self.armed.set(true);
    }

    pub(super) fn install(&self, command: &mut Command) {
        self.armed.set(true);
        let fd = self.file.as_raw_fd();
        let initial = self.initial;
        // SAFETY: this hook uses only native calls and stack storage after fork.
        // It attempts to persist the identity before the target can exec.
        unsafe {
            command.pre_exec(move || {
                let start = native_start_token()?;
                let mut bytes = initial;
                bytes[24..32].copy_from_slice(&(libc::getpid() as u64).to_be_bytes());
                bytes[32..40].copy_from_slice(&start.to_be_bytes());
                let mut written = 0;
                while written < bytes.len() {
                    let count = libc::pwrite(
                        fd,
                        bytes[written..].as_ptr().cast(),
                        bytes.len() - written,
                        written as libc::off_t,
                    );
                    if count < 0 {
                        let error = io::Error::last_os_error();
                        if error.kind() == io::ErrorKind::Interrupted {
                            continue;
                        }
                        return Err(error);
                    }
                    if count == 0 {
                        return Err(io::Error::from_raw_os_error(libc::EIO));
                    }
                    written += count as usize;
                }
                while libc::fsync(fd) != 0 {
                    let error = io::Error::last_os_error();
                    if error.kind() != io::ErrorKind::Interrupted {
                        return Err(error);
                    }
                }
                Ok(())
            });
        }
    }

    pub(super) fn discard(&self) {
        if let (Ok(owned), Ok(current)) = (self.file.metadata(), fs::symlink_metadata(&self.path)) {
            if (owned.dev(), owned.ino()) == (current.dev(), current.ino()) {
                let _ = fs::remove_file(&self.path);
            }
        }
    }

    pub(super) fn discard_if(&self, confirmed: bool) {
        if confirmed {
            self.discard();
        }
    }

    /// Persist before sending completion or cancellation; either reply can be lost.
    pub(super) fn record_unspawned(&self) -> io::Result<()> {
        self.armed.set(true);
        let mut bytes = self.initial;
        bytes[..8].copy_from_slice(UNSPAWNED);
        self.file.write_all_at(&bytes, 0)?;
        self.file.set_len(bytes.len() as u64)?;
        self.file.sync_all()
    }

    pub(super) fn finish_unspawned(&self, confirm: impl FnOnce() -> bool) {
        if self.record_unspawned().is_err() {
            super::preparation::best_effort(
                "SPAWN_EVIDENCE_UNAVAILABLE\nNo child was started; recording is unconfirmed.",
            );
        }
        self.discard_if(confirm());
    }

    pub(super) fn finish_spawn_failed(&self, outcome: &Outcome, confirm: impl FnOnce() -> bool) {
        self.armed.set(true);
        let mut bytes = self.initial;
        bytes[..8].copy_from_slice(SPAWN_FAILED);
        if let Outcome::SpawnFailed { reason, os_code } = outcome {
            let code: u64 = match reason {
                SpawnReason::NotFound => 1,
                SpawnReason::PermissionDenied => 2,
                SpawnReason::InvalidInput => 3,
                SpawnReason::ResourceUnavailable => 4,
                SpawnReason::Other => 5,
            };
            bytes[24..32].copy_from_slice(&code.to_be_bytes());
            if let Some(code) = os_code {
                bytes[32..36].copy_from_slice(&1u32.to_be_bytes());
                bytes[36..40].copy_from_slice(&code.to_be_bytes());
            }
            if self
                .file
                .write_all_at(&bytes, 0)
                .and_then(|()| self.file.sync_all())
                .is_err()
            {
                super::preparation::best_effort(
                    "SPAWN_EVIDENCE_UNAVAILABLE\nNo child was started; recording is unconfirmed.",
                );
            }
        }
        self.discard_if(confirm());
    }
}
impl Drop for Witness {
    fn drop(&mut self) {
        if !self.armed.get() {
            self.discard();
        }
    }
}

#[cfg(target_os = "macos")]
unsafe fn native_start_token() -> io::Result<u64> {
    let mut info: libc::proc_bsdinfo = std::mem::zeroed();
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    if libc::proc_pidinfo(
        libc::getpid(),
        libc::PROC_PIDTBSDINFO,
        0,
        (&mut info as *mut libc::proc_bsdinfo).cast(),
        size,
    ) != size
    {
        return Err(io::Error::last_os_error());
    }
    info.pbi_start_tvsec
        .checked_mul(1_000_000)
        .and_then(|seconds| seconds.checked_add(info.pbi_start_tvusec))
        .filter(|start| *start > 0)
        .ok_or_else(|| io::Error::from_raw_os_error(libc::EIO))
}

#[cfg(test)]
mod tests {
    use super::{Witness, MAGIC, RECORD_SIZE, UNSPAWNED};
    use arc_core::process_identity::ProcessIdentity;
    use arc_ipc::{jobs::JobId, traffic::LineageContext};
    use std::{
        cell::Cell,
        fs::{self, File},
        os::unix::fs::{DirBuilderExt, PermissionsExt},
        process::Command,
    };

    #[test]
    fn a_witness_write_failure_prevents_native_execution() {
        let witness = Witness {
            file: File::open("/dev/null").unwrap(),
            path: "/dev/null".into(),
            initial: [0; RECORD_SIZE],
            armed: Cell::new(false),
        };
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exit 17"]);
        witness.install(&mut command);
        assert!(command.spawn().is_err());
    }

    #[test]
    fn unconfirmed_cancel_keeps_no_spawn_evidence_but_confirmed_cancel_removes_it() {
        let root = std::env::temp_dir().join(format!(
            "arc-no-spawn-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let key = LineageContext::try_from("b".repeat(64)).unwrap();
        let id = JobId::try_from("1234567890abcdef1234567890abcdef".to_owned()).unwrap();
        let owner = ProcessIdentity::capture(std::process::id()).unwrap();
        let witness = Witness::create(&root.join("sock"), &id, &key, &owner).unwrap();
        witness.finish_unspawned(|| false);
        let path = witness.path.clone();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(&bytes[..8], UNSPAWNED);
        assert_eq!(
            u64::from_be_bytes(bytes[8..16].try_into().unwrap()),
            owner.pid as u64
        );
        assert_eq!(
            u64::from_be_bytes(bytes[16..24].try_into().unwrap()).to_string(),
            owner.start_token
        );
        assert!(bytes[24..40].iter().all(|byte| *byte == 0));
        assert_eq!(&bytes[40..104], key.as_str().as_bytes());
        assert_eq!(&bytes[104..], id.as_str().as_bytes());
        drop(witness);
        assert!(path.exists());
        fs::remove_file(&path).unwrap();
        let confirmed = Witness::create(&root.join("sock"), &id, &key, &owner).unwrap();
        confirmed.finish_unspawned(|| true);
        assert!(!path.exists());
        drop(confirmed);
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn child_witness_contains_its_identity_and_is_removed_only_when_safe() {
        let root = std::env::temp_dir().join(format!(
            "arc-witness-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let id = JobId::try_from("1234567890abcdef1234567890abcdef".to_owned()).unwrap();
        let key = LineageContext::try_from("a".repeat(64)).unwrap();
        let socket = root.join("sock");
        let owner = ProcessIdentity::capture(std::process::id()).unwrap();
        let witness = Witness::create(&socket, &id, &key, &owner).unwrap();
        let before_spawn = fs::read(&witness.path).unwrap();
        assert_eq!(&before_spawn[..8], MAGIC);
        assert_eq!(
            u64::from_be_bytes(before_spawn[8..16].try_into().unwrap()),
            owner.pid as u64
        );
        assert_eq!(
            u64::from_be_bytes(before_spawn[16..24].try_into().unwrap()).to_string(),
            owner.start_token
        );
        assert!(before_spawn[24..40].iter().all(|byte| *byte == 0));
        assert!(Witness::create(&socket, &id, &key, &owner).is_err());
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        witness.install(&mut command);
        let mut child = command.spawn().unwrap();
        let identity = ProcessIdentity::capture(child.id()).unwrap();
        let bytes = fs::read(&witness.path).unwrap();
        assert_eq!(&bytes[..8], MAGIC);
        assert_eq!(
            u64::from_be_bytes(bytes[24..32].try_into().unwrap()),
            identity.pid as u64
        );
        assert_eq!(
            u64::from_be_bytes(bytes[32..40].try_into().unwrap()).to_string(),
            identity.start_token
        );
        assert_eq!(&bytes[40..104], key.as_str().as_bytes());
        assert_eq!(&bytes[104..], id.as_str().as_bytes());
        child.kill().unwrap();
        child.wait().unwrap();
        let path = witness.path.clone();
        drop(witness);
        assert!(path.exists(), "armed evidence survives its owner");
        fs::remove_file(path).unwrap();
        let unused = Witness::create(&socket, &id, &key, &owner).unwrap();
        let unused_path = unused.path.clone();
        drop(unused);
        assert!(!unused_path.exists());
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(Witness::create(&socket, &id, &key, &owner).is_err());
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn spawn_failure_outcome_is_durable_before_completion_delivery() {
        use arc_ipc::jobs::{Outcome, SpawnReason};
        let root = std::env::temp_dir().join(format!(
            "arc-failed-witness-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let id = JobId::try_from("1234567890abcdef1234567890abcdef".to_owned()).unwrap();
        let key = LineageContext::try_from("c".repeat(64)).unwrap();
        let owner = ProcessIdentity::capture(std::process::id()).unwrap();
        let witness = Witness::create(&root.join("sock"), &id, &key, &owner).unwrap();
        for (reason, expected, os_code) in [
            (SpawnReason::NotFound, 1u64, Some(libc::ENOENT)),
            (SpawnReason::PermissionDenied, 2, Some(libc::EACCES)),
            (SpawnReason::InvalidInput, 3, None),
            (SpawnReason::ResourceUnavailable, 4, Some(libc::EAGAIN)),
            (SpawnReason::Other, 5, Some(-7)),
        ] {
            let outcome = Outcome::SpawnFailed { reason, os_code };
            witness.finish_spawn_failed(&outcome, || {
                let bytes = fs::read(&witness.path).unwrap();
                assert_eq!(&bytes[..8], b"ARCERR03");
                assert_eq!(
                    u64::from_be_bytes(bytes[24..32].try_into().unwrap()),
                    expected
                );
                assert_eq!(
                    u32::from_be_bytes(bytes[32..36].try_into().unwrap()),
                    u32::from(os_code.is_some())
                );
                assert_eq!(
                    i32::from_be_bytes(bytes[36..40].try_into().unwrap()),
                    os_code.unwrap_or(0)
                );
                false
            });
        }
        witness.discard();
        drop(witness);
        fs::remove_dir(root).unwrap();
    }
}

#[cfg(target_os = "linux")]
unsafe fn native_start_token() -> io::Result<u64> {
    let fd = libc::open(
        c"/proc/self/stat".as_ptr(),
        libc::O_RDONLY | libc::O_CLOEXEC,
    );
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut bytes = [0u8; 4096];
    let count = libc::read(fd, bytes.as_mut_ptr().cast(), bytes.len());
    libc::close(fd);
    if count <= 0 {
        return Err(io::Error::from_raw_os_error(libc::EIO));
    }
    let stat = &bytes[..count as usize];
    let Some(end) = stat.iter().rposition(|byte| *byte == b')') else {
        return Err(io::Error::from_raw_os_error(libc::EIO));
    };
    let Some(token) = stat[end + 1..]
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|field| !field.is_empty())
        .nth(19)
    else {
        return Err(io::Error::from_raw_os_error(libc::EIO));
    };
    let mut value = 0u64;
    for &digit in token {
        if !digit.is_ascii_digit() {
            return Err(io::Error::from_raw_os_error(libc::EIO));
        }
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add((digit - b'0') as u64))
            .ok_or_else(|| io::Error::from_raw_os_error(libc::EIO))?;
    }
    (value > 0)
        .then_some(value)
        .ok_or_else(|| io::Error::from_raw_os_error(libc::EIO))
}
