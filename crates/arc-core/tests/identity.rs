use arc_core::process_identity::{
    boot_id, probe, probe_live, process_is_alive, process_start_time, random_job_id,
    IdentityStatus, ProcessIdentity,
};
use std::process::Command;

#[test]
fn native_identity_matches_current_process_and_rejects_reuse_and_other_boot() {
    let current = ProcessIdentity::capture(std::process::id()).unwrap();
    assert!(current.is_valid());
    assert_eq!(current.boot_id, boot_id().unwrap());
    assert_eq!(probe(&current), IdentityStatus::VerifiedMatch);
    assert_eq!(probe_live(&current), IdentityStatus::VerifiedMatch);
    assert!(process_is_alive(current.pid));
    assert_eq!(
        process_start_time(current.pid).unwrap().to_string(),
        current.start_token
    );
    let mut reused = current.clone();
    reused.start_token = (reused.start_token.parse::<u64>().unwrap() + 1).to_string();
    assert_eq!(probe(&reused), IdentityStatus::Mismatch);
    reused.boot_id = "00000000-0000-0000-0000-000000000000".into();
    assert_eq!(probe(&reused), IdentityStatus::Mismatch);
    for token in ["0", "01", "-1", "18446744073709551616", "secret"] {
        reused.start_token = token.into();
        assert!(!reused.is_valid());
        assert_eq!(probe(&reused), IdentityStatus::Indeterminate);
    }
    assert!(ProcessIdentity::capture(0).is_err());
    assert!(process_start_time(u32::MAX).is_err());
}
#[test]
fn random_keys_are_native_and_not_reused() {
    let a = random_job_id().unwrap();
    let b = random_job_id().unwrap();
    assert_ne!(a, b);
    assert_eq!(a.len(), 32);
    assert!(a
        .bytes()
        .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v)));
}
#[test]
fn immediate_exit_identity_is_captured_after_exit_before_reap() {
    let mut child = Command::new("/usr/bin/true").spawn().unwrap();
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    // SAFETY: this is our unreaped child; WNOWAIT observes but does not reap it.
    assert_eq!(
        unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                &mut info,
                libc::WEXITED | libc::WNOWAIT,
            )
        },
        0
    );
    let identity = ProcessIdentity::capture(child.id()).unwrap();
    assert_eq!(probe(&identity), IdentityStatus::VerifiedMatch);
    assert_eq!(probe_live(&identity), IdentityStatus::Missing);
    assert!(child.wait().unwrap().success());
    assert_eq!(probe(&identity), IdentityStatus::Missing);
    assert!(!process_is_alive(child.id()));
}

#[cfg(target_os = "macos")]
#[test]
fn live_and_zombie_native_tokens_use_the_same_namespace() {
    let mut child = Command::new("/bin/cat")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let live = ProcessIdentity::capture(child.id()).unwrap();
    drop(child.stdin.take());
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                &mut info,
                libc::WEXITED | libc::WNOWAIT,
            )
        },
        0
    );
    let zombie = ProcessIdentity::capture(child.id()).unwrap();
    let waited = child.wait().unwrap();
    assert!(waited.success());
    assert_eq!(live, zombie);
}
