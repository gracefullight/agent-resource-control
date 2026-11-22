//! Project identity of one admitted-class job (AT-TRF-08).
//!
//! The key is the SHA-256 digest, as 64 lowercase hexadecimal digits, of the
//! nearest ancestor of the working directory that holds a `.git` entry (a
//! directory or a file, so each worktree is its own project), or of the
//! working directory itself when no ancestor does. It is derived locally from
//! the filesystem, without reading a repository or running a tool. Only the
//! digest leaves this process: no path crosses the wire, enters SQLite or
//! appears in diagnostics.
//!
//! A key is sent only to a daemon that advertised
//! `arc_ipc::traffic::fairness::CAPABILITY`. The daemon advertises by holding
//! a write lock on `fairness::ADVERTISEMENT` beside its socket, so the gate
//! costs no request: an older or scripted peer sees exactly the requests it
//! saw before, and the enqueue without a key is byte-identical to protocol 1.
use arc_ipc::{
    client::Continuity,
    traffic::fairness::{ProjectKey, ADVERTISEMENT},
};
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    os::{
        fd::AsRawFd,
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::Path,
};

const MAX_ANCESTORS: usize = 64;
/// Separates this digest from any other digest of the same path.
const DOMAIN: &[u8] = b"arc.traffic.project.v1\0";

/// The key to send with this process's enqueue, or `None` to send none.
/// `daemon` is the peer that answered this run's capability probe. Without an
/// advertisement held by that very process, or with an underivable key, no
/// key is sent; this never delays or fails the command.
pub(super) fn negotiated(socket: &Path, daemon: &Continuity) -> Option<ProjectKey> {
    let daemon = daemon.identity()?;
    if !advertised(socket, daemon.pid) {
        return None;
    }
    derive(&std::env::current_dir().ok()?)
}

/// Whether process `daemon` holds the advertisement lock beside `socket`.
/// The kernel names the holder; a file nobody locks advertises nothing.
fn advertised(socket: &Path, daemon: u32) -> bool {
    let Some(directory) = socket.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return false;
    };
    let Ok(file) = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(directory.join(ADVERTISEMENT))
    else {
        return false;
    };
    // SAFETY: geteuid has no preconditions.
    if !file
        .metadata()
        .is_ok_and(|m| m.is_file() && m.uid() == unsafe { libc::geteuid() })
    {
        return false;
    }
    // SAFETY: an all-zero flock is valid; fcntl receives a live descriptor
    // and writable storage. F_GETLK takes no lock of its own.
    unsafe {
        let mut lock: libc::flock = std::mem::zeroed();
        lock.l_type = libc::F_RDLCK as _;
        lock.l_whence = libc::SEEK_SET as _;
        libc::fcntl(file.as_raw_fd(), libc::F_GETLK, &mut lock) == 0
            && lock.l_type != libc::F_UNLCK as libc::c_short
            && u32::try_from(lock.l_pid).is_ok_and(|holder| holder == daemon)
    }
}

/// Two working directories under one project root share a key. Symbolic
/// links are resolved first, so every spelling of a directory agrees.
fn derive(working_directory: &Path) -> Option<ProjectKey> {
    let directory = working_directory.canonicalize().ok()?;
    let root = directory
        .ancestors()
        .take(MAX_ANCESTORS)
        .find(|ancestor| ancestor.join(".git").symlink_metadata().is_ok())
        .unwrap_or(&directory);
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update(root.as_os_str().as_bytes());
    let text: String = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    ProjectKey::try_from(text).ok()
}

#[cfg(test)]
mod tests;
