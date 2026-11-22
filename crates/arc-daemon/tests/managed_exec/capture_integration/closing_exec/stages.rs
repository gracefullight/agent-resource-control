//! Mixed command stages through the installed hook (AT-TRF-12, DET-016).
//! Lane B owns this module; `closing_exec.rs` only registers it.
use super::*;

// Shared with the `arcctl exec` route's tests, which use the other subset.
#[allow(clippy::duplicate_mod)]
#[path = "../closing_shell/stages/admission.rs"]
mod admission;
#[allow(clippy::duplicate_mod, dead_code)]
#[path = "../closing_shell/stages/harness.rs"]
mod harness;
use harness::{installed_hook_rewrite, Route, BUN, MAKE};

// AT-TRF-12 item 1, DET-016 item 1.
#[test]
fn mixed_bun_stages_via_hook() {
    admission::stage_by_stage(Route::Hook("/bin/sh"), BUN);
}

// DET-016 item 2: the literal Makefile recipe `cargo test`.
#[test]
fn mixed_make_stages_via_hook() {
    admission::stage_by_stage(Route::Hook("/bin/zsh"), MAKE);
}

// Contract seam: a compound that cannot be separated safely is left to the
// vendor's shell, and an existing literal keeps its one-child rewrite.
#[test]
fn stage_seam_installed_hook_leaves_unseparable_compounds_unrewritten() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    for source in [
        "bun test $(printf x) && gh workflow run ci.yml",
        "bun test && gh workflow run ci.yml > dispatch.log",
        "bun test && gh workflow run \"$WORKFLOW\"",
        "bash -lc 'bun test && gh workflow run ci.yml'",
        "make test `printf x` && gh workflow run ci.yml",
    ] {
        assert_eq!(installed_hook_rewrite(&s, &f, source), None, "{source}");
    }
    let literal = installed_hook_rewrite(&s, &f, "cargo build").unwrap();
    // Still one child; the recorded event's ids name it (AT-TRF-09).
    assert!(
        literal.contains(" hook-exec --arc-event ") && literal.ends_with(" -- cargo build"),
        "{literal}"
    );
    assert!(!literal.contains("--arc-request"), "{literal}");
    assert_eq!(s.calls(), 0);
}
