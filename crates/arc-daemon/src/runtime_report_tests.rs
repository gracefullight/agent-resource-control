use super::*;
use std::{
    fs,
    os::unix::fs::{symlink, DirBuilderExt},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = arc_core::runtime_root().unwrap().join(format!(
            "rr-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(1)
}

#[test]
fn report_open_is_private_create_new_and_refuses_links_and_unsafe_parents() {
    let directory = Directory::new();
    let path = directory.0.join("report");
    let report = RuntimeReport::open(Some(&path)).unwrap();
    let metadata = fs::symlink_metadata(&path).unwrap();
    assert!(metadata.is_file());
    assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    assert_eq!(metadata.mode() & 0o7777, 0o600);
    report
        .finish(&serde_json::json!({"complete":true}), deadline())
        .unwrap();
    let content = fs::read(&path).unwrap();
    assert!(RuntimeReport::open(Some(&path)).is_err());
    assert_eq!(fs::read(&path).unwrap(), content);
    let link = directory.0.join("link");
    symlink(&path, &link).unwrap();
    assert!(RuntimeReport::open(Some(&link)).is_err());
    assert_eq!(fs::read(&path).unwrap(), content);
    let alias = directory.0.join("alias");
    symlink(&directory.0, &alias).unwrap();
    assert!(RuntimeReport::open(Some(&alias.join("new"))).is_err());
    assert!(!directory.0.join("new").exists());
    fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(RuntimeReport::open(Some(&directory.0.join("unsafe"))).is_err());
    assert!(!directory.0.join("unsafe").exists());
}

#[test]
fn retained_descriptor_is_not_redirected_by_path_replacement() {
    let directory = Directory::new();
    let path = directory.0.join("report");
    let retained = directory.0.join("retained");
    let target = directory.0.join("target");
    fs::write(&target, b"unchanged").unwrap();
    let report = RuntimeReport::open(Some(&path)).unwrap();
    fs::rename(&path, &retained).unwrap();
    symlink(&target, &path).unwrap();
    report
        .finish(&serde_json::json!({"complete":true}), deadline())
        .unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"unchanged");
    assert_eq!(fs::read(&retained).unwrap(), b"{\"complete\":true}");
}

#[test]
fn complete_serialization_is_capped_before_any_file_write() {
    let directory = Directory::new();
    let path = directory.0.join("report");
    let oversized = serde_json::Value::String("x".repeat(MAX_REPORT_BYTES - 1));
    assert!(RuntimeReport::open(Some(&path))
        .unwrap()
        .finish(&oversized, deadline())
        .is_err());
    assert!(fs::read(&path).unwrap().is_empty());
    let exact = serde_json::Value::String("x".repeat(MAX_REPORT_BYTES - 2));
    assert_eq!(encode(&exact).unwrap().len, MAX_REPORT_BYTES);
    let accepted = directory.0.join("exact");
    RuntimeReport::open(Some(&accepted))
        .unwrap()
        .finish(&exact, deadline())
        .unwrap();
    assert_eq!(
        fs::metadata(&accepted).unwrap().len(),
        MAX_REPORT_BYTES as u64
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(accepted).unwrap()).unwrap(),
        exact
    );
}

#[test]
fn expired_deadline_and_real_os_write_errors_are_failures() {
    let directory = Directory::new();
    let path = directory.0.join("report");
    let report = RuntimeReport::open(Some(&path)).unwrap();
    assert_eq!(
        report
            .finish(&serde_json::json!({}), Instant::now())
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
    );
    assert!(fs::read(&path).unwrap().is_empty());
    let report = RuntimeReport(Some(File::open(path).unwrap()));
    let error = report
        .finish(&serde_json::json!({}), deadline())
        .unwrap_err();
    assert_eq!(error.raw_os_error(), Some(libc::EBADF));
}

#[test]
fn invalid_report_names_fail_before_creating_a_file() {
    let directory = Directory::new();
    for path in [
        std::path::PathBuf::from("no-parent"),
        directory.0.join(".."),
        directory.0.join("nul\0name"),
    ] {
        let error = RuntimeReport::open(Some(&path)).err().unwrap();
        assert_eq!(
            error.kind(),
            io::ErrorKind::InvalidInput,
            "{path:?}: {error}"
        );
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    }
}
