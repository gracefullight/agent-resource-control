//! Executable-local native symbol interposition tests the actual arc-core probe.
#![cfg(all(test, target_os = "macos"))]
use arc_core::process_identity::{boot_id, probe_live, IdentityStatus, ProcessIdentity};
use std::sync::atomic::{AtomicI32, Ordering};
static MODE: AtomicI32 = AtomicI32::new(0);
static CALLS: AtomicI32 = AtomicI32::new(0);
#[unsafe(no_mangle)]
unsafe extern "C" fn proc_pidinfo(
    _pid: libc::c_int,
    _flavor: libc::c_int,
    _arg: u64,
    buffer: *mut libc::c_void,
    size: libc::c_int,
) -> libc::c_int {
    CALLS.fetch_add(1, Ordering::SeqCst);
    match MODE.load(Ordering::SeqCst) {
        1 => { unsafe { *libc::__error() = libc::EPERM }; 0 }
        2 => { unsafe { *libc::__error() = 0 }; size - 1 }
        3 => { unsafe { *libc::__error() = libc::ESRCH }; 0 }
        _ => {
            let info = unsafe { &mut *buffer.cast::<libc::proc_bsdinfo>() };
            info.pbi_start_tvsec = 12;
            info.pbi_start_tvusec = 34;
            size
        }
    }
}
fn identity() -> ProcessIdentity {
    ProcessIdentity { pid: std::process::id(), start_token: "12000034".into(), boot_id: boot_id().unwrap() }
}
#[test]
fn public_native_probe_keeps_permission_denial_and_short_reads_indeterminate() {
    let identity = identity();
    for mode in [1, 2] {
        MODE.store(mode, Ordering::SeqCst);
        CALLS.store(0, Ordering::SeqCst);
        assert_eq!(probe_live(&identity), IdentityStatus::Indeterminate);
        assert_eq!(CALLS.load(Ordering::SeqCst), 1, "must exercise the injected native call");
    }
}
#[test]
fn public_native_probe_distinguishes_match_reuse_boot_change_and_missing() {
    let mut identity = identity();
    MODE.store(0, Ordering::SeqCst);
    assert_eq!(probe_live(&identity), IdentityStatus::VerifiedMatch);
    identity.start_token = "12000035".into();
    assert_eq!(probe_live(&identity), IdentityStatus::Mismatch);
    identity.boot_id = "00000000-0000-0000-0000-000000000000".into();
    CALLS.store(0, Ordering::SeqCst);
    assert_eq!(probe_live(&identity), IdentityStatus::Mismatch);
    assert_eq!(CALLS.load(Ordering::SeqCst), 0);
    identity = self::identity();
    MODE.store(3, Ordering::SeqCst);
    assert_eq!(probe_live(&identity), IdentityStatus::Missing);
}
