//! Lifetime of the pre-Meter compatibility peer shared by the serialized case.
use std::{
    fs::{self, File, OpenOptions},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{MetadataExt, OpenOptionsExt},
            process::CommandExt,
        },
    },
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

#[path = "legacy_preparation_receipt.rs"]
mod receipt;

// A stable, per-worktree inode prevents acquisition from racing retirement.
// It lives outside target so the last successful fixture leaves no build tree.
fn coordination_path(root: &Path) -> PathBuf {
    let metadata = fs::metadata(root).expect("legacy workspace metadata");
    let uid = unsafe { libc::geteuid() };
    PathBuf::from("/tmp").join(format!(
        "arc-legacy-{uid}-{}-{}.lock",
        metadata.dev(),
        metadata.ino()
    ))
}

fn coordination_lock(root: &Path) -> File {
    let uid = unsafe { libc::geteuid() };
    let path = coordination_path(root);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .expect("open legacy coordination lock");
    let metadata = file.metadata().expect("legacy coordination lock metadata");
    assert!(metadata.is_file() && metadata.uid() == uid && metadata.mode() & 0o7777 == 0o600);
    file.lock().expect("lock legacy preparation coordination");
    file
}

pub struct LegacyPreparation {
    pub receipt: serde_json::Value,
    directory: PathBuf,
    latest: PathBuf,
    latest_bytes: Vec<u8>,
    lease: Option<File>,
    root: PathBuf,
}

impl LegacyPreparation {
    pub fn acquire_prepared(root: &Path) -> Self {
        Self::ensure_with(root, |coordination| {
            let fd = coordination.as_raw_fd();
            let mut command = Command::new("bun");
            command
                .arg("scripts/host-meter/prepare-legacy.ts")
                .current_dir(root)
                .env("ARC_LEGACY_COORDINATION_FD", fd.to_string())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .process_group(0);
            unsafe {
                command.pre_exec(move || {
                    if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            let mut child = crate::native::OwnedChild::new(
                command.spawn().expect("start legacy peer preparation"),
            );
            // The preparation's own build guard is two hours. Allow that guard
            // to report first, but never wait indefinitely for this child.
            let output = child
                .wait_with_output_for(Duration::from_secs(7_260), true)
                .expect("legacy peer preparation exit/pipe watchdog");
            assert!(
                output.status.success(),
                "legacy peer preparation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        })
    }

    fn ensure_with(root: &Path, prepare: impl FnOnce(&File)) -> Self {
        let coordination = coordination_lock(root);
        if !root.join("target/host-meter/legacy/latest.json").exists() {
            prepare(&coordination);
        }
        Self::acquire_locked(root)
    }

    pub fn acquire(root: &Path) -> Self {
        let _coordination = coordination_lock(root);
        Self::acquire_locked(root)
    }

    fn acquire_locked(root: &Path) -> Self {
        let latest = root.join("target/host-meter/legacy/latest.json");
        let latest_bytes = fs::read(&latest)
            .expect("prepare accepted legacy peer with bun scripts/host-meter/prepare-legacy.ts");
        let receipt: serde_json::Value = serde_json::from_slice(&latest_bytes).unwrap();
        assert_eq!(
            receipt["acceptedHead"],
            "efedf0f4eb9b97eed641c82f3c4766bb16f2e3de"
        );
        assert_eq!(receipt["exitCode"], 0);
        let path = Path::new(
            receipt["binary"]["path"]
                .as_str()
                .expect("legacy binary path"),
        );
        let parts: Vec<_> = path.components().collect();
        assert_eq!(parts.len(), 7, "unexpected legacy binary location");
        for (index, expected) in [
            (0, "target"),
            (1, "host-meter"),
            (2, "legacy"),
            (4, "target"),
            (5, "debug"),
            (6, "arc-daemon"),
        ] {
            assert_eq!(
                parts[index],
                Component::Normal(expected.as_ref()),
                "unexpected legacy binary location"
            );
        }
        let Component::Normal(id) = parts[3] else {
            panic!("invalid legacy preparation identity")
        };
        let id = id
            .to_str()
            .expect("legacy preparation identity must be UTF-8");
        assert!(
            id.len() == 36 && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-'),
            "invalid legacy preparation identity"
        );
        let directory = root.join("target/host-meter/legacy").join(id);
        let metadata = fs::symlink_metadata(&directory).expect("legacy preparation directory");
        assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(directory.join("lease.lock"))
            .expect("open legacy preparation lease");
        lease
            .lock_shared()
            .expect("acquire legacy preparation lease");
        assert!(
            directory.is_dir(),
            "legacy preparation disappeared during acquisition"
        );
        receipt::validate(root, &directory, id, &latest_bytes, &receipt);
        Self {
            receipt,
            directory,
            latest,
            latest_bytes,
            lease: Some(lease),
            root: root.to_owned(),
        }
    }
}

impl LegacyPreparation {
    fn retire_with(&mut self, after_lock: impl FnOnce()) {
        if self.lease.is_none() {
            return;
        }
        let _coordination = coordination_lock(&self.root);
        after_lock();
        // Failure is durable even if another consumer later succeeds.
        if std::thread::panicking() {
            if let Err(error) = fs::write(self.directory.join("failed-case"), b"retained\n") {
                eprintln!("cannot mark failed legacy preparation: {error}");
            }
            if fs::read(&self.latest).ok().as_deref() == Some(&self.latest_bytes) {
                let _ = fs::remove_file(&self.latest);
            }
            return;
        }
        // Explicitly unlock before closing: another test may be between fork
        // and exec and briefly hold an inherited duplicate of this descriptor.
        // Closing only our copy would leave that duplicate's shared lock live.
        if let Some(lease) = self.lease.take() {
            lease.unlock().expect("release legacy preparation lease");
        }
        let lease = match OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.directory.join("lease.lock"))
        {
            Ok(lease) => lease,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => panic!("open legacy cleanup lease: {error}"),
        };
        match lease.try_lock() {
            Ok(()) => (),
            Err(fs::TryLockError::WouldBlock) => return,
            Err(error) => panic!("lock legacy cleanup lease: {error}"),
        }
        if self.directory.join("failed-case").exists() {
            return;
        }
        fs::remove_dir_all(&self.directory).expect("remove consumed legacy preparation");
        if fs::read(&self.latest).ok().as_deref() == Some(&self.latest_bytes) {
            fs::remove_file(&self.latest).expect("remove consumed legacy pointer");
        }
    }
}

impl Drop for LegacyPreparation {
    fn drop(&mut self) {
        self.retire_with(|| {});
    }
}

#[cfg(test)]
#[path = "legacy_preparation_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "legacy_preparation_race_tests.rs"]
mod race_tests;
