//! Read owner-only pre-exec witnesses without trusting their paths or contents.
use arc_ipc::{
    jobs::{Outcome, ProcessIdentity, SpawnReason},
    traffic::LineageContext,
};
use arc_storage::jobs::traffic::RecoveryCandidate;
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};

pub(super) enum Proof {
    Child(ProcessIdentity),
    Unspawned,
    SpawnFailed(Outcome),
}

pub(super) struct Receipt {
    file: File,
    path: PathBuf,
    key: LineageContext,
    proof: Proof,
}
impl Receipt {
    pub(super) fn key(&self) -> &LineageContext {
        &self.key
    }
    pub(super) fn proof(&self) -> &Proof {
        &self.proof
    }
    pub(super) fn discard(self) {
        if let (Ok(owned), Ok(current)) = (self.file.metadata(), fs::symlink_metadata(&self.path)) {
            if (owned.dev(), owned.ino()) == (current.dev(), current.ino()) {
                let _ = fs::remove_file(self.path);
            }
        }
    }
}

pub(super) fn read(candidate: &RecoveryCandidate) -> Option<Receipt> {
    read_at(&arc_core::socket_path().ok()?, candidate)
}

pub(super) fn read_at(socket: &Path, candidate: &RecoveryCandidate) -> Option<Receipt> {
    let key = candidate.witness_context.as_ref()?;
    let parent = socket.parent()?;
    let directory = fs::symlink_metadata(parent).ok()?;
    let uid = unsafe { libc::geteuid() };
    if !directory.is_dir() || directory.uid() != uid || directory.mode() & 0o7777 != 0o700 {
        return None;
    }
    let path = parent.join(format!(
        ".traffic-witness-{}",
        candidate.job.job_id.as_str()
    ));
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)
        .ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
        || !matches!(metadata.len(), 88 | 104 | 136)
    {
        return None;
    }
    // A forked pre-exec child inherits the writer's lock until exec closes the
    // descriptor. Never read a record while that child can still update it.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return None;
    }
    let mut bytes = vec![0u8; metadata.len() as usize];
    file.read_exact(&mut bytes).ok()?;
    let (proof, pid, start) = if bytes.len() >= 104 {
        if &bytes[40..104] != key.as_str().as_bytes()
            || (bytes.len() == 136 && &bytes[104..] != candidate.job.job_id.as_str().as_bytes())
            || u64::from_be_bytes(bytes[8..16].try_into().ok()?)
                != candidate.job.observer.pid as u64
            || u64::from_be_bytes(bytes[16..24].try_into().ok()?).to_string()
                != candidate.job.observer.start_token
        {
            return None;
        }
        let pid = u64::from_be_bytes(bytes[24..32].try_into().ok()?);
        let start = u64::from_be_bytes(bytes[32..40].try_into().ok()?);
        let proof = match &bytes[..8] {
            b"ARCWIT02" if bytes.len() == 104 && pid == 0 && start == 0 => Some(Proof::Unspawned),
            b"ARCWIT02" if bytes.len() == 104 && pid > 0 && start > 0 => None,
            b"ARCNOS02" if bytes.len() == 104 && pid == 0 && start == 0 => Some(Proof::Unspawned),
            b"ARCWIT03" if bytes.len() == 136 && pid == 0 && start == 0 => Some(Proof::Unspawned),
            b"ARCWIT03" if bytes.len() == 136 && pid > 0 && start > 0 => None,
            b"ARCNOS03" if bytes.len() == 136 && pid == 0 && start == 0 => Some(Proof::Unspawned),
            b"ARCERR02" if bytes.len() == 104 => {
                let reason = match pid {
                    1 => SpawnReason::NotFound,
                    2 => SpawnReason::PermissionDenied,
                    3 => SpawnReason::InvalidInput,
                    4 => SpawnReason::ResourceUnavailable,
                    5 => SpawnReason::Other,
                    _ => return None,
                };
                let present = u32::from_be_bytes(bytes[32..36].try_into().ok()?);
                let code = i32::from_be_bytes(bytes[36..40].try_into().ok()?);
                let os_code = match present {
                    0 if code == 0 => None,
                    1 => Some(code),
                    _ => return None,
                };
                Some(Proof::SpawnFailed(Outcome::SpawnFailed { reason, os_code }))
            }
            b"ARCERR03" if bytes.len() == 136 => {
                let reason = match pid {
                    1 => SpawnReason::NotFound,
                    2 => SpawnReason::PermissionDenied,
                    3 => SpawnReason::InvalidInput,
                    4 => SpawnReason::ResourceUnavailable,
                    5 => SpawnReason::Other,
                    _ => return None,
                };
                let present = u32::from_be_bytes(bytes[32..36].try_into().ok()?);
                let code = i32::from_be_bytes(bytes[36..40].try_into().ok()?);
                let os_code = match present {
                    0 if code == 0 => None,
                    1 => Some(code),
                    _ => return None,
                };
                Some(Proof::SpawnFailed(Outcome::SpawnFailed { reason, os_code }))
            }
            _ => return None,
        };
        (proof, pid, start)
    } else {
        if &bytes[24..] != key.as_str().as_bytes() {
            return None;
        }
        let pid = u64::from_be_bytes(bytes[8..16].try_into().ok()?);
        let start = u64::from_be_bytes(bytes[16..24].try_into().ok()?);
        let proof = match &bytes[..8] {
            b"ARCNOS01" if pid == 0 && start == 0 => Some(Proof::Unspawned),
            b"ARCWIT01" if pid > 0 && start > 0 => None,
            _ => return None,
        };
        (proof, pid, start)
    };
    if let Some(proof) = proof {
        return Some(Receipt {
            file,
            path,
            key: key.clone(),
            proof,
        });
    }
    let child = ProcessIdentity {
        pid: u32::try_from(pid).ok()?,
        start_token: start.to_string(),
        boot_id: candidate.job.observer.boot_id.clone(),
    };
    if !child.is_valid() {
        return None;
    }
    Some(Receipt {
        file,
        path,
        key: key.clone(),
        proof: Proof::Child(child),
    })
}
