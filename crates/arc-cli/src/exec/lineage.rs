//! Local lineage sidecar for nested clients that can verify live ancestry.
use arc_core::process_identity::{boot_id, probe_live, IdentityStatus, ProcessIdentity};
use arc_ipc::traffic::LineageContext;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

const MAGIC: &[u8; 8] = b"ARCLIN01";

fn directory(socket: &Path) -> io::Result<&Path> {
    let parent = socket
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "socket has no parent"))?;
    let metadata = fs::symlink_metadata(parent)?;
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o7777 != 0o700
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "socket directory is not private",
        ));
    }
    Ok(parent)
}

fn path(parent: &Path, owner: &ProcessIdentity) -> PathBuf {
    parent.join(format!(
        ".traffic-lineage-{}-{}",
        owner.pid, owner.start_token
    ))
}

pub(super) struct Lineage {
    file: File,
    path: PathBuf,
}
impl Lineage {
    pub(super) fn create(
        socket: &Path,
        owner: &ProcessIdentity,
        context: &LineageContext,
    ) -> io::Result<Self> {
        let parent = directory(socket)?;
        reap_stale(parent);
        let path = path(parent, owner);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(&path)?;
        let result = (|| {
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
            file.write_all(MAGIC)?;
            file.write_all(owner.boot_id.as_bytes())?;
            file.write_all(context.as_str().as_bytes())?;
            file.sync_all()
        })();
        let lineage = Self { file, path };
        result.map(|()| lineage)
    }
}

/// A killed wrapper cannot run Drop. Reap only private records belonging to a
/// definitively gone identity, with a bounded scan and inode recheck.
fn reap_stale(parent: &Path) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.take(512).flatten() {
        let name = entry.file_name();
        let Some((pid_text, start)) = name
            .to_str()
            .and_then(|name| name.strip_prefix(".traffic-lineage-"))
            .and_then(|name| name.split_once('-'))
        else {
            continue;
        };
        let (Ok(pid), Ok(start_value)) = (pid_text.parse::<u32>(), start.parse::<u64>()) else {
            continue;
        };
        if pid == 0
            || start_value == 0
            || pid.to_string() != pid_text
            || start_value.to_string() != start
        {
            continue;
        }
        let Ok(mut file) = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(entry.path())
        else {
            continue;
        };
        let Ok(metadata) = file.metadata() else {
            continue;
        };
        if !metadata.is_file()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o7777 != 0o600
            || metadata.nlink() != 1
            || !matches!(metadata.len(), 0 | 108)
        {
            continue;
        }
        let boot = if metadata.len() == 108 {
            let mut bytes = [0u8; 108];
            if file.read_exact(&mut bytes).is_err() || &bytes[..8] != MAGIC {
                continue;
            }
            let Ok(boot) = std::str::from_utf8(&bytes[8..44]) else {
                continue;
            };
            boot.to_owned()
        } else {
            let Ok(boot) = boot_id() else { continue };
            boot
        };
        let identity = ProcessIdentity {
            pid,
            start_token: start.to_owned(),
            boot_id: boot,
        };
        if !identity.is_valid()
            || !matches!(
                probe_live(&identity),
                IdentityStatus::Missing | IdentityStatus::Mismatch
            )
        {
            continue;
        }
        if let Ok(current) = fs::symlink_metadata(entry.path()) {
            if (metadata.dev(), metadata.ino()) == (current.dev(), current.ino()) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

impl Drop for Lineage {
    fn drop(&mut self) {
        if let (Ok(owned), Ok(current)) = (self.file.metadata(), fs::symlink_metadata(&self.path)) {
            if (owned.dev(), owned.ino()) == (current.dev(), current.ino()) {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

/// Select only a live native ancestor's record. The daemon independently
/// checks ancestry and the active permit before allowing reuse.
pub(super) fn inherited(socket: &Path) -> Option<LineageContext> {
    let parent = directory(socket).ok()?;
    let mut current = ProcessIdentity::capture(std::process::id()).ok()?;
    for _ in 0..128 {
        let pid = parent_pid(current.pid).ok()?;
        if pid == 0 || pid == current.pid || probe_live(&current) != IdentityStatus::VerifiedMatch {
            return None;
        }
        let ancestor = ProcessIdentity::capture(pid).ok()?;
        if let Some(context) = read(parent, &ancestor) {
            if probe_live(&ancestor) == IdentityStatus::VerifiedMatch {
                return Some(context);
            }
        }
        current = ancestor;
    }
    None
}

fn read(parent: &Path, owner: &ProcessIdentity) -> Option<LineageContext> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path(parent, owner))
        .ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
        || metadata.len() != 108
    {
        return None;
    }
    let mut bytes = [0u8; 108];
    file.read_exact(&mut bytes).ok()?;
    if &bytes[..8] != MAGIC || &bytes[8..44] != owner.boot_id.as_bytes() {
        return None;
    }
    LineageContext::try_from(std::str::from_utf8(&bytes[44..]).ok()?.to_owned()).ok()
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
            .ok_or_else(|| io::Error::other("process identity unavailable"))?;
        stat[end + 1..]
            .split_whitespace()
            .nth(1)
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| io::Error::other("process identity unavailable"))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        Err(io::ErrorKind::Unsupported.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::{
            ffi::OsStrExt,
            fs::{symlink, DirBuilderExt, FileTypeExt, PermissionsExt},
        },
        process::Command,
    };

    #[test]
    fn dead_owner_records_are_reaped_without_touching_live_or_symlink_entries() {
        let root = std::env::temp_dir().join(format!(
            "arc-lineage-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let socket = root.join("sock");
        let key = LineageContext::try_from("a".repeat(64)).unwrap();
        let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let dead = ProcessIdentity::capture(child.id()).unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
        let stale = path(&root, &dead);
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(dead.boot_id.as_bytes());
        bytes.extend_from_slice(key.as_str().as_bytes());
        fs::write(&stale, bytes).unwrap();
        fs::set_permissions(&stale, fs::Permissions::from_mode(0o600)).unwrap();
        let empty = root.join(format!(
            ".traffic-lineage-{}-{}",
            dead.pid,
            dead.start_token.parse::<u64>().unwrap() + 1
        ));
        fs::write(&empty, []).unwrap();
        fs::set_permissions(&empty, fs::Permissions::from_mode(0o600)).unwrap();
        let target = root.join("target");
        fs::write(&target, "keep").unwrap();
        let link = root.join(format!(
            ".traffic-lineage-{}-{}",
            dead.pid,
            dead.start_token.parse::<u64>().unwrap() + 2
        ));
        symlink(&target, &link).unwrap();
        let fifo = root.join(format!(
            ".traffic-lineage-{}-{}",
            dead.pid,
            dead.start_token.parse::<u64>().unwrap() + 3
        ));
        let fifo_name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo_name.as_ptr(), 0o600) }, 0);
        let owner = ProcessIdentity::capture(std::process::id()).unwrap();
        let current = Lineage::create(&socket, &owner, &key).unwrap();
        assert!(!stale.exists());
        assert!(!empty.exists());
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo());
        assert_eq!(fs::read(&target).unwrap(), b"keep");
        assert_eq!(read(&root, &owner), Some(key));
        reap_stale(&root);
        assert!(current.path.exists());
        drop(current);
        fs::remove_file(link).unwrap();
        fs::remove_file(fifo).unwrap();
        fs::remove_file(target).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
