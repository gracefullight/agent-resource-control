//! Ownership of the attribution fixture: a failed assertion leaves no
//! fixture process behind.
use crate::{fixture, journey::Sandbox, runtime::Fixture, summary::identity, target_serial, tree};
use arc_core::process_identity::{probe_live, IdentityStatus};
use std::{
    os::unix::net::UnixListener,
    panic::{catch_unwind, AssertUnwindSafe},
    path::Path,
};

/// A failed assertion while the respawner's PID-cycling workers, its
/// low-CPU role and a detached burner run: the set owned before they started
/// ends with the unwinding test, and no fixture process is left behind.
#[test]
fn fixture_ownership_reaps_every_descendant_when_an_assertion_fails() {
    let _serial = target_serial::acquire();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    let programs = fixture::compile(&f, &s);
    // An observer that accepts nothing: the fixtures only connect to it.
    let _observer = UnixListener::bind(f.root.join("self.sock")).unwrap();
    let lifeline = fixture::Lifeline::new(&f);
    let members = lifeline.members().to_owned();
    let respawn_dir = tree::directory(&f, "respawn");
    let outside_dir = tree::directory(&f, "outside");
    let mut seen = Vec::new();
    let mut cycling = false;
    let failed = catch_unwind(AssertUnwindSafe(|| {
        let lifeline = lifeline;
        let respawn_args = [Path::new("respawn"), &respawn_dir, &programs.burner];
        fixture::detach(
            &f,
            &s,
            &project,
            &programs,
            &lifeline,
            &programs.burner,
            &respawn_args,
        );
        let outside_args = [Path::new("burn"), Path::new("outside"), &outside_dir];
        fixture::detach(
            &f,
            &s,
            &project,
            &programs,
            &lifeline,
            &programs.burner,
            &outside_args,
        );
        for (dir, name) in [
            (&respawn_dir, "respawner.pid"),
            (&respawn_dir, "respawn-1.pid"),
            (&outside_dir, "outside.pid"),
        ] {
            seen.push(identity(tree::published(dir, name)));
        }
        assert!(fixture::members_present(&members));
        cycling = !respawn_dir.join("cycled").exists();
        panic!("deliberate assertion failure while the PID cycle runs");
    }));
    assert!(failed.is_err());
    assert_eq!(seen.len(), 3);
    for id in &seen {
        assert_ne!(probe_live(id), IdentityStatus::VerifiedMatch, "{id:?}");
    }
    assert!(
        !fixture::members_present(&members),
        "a burner or worker outlived the failed test"
    );
    assert!(
        cycling,
        "the PID-cycling workers were running at the failure"
    );
}
