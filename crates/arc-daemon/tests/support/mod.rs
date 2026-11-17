use std::{
    fs::{File, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    sync::{Mutex, MutexGuard},
};

/// Separate from production admission; shared by threads and integration binaries.
/// The caller holds this until all its owned daemon children have been reaped.
pub struct Serial {
    _file: Option<File>,
    _thread: MutexGuard<'static, ()>,
}

impl Serial {
    /// Let a child acquire the process lock while excluding sibling test threads.
    /// Keep this guard alive until that child exits, so a local fixture cannot
    /// take the file lock ahead of the child during the coordination probe.
    #[allow(dead_code)] // Only the resident coordination probe hands off this lock.
    pub fn release_process_lock(&mut self) {
        self._file.take();
    }
}

pub fn serial() -> Serial {
    static THREAD: Mutex<()> = Mutex::new(());
    let thread = THREAD.lock().unwrap_or_else(|error| error.into_inner());
    let uid = unsafe { libc::geteuid() };
    let path = arc_core::runtime_root()
        .unwrap()
        .join(format!("arc-fixtures-{uid}.lock"));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
        .unwrap();
    let metadata = file.metadata().unwrap();
    assert!(metadata.is_file());
    assert_eq!(metadata.uid(), uid);
    assert_eq!(metadata.mode() & 0o7777, 0o600);
    // The production daemon owns one per-user instance anchor. Wait directly
    // on that shared fixture lock rather than polling or imposing a deadline.
    if let Err(error) = file.lock() {
        panic!("daemon fixture coordination failed: {error}");
    }
    Serial {
        _file: Some(file),
        _thread: thread,
    }
}
