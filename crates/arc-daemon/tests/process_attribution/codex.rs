//! An owned Codex agent outside ARC: a process executing a file named
//! exactly `codex` starts a burner, which the daemon's scan attributes to
//! Codex at Estimated confidence through the executable mapping.
use crate::{
    fixture::{self, Lifeline, Programs},
    journey::Sandbox,
    runtime::Fixture,
    summary::identity,
    tree,
};
use arc_core::process_identity::ProcessIdentity;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct Codex {
    pub dir: PathBuf,
    pub agent: ProcessIdentity,
    pub burner: ProcessIdentity,
}

pub fn start(
    f: &Fixture,
    s: &Sandbox,
    project: &Path,
    programs: &Programs,
    lifeline: &Lifeline,
) -> Codex {
    // A copy, never a symlink: the kernel names a process after the file
    // it executed. It sits beside the Claude-named agent, off the PATH.
    let exe = programs.agent.with_file_name("codex");
    fs::copy(&programs.burner, &exe).unwrap();
    let dir = tree::directory(f, "codex");
    let args = [Path::new("agent"), &dir, &programs.burner];
    fixture::detach(f, s, project, programs, lifeline, &exe, &args);
    Codex {
        agent: identity(tree::published(&dir, "agent.pid")),
        burner: identity(tree::published(&dir, "agent-burner.pid")),
        dir,
    }
}
