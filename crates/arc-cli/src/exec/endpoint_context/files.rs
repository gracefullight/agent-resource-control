//! Bounded, nonblocking reads of regular configuration files.
use std::{
    fs::OpenOptions,
    io::{self, Read},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

pub(super) struct Reads {
    bytes: usize,
    files: usize,
}
impl Reads {
    pub(super) fn new() -> Self {
        Self { bytes: 0, files: 0 }
    }
    pub(super) fn read(&mut self, path: &Path) -> Option<Option<Vec<u8>>> {
        self.files = self.files.checked_add(1)?;
        if self.files > 16 {
            return None;
        }
        let file = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
            .open(path)
        {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Some(None),
            Err(_) => return None,
        };
        if !file.metadata().ok()?.is_file() {
            return None;
        }
        let limit = arc_detector::MAX_CONTEXT_BYTES.checked_sub(self.bytes)?;
        let mut data = Vec::new();
        file.take((limit + 1) as u64).read_to_end(&mut data).ok()?;
        self.bytes = self.bytes.checked_add(data.len())?;
        (self.bytes <= arc_detector::MAX_CONTEXT_BYTES).then_some(Some(data))
    }
}

pub(super) fn path_env(key: &str) -> Option<Option<std::path::PathBuf>> {
    match std::env::var_os(key) {
        None => Some(None),
        Some(value)
            if value.as_encoded_bytes().len() <= arc_detector::MAX_CONTEXT_VALUE_BYTES
                && !value.is_empty() =>
        {
            Some(Some(value.into()))
        }
        Some(value) if value.is_empty() => Some(None),
        Some(_) => None,
    }
}

pub(super) fn fact(
    context: &mut arc_detector::EndpointContext,
    kind: arc_detector::EndpointKind,
    selector: &str,
    endpoint: String,
) -> Option<()> {
    context.facts.push(arc_detector::EndpointFact {
        kind,
        selector: selector.to_owned(),
        endpoints: vec![endpoint],
    });
    context.is_valid_at(context.captured_at_ms).then_some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::CString,
        fs,
        os::unix::{ffi::OsStrExt, fs::symlink},
    };

    #[test]
    fn rejects_oversize_symlink_and_fifo_without_blocking() {
        let root = std::env::temp_dir().join(format!("arc-read-bounds-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let file = root.join("config");
        fs::write(&file, vec![b'x'; arc_detector::MAX_CONTEXT_BYTES + 1]).unwrap();
        assert!(Reads::new().read(&file).is_none());
        let link = root.join("link");
        symlink(&file, &link).unwrap();
        assert!(Reads::new().read(&link).is_none());
        let fifo = root.join("fifo");
        let fifo_name = CString::new(fifo.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo_name.as_ptr(), 0o600) }, 0);
        assert!(Reads::new().read(&fifo).is_none());
    }
}
