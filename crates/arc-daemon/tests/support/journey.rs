//! Isolated built-CLI fixtures shared by the Phase 1 journey targets.
//!
//! Every process started here gets a cleared environment, a temporary HOME,
//! explicit configuration paths and only the coverage destination of the
//! test runner. Tools are offline stubs. [`Sandbox::new`] also interposes a
//! synthetic interface inventory compiled for it; [`Sandbox::provisioned`]
//! launches no build or compiler and serves journeys whose commands never
//! reach endpoint discovery, the CLI's only interface reader.
#![allow(dead_code)] // Each journey target uses the subset it needs.
use crate::runtime::{child_watchdog, instrumentation, Fixture};
use std::{
    ffi::OsStr,
    fs,
    io::{Read, Write},
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

/// The built `arcctl` or `arc-exec-fixture` beside the daemon under test.
pub fn binary(name: &str) -> PathBuf {
    static BUILT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    let daemon = Path::new(env!("CARGO_BIN_EXE_arc-daemon"));
    BUILT.get_or_init(|| {
        let profile = daemon.parent().expect("daemon profile directory");
        let target = profile.parent().expect("Cargo target directory");
        let mut build = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
        build
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args(["build", "--offline", "-p", "arc-cli", "--bin", "arcctl"])
            .args(["--bin", "arc-exec-fixture", "--target-dir"])
            .arg(target);
        if profile.file_name().is_some_and(|name| name == "release") {
            build.arg("--release");
        }
        let output = child_watchdog::output_for(
            &mut build,
            "offline CLI fixture build",
            Duration::from_secs(7200),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "offline CLI fixture build failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    });
    let path = daemon.with_file_name(name);
    assert!(path.is_file(), "missing built {}", path.display());
    path
}

/// An already provisioned binary beside the daemon under test. Nothing is
/// built here: a missing binary is a prerequisite failure naming its fix.
pub fn provisioned(name: &str) -> PathBuf {
    let daemon = Path::new(env!("CARGO_BIN_EXE_arc-daemon"));
    let path = daemon.with_file_name(name);
    assert!(
        path.is_file(),
        "prerequisite missing: {} is not provisioned beside {}. Build the workspace \
         test binaries first (`cargo test --workspace --no-run` builds arc-cli's \
         arcctl and arc-exec-fixture); this test launches no build or compiler.",
        path.display(),
        daemon.display()
    );
    path
}

pub struct Sandbox {
    pub home: PathBuf,
    pub bin: PathBuf,
    root: PathBuf,
    interfaces: Option<PathBuf>,
    provisioned: bool,
}

impl Sandbox {
    fn directories(f: &Fixture) -> (PathBuf, PathBuf, PathBuf) {
        let root = f.root.join("journey");
        let (home, bin) = (root.join("home"), root.join("bin"));
        for directory in [home.join("gh"), home.join("xdg"), bin.clone()] {
            fs::create_dir_all(directory).unwrap();
        }
        fs::write(home.join("global"), "").unwrap();
        (root, home, bin)
    }

    /// Already provisioned `arcctl` and offline fixture, with no loader
    /// injection: no nested build, no compiler. Its journeys run commands
    /// (Cargo, Bun, hook installation, summaries) that do not enter endpoint
    /// discovery, so the CLI never reads a host interface inventory.
    pub fn provisioned(f: &Fixture) -> Self {
        let (root, home, bin) = Self::directories(f);
        provisioned("arcctl");
        provisioned("arc-exec-fixture");
        Self {
            home,
            bin,
            root,
            interfaces: None,
            provisioned: true,
        }
    }

    fn program(&self, name: &str) -> PathBuf {
        if self.provisioned {
            provisioned(name)
        } else {
            binary(name)
        }
    }

    pub fn new(f: &Fixture) -> Self {
        let (root, home, bin) = Self::directories(f);
        let source = root.join("interfaces.c");
        let interfaces = root.join("interfaces-library");
        fs::write(
            &source,
            include_str!("../managed_exec/capture_integration/interfaces.c"),
        )
        .unwrap();
        let mut compiler = Command::new("/usr/bin/cc");
        compiler
            .env_clear()
            .env("HOME", &home)
            .env("PATH", "/usr/bin:/bin");
        instrumentation::forward(&mut compiler);
        if cfg!(target_os = "macos") {
            compiler.arg("-dynamiclib");
        } else {
            compiler.args(["-shared", "-fPIC"]);
        }
        compiler.arg("-o").arg(&interfaces).arg(&source);
        let result = child_watchdog::output(&mut compiler, "fixture interface compiler").unwrap();
        assert!(result.status.success(), "{result:?}");
        Self {
            home,
            bin,
            root,
            interfaces: Some(interfaces),
            provisioned: false,
        }
    }

    /// A distinct project root holding a `.git` directory.
    pub fn project(&self, name: &str) -> PathBuf {
        let project = self.root.join(name);
        for directory in ["objects/info", "objects/pack", "refs/heads", "refs/tags"] {
            fs::create_dir_all(project.join(".git").join(directory)).unwrap();
        }
        fs::write(project.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(project.join(".git/config"), "").unwrap();
        project
    }

    /// Make `program` on the sandbox PATH the offline fixture child.
    pub fn stub(&self, program: &str) {
        symlink(self.program("arc-exec-fixture"), self.bin.join(program)).unwrap();
    }

    /// Any program with the isolated environment, started in `cwd`.
    pub fn command(&self, f: &Fixture, program: impl AsRef<OsStr>, cwd: &Path) -> Command {
        let mut command = Command::new(program);
        command
            .env_clear()
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join("xdg"))
            .env("GH_CONFIG_DIR", self.home.join("gh"))
            .env("GIT_CONFIG_GLOBAL", self.home.join("global"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("PATH", &self.bin)
            .env(
                "ARC_TEST_INTERFACE_MARKER",
                self.home.join("interface-calls"),
            )
            .current_dir(cwd);
        if let Some(interfaces) = &self.interfaces {
            command.env(
                if cfg!(target_os = "macos") {
                    "DYLD_INSERT_LIBRARIES"
                } else {
                    "LD_PRELOAD"
                },
                interfaces,
            );
        }
        instrumentation::forward(&mut command);
        command
    }

    pub fn arcctl(&self, f: &Fixture, cwd: &Path) -> Command {
        self.command(f, self.program("arcctl"), cwd)
    }

    /// The vendor's shell tool: `/bin/sh -c <source>` in `cwd`.
    pub fn shell(&self, f: &Fixture, cwd: &Path, source: &str) -> Command {
        let mut command = self.command(f, "/bin/sh", cwd);
        command.args(["-c", source]);
        command
    }
}

/// Run to exit with the given standard input, under the 60 s child watchdog.
pub fn output_with_input(command: &mut Command, input: &[u8], label: &str) -> Output {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = child_watchdog::Owned::spawn(command).unwrap();
    child.child.stdin.take().unwrap().write_all(input).unwrap();
    let status = child.wait(label).unwrap();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let mut out = child.child.stdout.take().unwrap();
    out.read_to_end(&mut stdout).unwrap();
    let mut err = child.child.stderr.take().unwrap();
    err.read_to_end(&mut stderr).unwrap();
    Output {
        status,
        stdout,
        stderr,
    }
}

/// Wait for observed state; the deadline is a diagnostic watchdog only.
#[track_caller]
pub fn eventually(label: &str, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !check() {
        assert!(Instant::now() < deadline, "{label}: 60 s watchdog");
        thread::sleep(Duration::from_millis(5));
    }
}

pub fn database(f: &Fixture) -> rusqlite::Connection {
    rusqlite::Connection::open(&f.database).unwrap()
}

/// The queue state of the entry owned by the wrapper process `observer`.
pub fn queue_state(db: &rusqlite::Connection, observer: u32) -> Option<String> {
    db.query_row(
        "SELECT t.state FROM traffic_entries t JOIN job_lifecycle l ON l.job_id=t.job_id \
         WHERE l.observer_pid=?1",
        [observer],
        |row| row.get(0),
    )
    .ok()
}

/// Recorded child starts: one line per execution of a marked offline stub.
pub fn starts(marker: &Path) -> usize {
    fs::read_to_string(marker).map_or(0, |text| text.lines().count())
}
