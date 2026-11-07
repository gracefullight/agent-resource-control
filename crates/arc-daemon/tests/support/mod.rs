use std::{
    fs::{File, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    sync::{Mutex, MutexGuard},
    thread,
    time::{Duration, Instant},
};

/// Separate from production admission; shared by threads and integration binaries.
/// The caller holds this until all its owned daemon children have been reaped.
pub struct Serial {
    _file: File,
    _thread: MutexGuard<'static, ()>,
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
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        match file.try_lock() {
            Ok(()) => {
                return Serial {
                    _file: file,
                    _thread: thread,
                }
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                assert!(
                    Instant::now() < deadline,
                    "daemon fixture coordination timed out: {}",
                    path.display()
                );
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => panic!("daemon fixture coordination failed: {error}"),
        }
    }
}
