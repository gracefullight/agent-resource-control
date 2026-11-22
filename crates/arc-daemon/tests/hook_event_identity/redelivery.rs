//! One fixture world for the redelivery journeys: the sandbox, a project and
//! the offline `cargo`, with the vendor's side of an execution.
use crate::{
    journey::Sandbox,
    runtime::Fixture,
    stage::{self, Finished, Observed, Run, Tool},
    support,
};
use std::{fs, os::unix::ffi::OsStrExt, path::PathBuf, process::Command};

pub struct World {
    pub f: Fixture,
    pub s: Sandbox,
    pub project: PathBuf,
    pub cargo: Tool,
    // Held until every daemon and child of this world has been reaped.
    _serial: support::Serial,
}

/// One command running in the vendor's shell. The offline cargo it starts
/// reads its standard input to the end, so it runs until `finish`.
pub struct Execution {
    pub run: Run,
    shell: Command,
    name: String,
}

/// A finished execution: its streams and status, and what the offline cargo
/// was started with.
pub struct Done {
    pub finished: Finished,
    pub start: Observed,
}

impl World {
    pub fn new() -> Self {
        let serial = support::serial();
        let f = Fixture::new();
        let s = Sandbox::new(&f);
        let project = s.project("project");
        let cargo = Tool::cargo(&f, &s);
        Self {
            f,
            s,
            project,
            cargo,
            _serial: serial,
        }
    }

    /// Execute `command` as the vendor's shell tool does and leave it running.
    pub fn start(&self, name: &str, command: &str) -> Execution {
        let mut shell = self.s.shell(&self.f, &self.project, command);
        self.cargo.arm(&mut shell);
        let run = Run::start(&mut shell, &self.f.root, name);
        Execution {
            run,
            shell,
            name: name.to_owned(),
        }
    }

    /// Execute `command` to its exit.
    pub fn run(&self, name: &str, command: &str) -> Done {
        self.start(name, command).finish(self)
    }

    /// Where the standard error of the execution `name` is collected.
    pub fn stderr(&self, name: &str) -> Vec<u8> {
        fs::read(self.f.root.join(format!("{name}.stderr"))).unwrap()
    }
}

impl Execution {
    /// Supply the standard input, close it and wait for the exit. The tool's
    /// environment must be exactly the one the command was started with.
    pub fn finish(mut self, w: &World) -> Done {
        self.run.input(stage::INPUT);
        let finished = self.run.finish(&self.name);
        let start = w.cargo.take();
        stage::assert_environment(&start, &self.shell);
        Done { finished, start }
    }
}

/// `managed` ran `cargo build` exactly as `direct`, the literal without ARC:
/// the same arguments, working directory, environment, standard input, output
/// and exit status, and on standard error only `notices` before the tool's
/// own bytes.
#[track_caller]
pub fn assert_unchanged(w: &World, managed: &Done, direct: &Done, notices: &[u8]) {
    assert_eq!(managed.finished.status.code(), Some(stage::TOOL_EXIT));
    assert_eq!(managed.finished.status, direct.finished.status);
    assert_eq!(managed.finished.stdout, stage::TOOL_STDOUT);
    assert_eq!(managed.finished.stdout, direct.finished.stdout);
    assert_eq!(
        String::from_utf8_lossy(&managed.finished.stderr),
        String::from_utf8_lossy(&[notices, &direct.finished.stderr[..]].concat())
    );
    assert_eq!(direct.finished.stderr, stage::TOOL_STDERR);
    stage::assert_same_start(&managed.start, &direct.start);
    assert_eq!(managed.start.args, [b"build".to_vec()]);
    assert_eq!(
        managed.start.cwd,
        fs::canonicalize(&w.project).unwrap().as_os_str().as_bytes()
    );
    assert_eq!(managed.start.stdin, stage::INPUT);
}
