use std::{
    fs::{Metadata, OpenOptions},
    io::{self, Read},
    os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
    path::Path,
};

const MAX_BYTES: usize = 64 * 1024;
const MAX_FILES: usize = 12;

pub(in crate::exec::ci_context) struct Reader {
    bytes: usize,
    files: usize,
}

impl Reader {
    pub(in crate::exec::ci_context) fn new() -> Self {
        Self { bytes: 0, files: 0 }
    }

    pub(super) fn system_config(&mut self, config: &mut super::Config, skip: bool) -> Option<()> {
        if skip {
            return Some(());
        }
        // This override replaces Git's build-time system path. Without it,
        // the selected installation's compiled path cannot be established
        // from the program's location or from PATH.
        let path = super::env_path("GIT_CONFIG_SYSTEM")??;
        if !path.is_absolute() {
            return None;
        }
        super::read_config(self, config, &path)
    }

    pub(in crate::exec::ci_context) fn read(&mut self, path: &Path) -> Option<Option<Vec<u8>>> {
        self.files += 1;
        if self.files > MAX_FILES {
            return None;
        }
        let mut file = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
            .open(path)
        {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Some(None),
            Err(_) => return None,
        };
        let before = file.metadata().ok()?;
        if !before.is_file() && !is_null_device(path, &before) {
            return None;
        }
        let remaining = MAX_BYTES.checked_sub(self.bytes)?;
        let mut bytes = Vec::new();
        (&mut file)
            .take((remaining + 1) as u64)
            .read_to_end(&mut bytes)
            .ok()?;
        self.bytes += bytes.len();
        if self.bytes > MAX_BYTES
            || !unchanged(&before, &file.metadata().ok()?, bytes.len())
            || !unchanged(&before, &std::fs::symlink_metadata(path).ok()?, bytes.len())
        {
            return None;
        }
        Some(Some(bytes))
    }
}

fn is_null_device(path: &Path, metadata: &Metadata) -> bool {
    if path != Path::new("/dev/null") || !metadata.file_type().is_char_device() {
        return false;
    }
    let device = metadata.rdev() as libc::dev_t;
    #[cfg(target_os = "macos")]
    return libc::major(device) == 3 && libc::minor(device) == 2;
    #[cfg(target_os = "linux")]
    return libc::major(device) == 1 && libc::minor(device) == 3;
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = device;
        false
    }
}

fn unchanged(before: &Metadata, after: &Metadata, bytes: usize) -> bool {
    before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.len() == after.len()
        && before.len() == bytes as u64
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
        && before.ctime() == after.ctime()
        && before.ctime_nsec() == after.ctime_nsec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Write, time::SystemTime};

    #[test]
    fn changed_file_metadata_invalidates_a_snapshot() {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("arc-ci-reader-{}-{nonce}", std::process::id()));
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap()
            .write_all(b"first")
            .unwrap();
        let before = fs::metadata(&path).unwrap();
        assert!(!is_null_device(Path::new("/dev/null"), &before));
        assert!(unchanged(&before, &fs::metadata(&path).unwrap(), 5));
        assert!(!unchanged(&before, &before, 4));
        fs::write(&path, b"changed content").unwrap();
        let after = fs::metadata(&path).unwrap();
        assert!(!unchanged(&before, &after, 5));
        let replacement = path.with_extension("replacement");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&replacement)
            .unwrap()
            .write_all(b"changed content")
            .unwrap();
        fs::rename(&replacement, &path).unwrap();
        assert!(!unchanged(&after, &fs::metadata(&path).unwrap(), 15));
        fs::remove_file(path).unwrap();
    }
}
