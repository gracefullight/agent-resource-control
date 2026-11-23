//! Hold the shared per-user fixture lock for this test binary's lifetime.
//! Individual tests remain serialized and clean up their own daemons before
//! releasing the in-process guard. Keeping the same file lock avoids letting
//! another binary insert an unrelated daemon lifetime between these tests.
use std::{
    fs::{File, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    sync::{Mutex, MutexGuard, OnceLock},
};

pub fn acquire() -> MutexGuard<'static, ()> {
    static THREAD: Mutex<()> = Mutex::new(());
    static PROCESS: OnceLock<File> = OnceLock::new();
    let thread = THREAD.lock().unwrap_or_else(|error| error.into_inner());
    PROCESS.get_or_init(|| {
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
            .open(path)
            .unwrap();
        let metadata = file.metadata().unwrap();
        assert!(metadata.is_file());
        assert_eq!(metadata.uid(), uid);
        assert_eq!(metadata.mode() & 0o7777, 0o600);
        file.lock().expect("daemon fixture coordination failed");
        file
    });
    thread
}
