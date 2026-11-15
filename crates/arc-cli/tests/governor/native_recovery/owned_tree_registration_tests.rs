//! A fixture's waiting process is identified before its request is published.
use super::{owned_tree, *};
use std::{io::Write, path::PathBuf};

// Invoked as a subprocess of a Go, shell or Perl fixture. The helper captures
// its actual parent while that parent waits, then publishes a complete identity
// with an atomic rename. An exited parent's stale request cannot identify a
// later process that happens to reuse its PID.
#[test]
fn owned_tree_registration_helper() {
    if std::env::var_os("ARC_TEST_OWNER_HELPER").as_deref() != Some(std::ffi::OsStr::new("1")) {
        return;
    }
    let directory = PathBuf::from(std::env::var_os(owned_tree::REGISTRY_ENV).unwrap());
    let parent = unsafe { libc::getppid() } as u32;
    let identity = ProcessIdentity::capture(parent).expect("capture requesting parent");
    assert_eq!(unsafe { libc::getppid() } as u32, parent);
    assert_eq!(
        arc_core::process_identity::probe(&identity),
        IdentityStatus::VerifiedMatch
    );
    let temporary = directory.join(format!(
        "{parent}.{}.{}.pending",
        identity.start_token,
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let request = directory.join(format!("{parent}.{}.request", identity.start_token));
    let ack = directory.join(format!("{parent}.{}.ack", identity.start_token));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .unwrap();
    file.write_all(&serde_json::to_vec(&identity).unwrap())
        .unwrap();
    drop(file);
    fs::rename(&temporary, &request).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !ack.exists() {
        assert!(
            Instant::now() < deadline,
            "ownership registration timed out"
        );
        assert_eq!(unsafe { libc::getppid() } as u32, parent, "parent exited");
        thread::sleep(Duration::from_millis(2));
    }
}
