//! The user-owned, bounded directory that holds project analysis entries.
//!
//! The directory is `analysis-cache` in the user's default ARC data directory,
//! independent of database overrides. If it is unsafe, an advertising daemon
//! may keep the same bounded entries in memory. The directory is never under
//! /tmp, /var/tmp or TMPDIR, and must be user-owned and owner-only.
//! There is one entry per project root, named by a domain-separated digest
//! of the canonical root; it holds no absolute path, script body or endpoint
//! value.
use arc_detector::scripts::cache::{Entry, MAX_ENTRIES, MAX_ENTRY_BYTES};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::{self, DirBuilder, File, OpenOptions},
    io::{Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
    },
    path::{Path, PathBuf},
};

pub(crate) const DIRECTORY: &str = "analysis-cache";
/// Directory entries examined when evicting.
const MAX_SCAN: usize = 4 * MAX_ENTRIES;

pub(crate) struct Store {
    dir: Option<PathBuf>,
    socket: Option<PathBuf>,
}

struct CacheLock(File);
impl Drop for CacheLock {
    fn drop(&mut self) {
        // A concurrent fork may inherit this open file description. Explicitly
        // unlock before close so the child cannot retain our completed lock.
        // SAFETY: this is the still-open descriptor on which we took the lock.
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

/// Temporary locations a cache must never use, lexically and canonically.
fn forbidden(dir: &Path, tmpdir: Option<&OsString>) -> bool {
    let mut roots: Vec<PathBuf> = ["/tmp", "/private/tmp", "/var/tmp", "/private/var/tmp"]
        .map(PathBuf::from)
        .to_vec();
    if let Some(tmp) = tmpdir.filter(|t| !t.is_empty()) {
        let tmp = PathBuf::from(tmp);
        if let Ok(real) = tmp.canonicalize() {
            roots.push(real);
        }
        roots.push(tmp);
    }
    // The nearest existing ancestor, resolved, catches a symlinked parent.
    let resolved = dir
        .ancestors()
        .find_map(|a| a.canonicalize().ok().map(|real| (a, real)))
        .and_then(|(a, real)| dir.strip_prefix(a).ok().map(|rest| real.join(rest)));
    !dir.is_absolute()
        || roots.iter().any(|root| {
            dir.starts_with(root) || resolved.as_ref().is_some_and(|r| r.starts_with(root))
        })
}

impl Store {
    /// User data location, independent of ARC_DB_PATH and TMPDIR.
    pub(crate) fn open() -> Option<Self> {
        let home = std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from);
        #[cfg(target_os = "macos")]
        let base = home.map(|p| p.join("Library/Application Support"));
        #[cfg(not(target_os = "macos"))]
        let base = std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty() && Path::new(v).is_absolute())
            .map(PathBuf::from)
            .or_else(|| home.map(|p| p.join(".local/share")));
        if let Some(store) = base.and_then(|base| Self::at(base.join("arc").join(DIRECTORY))) {
            return Some(store);
        }
        let socket = arc_core::socket_path().ok()?;
        let reply = Self::call(&socket, arc_ipc::workspace_cache::CacheRequest::Probe)?;
        (reply.get("capability")?.as_str()? == arc_ipc::workspace_cache::OPERATION).then_some(
            Self {
                dir: None,
                socket: Some(socket),
            },
        )
    }
    pub(crate) fn at(dir: PathBuf) -> Option<Self> {
        if forbidden(&dir, std::env::var_os("TMPDIR").as_ref()) {
            return None;
        }
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .ok()?;
        let meta = fs::symlink_metadata(&dir).ok()?;
        // SAFETY: geteuid has no preconditions or process-wide side effects.
        let uid = unsafe { libc::geteuid() };
        (meta.is_dir() && meta.uid() == uid && meta.permissions().mode() & 0o077 == 0).then_some(
            Self {
                dir: Some(dir),
                socket: None,
            },
        )
    }
    fn call(
        socket: &Path,
        request: arc_ipc::workspace_cache::CacheRequest,
    ) -> Option<serde_json::Value> {
        crate::exec::transport::call(
            socket,
            arc_ipc::workspace_cache::OPERATION,
            serde_json::to_value(request).ok()?,
        )
        .ok()
    }
    fn path(&self, key: &str) -> PathBuf {
        self.dir
            .as_ref()
            .expect("disk cache")
            .join(format!("{key}.json"))
    }
    /// A well-formed entry of this version, or `None`.
    pub(crate) fn load(&self, key: &str) -> Option<Entry> {
        if let Some(socket) = &self.socket {
            let reply = Self::call(
                socket,
                arc_ipc::workspace_cache::CacheRequest::Read {
                    key: key.to_owned(),
                },
            )?;
            let entry: Entry = serde_json::from_str(reply.get("entry")?.as_str()?).ok()?;
            return entry.is_well_formed().then_some(entry);
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
            .open(self.path(key))
            .ok()?;
        let meta = file.metadata().ok()?;
        // SAFETY: as above.
        let uid = unsafe { libc::geteuid() };
        if !meta.is_file() || meta.uid() != uid || meta.size() > MAX_ENTRY_BYTES as u64 {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(MAX_ENTRY_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        let entry: Entry = serde_json::from_slice(&bytes).ok()?;
        entry.is_well_formed().then_some(entry)
    }
    /// Replace the entry atomically; a failure leaves the cache as it was.
    pub(crate) fn save(&self, key: &str, entry: &Entry) {
        let Ok(bytes) = serde_json::to_vec(entry) else {
            return;
        };
        if bytes.len() > MAX_ENTRY_BYTES || !entry.is_well_formed() {
            return;
        }
        if let Some(socket) = &self.socket {
            let _ = Self::call(
                socket,
                arc_ipc::workspace_cache::CacheRequest::Write {
                    key: key.to_owned(),
                    entry: String::from_utf8(bytes).expect("serialized JSON is UTF-8"),
                },
            );
            return;
        }
        let Some(dir) = &self.dir else { return };
        // Lock the directory itself: the kernel releases this lock even when
        // a writer dies, without leaving a lockfile or depending on PID reuse.
        let Ok(lock) = File::open(dir) else {
            return;
        };
        // SAFETY: the descriptor is open and remains owned until this returns.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return;
        }
        let _lock = CacheLock(lock);
        if !self.prune() {
            return;
        }
        let temporary = dir.join(format!(".{key}.{}.tmp", std::process::id()));
        let written = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temporary)
            .and_then(|mut file| file.write_all(&bytes))
            .and_then(|()| fs::rename(&temporary, self.path(key)));
        if written.is_err() {
            let _ = fs::remove_file(&temporary);
        }
    }
    /// Remove interrupted writes and reserve a slot before publishing. When
    /// the directory is larger than the scan budget, clean this batch and
    /// decline the write; subsequent saves continue recovery without growth.
    fn prune(&self) -> bool {
        let Some(dir) = &self.dir else { return false };
        let Ok(listing) = fs::read_dir(dir) else {
            return false;
        };
        let mut entries = Vec::new();
        let mut overflow = false;
        for (index, item) in listing.enumerate() {
            if index == MAX_SCAN {
                overflow = true;
                break;
            }
            let Ok(item) = item else { return false };
            let name = item.file_name();
            let name = name.as_bytes();
            if name.starts_with(b".") && name.ends_with(b".tmp") {
                if fs::remove_file(item.path()).is_err() {
                    return false;
                }
            } else if !name.starts_with(b".") && name.ends_with(b".json") {
                let Ok(meta) = fs::symlink_metadata(item.path()) else {
                    return false;
                };
                if meta.is_file() {
                    entries.push(((meta.mtime(), meta.mtime_nsec()), item.path()));
                }
            }
        }
        if overflow {
            return false;
        }
        entries.sort();
        for (_, path) in &entries[..entries.len().saturating_sub(MAX_ENTRIES - 1)] {
            if fs::remove_file(path).is_err() {
                return false;
            }
        }
        true
    }
}

/// The entry name of one project: its canonical root.
pub(crate) fn key(root: &Path) -> String {
    let mut hash = Sha256::new();
    let mut frame = |bytes: &[u8]| {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    };
    frame(b"arc.analysis-cache.v3");
    frame(root.as_os_str().as_bytes());
    hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary_locations_and_relative_paths_are_refused() {
        for dir in [
            "/tmp/arc",
            "/private/tmp/x/y",
            "/var/tmp/arc",
            "relative/arc",
        ] {
            assert!(forbidden(Path::new(dir), None), "{dir}");
        }
        let tmp = OsString::from("/Users/someone/scratch");
        assert!(forbidden(
            Path::new("/Users/someone/scratch/arc"),
            Some(&tmp)
        ));
        assert!(!forbidden(
            Path::new("/Users/someone/Library/arc"),
            Some(&tmp)
        ));
        assert!(!forbidden(Path::new("/Users/someone/tmpdir"), None));
    }

    #[test]
    fn keys_separate_project_roots() {
        let base = key(Path::new("/w"));
        assert_eq!(base.len(), 64);
        assert_eq!(base, key(Path::new("/w")));
        assert_ne!(base, key(Path::new("/v")));
        assert_ne!(base, key(Path::new("/w/packages/a")));
    }

    #[test]
    fn entries_round_trip_and_eviction_keeps_the_bound() {
        use arc_detector::{
            scripts::cache::{CachedFile, FileIdentity, ENTRY_VERSION},
            Classification,
        };
        // Beside the test binary: neither /tmp nor TMPDIR.
        let dir = std::env::current_exe().unwrap().with_file_name(format!(
            "arc-analysis-store-{}-{}",
            std::process::id(),
            module_path!().replace("::", "-")
        ));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::at(dir.clone()).expect("owned store");
        let held = File::open(&dir).unwrap();
        let inherited = held.try_clone().unwrap();
        // A duplicated descriptor models the open description inherited by a
        // fork: dropping our guard must unlock even while that duplicate lives.
        assert_eq!(
            unsafe { libc::flock(held.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        let guard = CacheLock(held);
        let observer = File::open(&dir).unwrap();
        assert_ne!(
            unsafe { libc::flock(observer.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        drop(guard);
        assert_eq!(
            unsafe { libc::flock(observer.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        unsafe { libc::flock(observer.as_raw_fd(), libc::LOCK_UN) };
        drop(inherited);
        let classification: Classification =
            serde_json::from_str(r#"{"classification":"TEST","locality":"HOST_LOCAL","reason":"BUN_SCRIPT_TEST","uncertainty":"NONE"}"#).unwrap();
        let entry = Entry {
            version: ENTRY_VERSION,
            files: vec![CachedFile {
                path: "package.json".into(),
                identity: FileIdentity {
                    dev: 1,
                    ino: 2,
                    size: 3,
                    mtime_ns: 4,
                    ctime_ns: 5,
                },
                sha256: "a".repeat(64),
            }],
            memberships: vec![],
            absent: vec![],
            results: vec![arc_detector::scripts::cache::CachedResult {
                key: "b".repeat(64),
                classification,
                members: vec![],
            }],
        };
        assert_eq!(store.load("missing"), None);
        store.save("k0", &entry);
        assert_eq!(store.load("k0"), Some(entry.clone()));
        fs::write(store.path("bad"), b"{").unwrap();
        assert_eq!(store.load("bad"), None);
        fs::remove_file(store.path("bad")).unwrap();
        for at in 1..=MAX_ENTRIES + 2 {
            store.save(&format!("k{at}"), &entry);
        }
        let count = fs::read_dir(&dir).unwrap().count();
        assert_eq!(count, MAX_ENTRIES);
        assert_eq!(store.load("k0"), None, "the oldest entry was evicted");
        assert_eq!(
            fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        // Simulate writers killed after creating temporary files. A bounded
        // cleanup may need several saves, but cannot add entries while a scan
        // is incomplete or let temporary files hide entries from eviction.
        for at in 0..MAX_SCAN + 20 {
            fs::write(dir.join(format!(".orphan-{at}.tmp")), b"partial").unwrap();
        }
        for _ in 0..4 {
            store.save("recovered", &entry);
        }
        assert_eq!(store.load("recovered"), Some(entry.clone()));
        assert!(fs::read_dir(&dir).unwrap().count() <= MAX_ENTRIES);
        assert!(fs::read_dir(&dir).unwrap().all(|item| !item
            .unwrap()
            .file_name()
            .as_bytes()
            .ends_with(b".tmp")));
        std::thread::scope(|scope| {
            for at in 0..32 {
                let dir = dir.clone();
                let entry = &entry;
                scope.spawn(move || {
                    let concurrent = Store::at(dir).unwrap();
                    concurrent.save(&format!("parallel-{at}"), entry);
                });
            }
        });
        assert!(fs::read_dir(&dir).unwrap().count() <= MAX_ENTRIES);
        fs::remove_dir_all(&dir).unwrap();
    }
}
