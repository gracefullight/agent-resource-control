//! Ephemeral native ancestry evidence for terminal job accounting.
//!
//! A private record is a claim linked to a live native ancestor, never authority
//! to accept a report. The daemon must independently bind observer, owned child
//! and root to durable jobs while those identities can still be verified.
use arc_core::{
    job_meter::{valid_id, AccountingRelation},
    process_identity::{probe_live, IdentityStatus, ProcessIdentity},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

const MAGIC: &[u8; 8] = b"ARCMTR01";
const SIZE: usize = 109;

fn directory(socket: &Path) -> io::Result<&Path> {
    let parent = socket
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or(io::ErrorKind::InvalidInput)?;
    let meta = fs::symlink_metadata(parent)?;
    if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o7777 != 0o700 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(parent)
}
fn path(parent: &Path, owner: &ProcessIdentity) -> PathBuf {
    parent.join(format!(
        ".job-meter-lineage-{}-{}",
        owner.pid, owner.start_token
    ))
}

/// Capture and recheck every userspace edge, stopping at the kernel's PID-1
/// boundary without reading the protected launchd/init identity. PID 1 cannot
/// be a managed CLI wrapper. Missing/reparented/inaccessible earlier edges fail.
pub fn ancestors(observer: &ProcessIdentity) -> io::Result<Vec<ProcessIdentity>> {
    let mut chain = Vec::new();
    walk(observer, |identity| {
        chain.push(identity.clone());
        false
    })?;
    Ok(chain)
}

fn walk(
    observer: &ProcessIdentity,
    mut visit: impl FnMut(&ProcessIdentity) -> bool,
) -> io::Result<()> {
    if !observer.is_valid() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut chain = Vec::new();
    let mut current = observer.clone();
    for _ in 0..128 {
        if probe_live(&current) != IdentityStatus::VerifiedMatch {
            return Err(io::Error::other("native ancestry unavailable"));
        }
        if current.pid == 1 {
            return Ok(());
        }
        let parent = parent_pid(current.pid)?;
        if parent == 0
            || parent == current.pid
            || chain.iter().any(|p: &ProcessIdentity| p.pid == parent)
        {
            return Err(io::Error::other("native ancestry unavailable"));
        }
        // The verified current process's native ppid establishes this boundary;
        // no privileged lookup of launchd/init itself is necessary.
        if parent == 1 {
            if probe_live(&current) == IdentityStatus::VerifiedMatch
                && parent_pid(current.pid)? == 1
            {
                return Ok(());
            }
            return Err(io::Error::other("native ancestry changed"));
        }
        let identity = ProcessIdentity::capture(parent).map_err(|error| {
            io::Error::new(error.kind(), format!("ancestor identity {parent}: {error}"))
        })?;
        if probe_live(&current) != IdentityStatus::VerifiedMatch
            || parent_pid(current.pid)? != parent
            || identity.boot_id != observer.boot_id
        {
            return Err(io::Error::other("native ancestry changed"));
        }
        chain.push(identity.clone());
        if visit(&identity) {
            return Ok(());
        }
        current = identity;
    }
    Err(io::Error::other("native ancestry limit"))
}

/// Resolve a cross-mode claim before spawn/attachment. An opaque Traffic token
/// is deliberately insufficient; legacy Traffic-only ancestors stay unverified.
pub fn relation(socket: &Path, observer: &ProcessIdentity, job_id: &str) -> AccountingRelation {
    let unverified = AccountingRelation::Unverified {};
    if !valid_id(job_id) {
        return unverified;
    }
    let Ok(parent) = directory(socket) else {
        return unverified;
    };
    let mut legacy = false;
    let mut found = None;
    let result = walk(observer, |ancestor| match read(parent, ancestor) {
        Ok(Some((ancestor_job, Some(root)))) if ancestor_job != job_id && root != job_id => {
            found = Some(if probe_live(ancestor) == IdentityStatus::VerifiedMatch {
                AccountingRelation::IncludedInRoot { root_job_id: root }
            } else {
                AccountingRelation::Unverified {}
            });
            true
        }
        Ok(None) => {
            legacy |= fs::symlink_metadata(parent.join(format!(
                ".traffic-lineage-{}-{}",
                ancestor.pid, ancestor.start_token
            )))
            .is_ok();
            false
        }
        _ => {
            found = Some(AccountingRelation::Unverified {});
            true
        }
    });
    if result.is_err() {
        return unverified;
    }
    if let Some(relation) = found {
        return relation;
    }
    if legacy {
        unverified
    } else {
        AccountingRelation::CanonicalRoot {
            root_job_id: job_id.to_owned(),
        }
    }
}

/// Kept alive through terminal reporting; removal checks the owned inode. Stale
/// files cannot match a recycled PID because both start token and boot must match.
pub struct Record {
    file: File,
    path: PathBuf,
}
impl Record {
    pub fn create(
        socket: &Path,
        observer: &ProcessIdentity,
        job_id: &str,
        relation: &AccountingRelation,
    ) -> io::Result<Self> {
        if !observer.is_valid() || !valid_id(job_id) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let root = match relation {
            AccountingRelation::CanonicalRoot { root_job_id } if root_job_id == job_id => {
                Some(root_job_id)
            }
            AccountingRelation::IncludedInRoot { root_job_id }
                if root_job_id != job_id && valid_id(root_job_id) =>
            {
                Some(root_job_id)
            }
            AccountingRelation::Unverified {} => None,
            _ => return Err(io::ErrorKind::InvalidInput.into()),
        };
        let path = path(directory(socket)?, observer);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(&path)?;
        let mut record = Self { file, path };
        record
            .file
            .set_permissions(fs::Permissions::from_mode(0o600))?;
        record.file.write_all(MAGIC)?;
        record.file.write_all(observer.boot_id.as_bytes())?;
        record.file.write_all(job_id.as_bytes())?;
        record.file.write_all(&[u8::from(root.is_some())])?;
        record.file.write_all(
            root.map_or(b"--------------------------------".as_slice(), |r| {
                r.as_bytes()
            }),
        )?;
        Ok(record)
    }
}
impl Drop for Record {
    fn drop(&mut self) {
        if let (Ok(owned), Ok(current)) = (self.file.metadata(), fs::symlink_metadata(&self.path)) {
            if (owned.dev(), owned.ino()) == (current.dev(), current.ino()) {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}
fn read(parent: &Path, observer: &ProcessIdentity) -> io::Result<Option<(String, Option<String>)>> {
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path(parent, observer))
    {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let meta = file.metadata()?;
    if !meta.is_file()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o7777 != 0o600
        || meta.nlink() != 1
        || meta.len() != SIZE as u64
    {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut bytes = [0; SIZE];
    file.read_exact(&mut bytes)?;
    let text = |r| {
        std::str::from_utf8(r)
            .map(str::to_owned)
            .map_err(|_| io::ErrorKind::InvalidData)
    };
    let job = text(&bytes[44..76])?;
    let root = text(&bytes[77..109])?;
    if &bytes[..8] != MAGIC || &bytes[8..44] != observer.boot_id.as_bytes() || !valid_id(&job) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let root = match bytes[76] {
        0 if root == "--------------------------------" => None,
        1 if valid_id(&root) => Some(root),
        _ => return Err(io::ErrorKind::InvalidData.into()),
    };
    Ok(Some((job, root)))
}
fn parent_pid(pid: u32) -> io::Result<u32> {
    #[cfg(target_os = "macos")]
    {
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of_val(&info) as libc::c_int;
        if unsafe {
            libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdinfo).cast(),
                size,
            )
        } != size
        {
            return Err(io::Error::last_os_error());
        }
        Ok(info.pbi_ppid)
    }
    #[cfg(target_os = "linux")]
    {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
        let end = stat
            .rfind(')')
            .ok_or_else(|| io::Error::other("native ancestry unavailable"))?;
        stat[end + 1..]
            .split_whitespace()
            .nth(1)
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| io::Error::other("native ancestry unavailable"))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        Err(io::ErrorKind::Unsupported.into())
    }
}

#[cfg(test)]
#[path = "job_lineage_tests.rs"]
mod tests;
