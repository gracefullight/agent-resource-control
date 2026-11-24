//! The test's own filesystem observations and its bounded, owned write workload
//! for MTR-001 item 3. Nothing here reads a daemon or `arc-platform` value.
use crate::runtime::{child_watchdog, instrumentation, Fixture};
use arc_core::meter::DiskPressure;
use std::{
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    mem,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
    process::Command,
    thread,
};

const CHUNK_BYTES: usize = 256 * 1024;
const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// This file is removed even if another fixture deliberately retains its
/// directory after a panic. It is never preallocated or made sparse.
pub struct WriteFixture {
    path: PathBuf,
    file: Option<File>,
    pub written: u64,
    random: u64,
}

impl WriteFixture {
    pub fn new(path: PathBuf) -> Self {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        Self {
            path,
            file: Some(file),
            written: 0,
            random: 0x4d59_5df4_d0f3_3173,
        }
    }

    /// One durable chunk of incompressible bytes, never beyond `MAX_BYTES`.
    pub fn write_chunk(&mut self) {
        assert!(
            self.written + CHUNK_BYTES as u64 <= MAX_BYTES,
            "disk fixture reached its 64 MiB bound"
        );
        let mut data = vec![0u8; CHUNK_BYTES];
        for bytes in data.chunks_exact_mut(8) {
            self.random ^= self.random << 13;
            self.random ^= self.random >> 7;
            self.random ^= self.random << 17;
            bytes.copy_from_slice(&self.random.to_ne_bytes());
        }
        let file = self.file.as_mut().unwrap();
        file.write_all(&data).unwrap();
        file.sync_data().unwrap();
        self.written += CHUNK_BYTES as u64;
        assert_eq!(file.metadata().unwrap().len(), self.written);
    }

    /// Bytes the filesystem has actually allocated to the file.
    pub fn allocated(&self) -> u64 {
        self.file.as_ref().unwrap().metadata().unwrap().blocks() * 512
    }
}

impl Drop for WriteFixture {
    fn drop(&mut self) {
        drop(self.file.take());
        if let Err(error) = fs::remove_file(&self.path) {
            if error.kind() != io::ErrorKind::NotFound {
                if thread::panicking() {
                    eprintln!("disk fixture cleanup failed: {error}");
                } else {
                    panic!("disk fixture cleanup failed: {error}");
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct NativeFs {
    pub mount: String,
    pub kind: String,
    pub volume: String,
    pub local: bool,
    pub capacity: u64,
    pub available: u64,
}

fn native_text(bytes: &[libc::c_char]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .expect("terminated statfs field");
    String::from_utf8(bytes[..end].iter().map(|byte| *byte as u8).collect()).unwrap()
}

/// A fresh statfs in the test process, not a daemon or arc-platform reading.
pub fn statfs(path: &Path) -> NativeFs {
    let path = CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: statfs writes exactly the initialized Darwin SDK output buffer.
    let mut data: libc::statfs = unsafe { mem::zeroed() };
    assert_eq!(
        unsafe { libc::statfs(path.as_ptr(), &mut data) },
        0,
        "statfs: {}",
        io::Error::last_os_error()
    );
    assert!(data.f_bsize > 0 && data.f_bavail <= data.f_bfree && data.f_bfree <= data.f_blocks);
    // fsid_t is opaque in libc; read its two SDK words without accessing fields.
    assert_eq!(mem::size_of_val(&data.f_fsid), 8);
    let bytes =
        unsafe { std::slice::from_raw_parts(std::ptr::addr_of!(data.f_fsid).cast::<u8>(), 8) };
    let word = |part: &[u8]| i32::from_ne_bytes(part.try_into().unwrap());
    NativeFs {
        mount: native_text(&data.f_mntonname),
        kind: native_text(&data.f_fstypename),
        volume: format!("{:08x}:{:08x}", word(&bytes[..4]), word(&bytes[4..])),
        local: data.f_flags & libc::MNT_LOCAL as u32 != 0,
        capacity: data.f_blocks.checked_mul(u64::from(data.f_bsize)).unwrap(),
        available: data.f_bavail.checked_mul(u64::from(data.f_bsize)).unwrap(),
    }
}

/// `diskutil` resolves distinct APFS volume mounts to their actual container.
/// Inspect only that property; no personal configuration or interface is read.
pub fn apfs_container(home: &Path, mount: &str) -> String {
    let mut command = Command::new("/usr/sbin/diskutil");
    command
        .args(["info", "-plist", mount])
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/sbin:/usr/bin:/bin")
        .current_dir(home);
    instrumentation::forward(&mut command);
    let output = child_watchdog::output(&mut command, "APFS container lookup").unwrap();
    assert!(
        output.status.success(),
        "diskutil info failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let plist = std::str::from_utf8(&output.stdout).unwrap();
    let (_, value) = plist
        .split_once("<key>APFSContainerReference</key>")
        .expect("APFS container property");
    let (_, value) = value.split_once("<string>").expect("APFS container value");
    let (value, _) = value
        .split_once("</string>")
        .expect("APFS container terminator");
    assert!(!value.is_empty());
    value.to_owned()
}

/// The documented judgment from available/capacity in integer arithmetic,
/// independently of the production floating-point calculation: Critical at or
/// below 5%, Warning above 5% through 10%, Normal above 10%.
pub fn pressure(available: u64, capacity: u64) -> DiskPressure {
    assert!(capacity > 0 && available <= capacity);
    if u128::from(available) * 20 <= u128::from(capacity) {
        DiskPressure::Critical
    } else if u128::from(available) * 10 <= u128::from(capacity) {
        DiskPressure::Warning
    } else {
        DiskPressure::Normal
    }
}

pub fn rank(value: DiskPressure) -> u8 {
    match value {
        DiskPressure::Critical => 0,
        DiskPressure::Warning => 1,
        DiskPressure::Normal => 2,
    }
}

#[test]
fn disk_pressure_boundaries_are_inclusive() {
    for (available, expected) in [
        (4, DiskPressure::Critical),
        (5, DiskPressure::Critical),
        (6, DiskPressure::Warning),
        (10, DiskPressure::Warning),
        (11, DiskPressure::Normal),
        (100, DiskPressure::Normal),
    ] {
        assert_eq!(pressure(available, 100), expected, "{available}/100");
    }
}

#[test]
fn disk_fixture_is_removed_during_unwind() {
    let f = Fixture::new();
    let path = f.root.join("disk-unwind-check");
    let existed = std::cell::Cell::new(false);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut writer = WriteFixture::new(path.clone());
        writer.write_chunk();
        existed.set(path.exists());
        panic!("exercise disk fixture cleanup");
    }));
    assert!(result.is_err());
    assert!(existed.get(), "the workload file was never written");
    assert!(!path.exists());
}
