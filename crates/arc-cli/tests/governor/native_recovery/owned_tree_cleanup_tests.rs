//! Deterministic PID-reuse regressions and a real kernel generation check.
use super::super::owned_tree_process::{
    self, NativeOps, ObservedProcess, OwnedProcess, ProcessOps,
};
use super::*;
use std::{cell::Cell, os::unix::process::ExitStatusExt, process::Command};

struct FakeOps {
    live: IdentityStatus,
    state: libc::proc_bsdinfo,
    signals: Cell<usize>,
    enumerations: Cell<usize>,
    observations: Cell<usize>,
    replace_after_signal: bool,
    replace_after_enumeration: bool,
    reject_signal: bool,
}

impl FakeOps {
    fn new(identity: &ProcessIdentity, live: IdentityStatus) -> Self {
        let mut state: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let start = identity.start_token.parse::<u64>().unwrap();
        state.pbi_pid = identity.pid;
        state.pbi_start_tvsec = start / 1_000_000;
        state.pbi_start_tvusec = start % 1_000_000;
        state.pbi_status = libc::SSTOP;
        Self {
            live,
            state,
            signals: Cell::new(0),
            enumerations: Cell::new(0),
            observations: Cell::new(0),
            replace_after_signal: false,
            replace_after_enumeration: false,
            reject_signal: false,
        }
    }
}

impl ProcessOps for FakeOps {
    fn probe(&self, _: &ProcessIdentity) -> IdentityStatus {
        IdentityStatus::VerifiedMatch
    }
    fn probe_live(&self, _: &ProcessIdentity) -> IdentityStatus {
        self.live
    }
    fn observe(&self, _: u32) -> io::Result<ObservedProcess> {
        self.observations.set(self.observations.get() + 1);
        let replaced = (self.replace_after_signal && self.signals.get() > 0)
            || (self.replace_after_enumeration && self.enumerations.get() > 0);
        Ok(ObservedProcess {
            bsd: self.state,
            unique_id: if replaced { 502 } else { 501 },
            pid_version: if replaced { 8 } else { 7 },
        })
    }
    fn signal(&self, _: &OwnedProcess, _: i32) -> io::Result<()> {
        self.signals.set(self.signals.get() + 1);
        if self.reject_signal {
            Err(io::Error::from_raw_os_error(libc::ESRCH))
        } else {
            Ok(())
        }
    }
    fn children(&self, _: u32) -> io::Result<Vec<u32>> {
        self.enumerations.set(self.enumerations.get() + 1);
        Ok(vec![12345])
    }
}

fn identity() -> ProcessIdentity {
    ProcessIdentity {
        pid: 1234,
        start_token: "1234567".into(),
        boot_id: "11111111-1111-1111-1111-111111111111".into(),
    }
}

#[test]
fn owned_tree_live_mismatch_or_indeterminate_never_signals_or_enumerates() {
    for status in [IdentityStatus::Mismatch, IdentityStatus::Indeterminate] {
        let identity = identity();
        let ops = FakeOps::new(&identity, status);
        let mut owned = Vec::new();
        let result = freeze_tree_with_ops(
            identity,
            &mut owned,
            &|_| panic!("must not capture a descendant"),
            &|_| panic!("must not read a stopped state"),
            &ops,
        );
        assert_eq!(result.is_err(), status == IdentityStatus::Indeterminate);
        assert!(owned.is_empty());
        assert_eq!(ops.observations.get(), 0);
        assert_eq!(ops.signals.get(), 0);
        assert_eq!(ops.enumerations.get(), 0);
    }
}

#[test]
fn owned_tree_replacement_stopped_state_is_never_enumerated() {
    let identity = identity();
    let ops = FakeOps::new(&identity, IdentityStatus::VerifiedMatch);
    let mut replacement = ops.state;
    replacement.pbi_start_tvusec += 1;
    let mut owned = Vec::new();
    freeze_tree_with_ops(
        identity,
        &mut owned,
        &|_| panic!("must not capture a replacement child"),
        &|_| Some(replacement),
        &ops,
    )
    .unwrap();
    assert_eq!(ops.signals.get(), 1);
    assert_eq!(ops.enumerations.get(), 0);
}

#[test]
fn owned_tree_generation_change_after_verification_never_signals_replacement() {
    let identity = identity();
    let mut ops = FakeOps::new(&identity, IdentityStatus::VerifiedMatch);
    ops.reject_signal = true;
    ops.replace_after_signal = true;
    let mut owned = Vec::new();
    freeze_tree_with_ops(
        identity,
        &mut owned,
        &|_| panic!("must not capture a descendant"),
        &|_| panic!("must not read a stopped state"),
        &ops,
    )
    .unwrap();
    assert_eq!(
        ops.signals.get(),
        1,
        "the kernel rejected the old generation"
    );
    assert_eq!(ops.observations.get(), 2);
    assert_eq!(ops.enumerations.get(), 0);
    assert!(owned.is_empty());
}

#[test]
fn owned_tree_termination_refuses_unverified_or_replaced_processes() {
    let identity = identity();
    let process = OwnedProcess {
        identity: identity.clone(),
        unique_id: 501,
        pid_version: 7,
    };
    for status in [IdentityStatus::Mismatch, IdentityStatus::Indeterminate] {
        let ops = FakeOps::new(&identity, status);
        let result = owned_tree_process::terminate(&process, &ops);
        assert_eq!(result.is_err(), status == IdentityStatus::Indeterminate);
        assert_eq!(ops.signals.get(), 0);
        assert_eq!(ops.enumerations.get(), 0);
    }
    let mut ops = FakeOps::new(&identity, IdentityStatus::VerifiedMatch);
    ops.replace_after_signal = true;
    ops.signals.set(1); // The earlier successful stop belonged to the old process.
    owned_tree_process::terminate(&process, &ops).unwrap();
    assert_eq!(ops.signals.get(), 1, "must not kill a recycled PID");
    assert_eq!(ops.enumerations.get(), 0);
}

#[test]
fn owned_tree_discards_child_listing_if_parent_changes_during_enumeration() {
    let identity = identity();
    let mut ops = FakeOps::new(&identity, IdentityStatus::VerifiedMatch);
    ops.replace_after_enumeration = true;
    let mut owned = Vec::new();
    freeze_tree_with_ops(
        identity,
        &mut owned,
        &|_| panic!("must not capture replacement children"),
        &|_| Some(ops.state),
        &ops,
    )
    .unwrap();
    assert_eq!(ops.signals.get(), 1);
    assert_eq!(ops.enumerations.get(), 1);
}

#[test]
fn owned_tree_kernel_rejects_stale_generation_and_terminates_exact_child() {
    let mut child =
        crate::native::OwnedChild::new(Command::new("/bin/sleep").arg("300").spawn().unwrap());
    let identity = ProcessIdentity::capture(child.child.id()).unwrap();
    let (current, _) = owned_tree_process::acquire(&identity, &NativeOps)
        .unwrap()
        .unwrap();
    let mut stale = current.clone();
    stale.pid_version = stale.pid_version.wrapping_add(1);
    assert_eq!(
        NativeOps
            .signal(&stale, libc::SIGSTOP)
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ESRCH)
    );
    assert_eq!(probe(&identity), IdentityStatus::VerifiedMatch);
    NativeOps.signal(&current, libc::SIGSTOP).unwrap();
    owned_tree_process::terminate(&current, &NativeOps).unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.signal(), Some(libc::SIGKILL));
}

#[test]
fn uncaptured_owned_root_waits_for_the_kernel_stop_event() {
    let mut command = Command::new("/bin/cat");
    command.stdin(std::process::Stdio::piped());
    let mut child = crate::native::OwnedChild::new(command.spawn().unwrap());
    let pid = child.child.id();
    let mut owned = Vec::new();
    freeze_uncaptured_root(&child.child, &mut owned).unwrap();
    assert_eq!(info(pid).unwrap().pbi_status, libc::SSTOP);
    assert!(owned.is_empty());
    child.child.kill().unwrap();
    assert_eq!(
        child.wait_with_output().unwrap().status.signal(),
        Some(libc::SIGKILL)
    );
}
