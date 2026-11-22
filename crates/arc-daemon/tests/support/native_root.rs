//! Owned fixture directory names. A failed run keeps its directory for
//! diagnosis, so a later process that reuses the pid can meet a leftover name.
use std::{
    fs, io,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// Create `b<pid>-<n>` under `parent`, taking the next counter value while the
/// name belongs to a leftover directory.
pub fn fresh(parent: &Path, next: &AtomicU64) -> PathBuf {
    loop {
        let root = parent.join(format!(
            "b{:x}-{:x}",
            std::process::id(),
            next.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::DirBuilder::new().mode(0o700).create(&root) {
            Ok(()) => return root,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("cannot create owned fixture {}: {error}", root.display()),
        }
    }
}
