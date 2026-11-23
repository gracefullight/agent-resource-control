//! The offline attribution fixture, its ownership ([`Lifeline`]), the
//! daemon's opt-in probe of the fixture's own processes, and the native CPU
//! oracle of those processes.
use crate::{
    journey::{self, Sandbox},
    runtime::{child_watchdog, instrumentation, Fixture},
};
use arc_core::process_identity::ProcessIdentity;
use std::{
    collections::BTreeMap,
    fs,
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

/// The fixture on the sandbox PATH as `cargo`: the managed root of the
/// hook-returned `cargo build`, and the non-agent burner.
pub const PROGRAM: &str = "cargo";
/// An agent executable name of the fixed adapter-derived list.
pub const AGENT: &str = "claude";

pub struct Programs {
    pub burner: PathBuf,
    pub agent: PathBuf,
}

/// Compile `attribution.c` onto the sandbox PATH with a cleared environment,
/// and link it under an agent's name in a directory outside that PATH.
pub fn compile(f: &Fixture, s: &Sandbox) -> Programs {
    let source = f.root.join("attribution.c");
    fs::write(&source, include_str!("attribution.c")).unwrap();
    let burner = s.bin.join(PROGRAM);
    let mut compiler = Command::new("/usr/bin/cc");
    compiler
        .env_clear()
        .env("HOME", &s.home)
        .env("PATH", "/usr/bin:/bin");
    instrumentation::forward(&mut compiler);
    compiler
        .args(["-O1", "-Wall", "-Werror", "-o"])
        .arg(&burner)
        .arg(&source);
    let output = child_watchdog::output(&mut compiler, "attribution fixture compiler").unwrap();
    assert!(output.status.success(), "{output:?}");
    let agents = f.root.join("agent-bin");
    fs::create_dir_all(&agents).unwrap();
    let agent = agents.join(AGENT);
    // A copy, never a symlink: the kernel names a process after the file
    // it executed.
    fs::copy(&burner, &agent).unwrap();
    Programs { burner, agent }
}

/// The fixture set this test owns, established before any fixture process
/// starts (see `attribution.c`): every fixture process and descendant
/// holds a shared lock on the members file and exits once the test no
/// longer holds its exclusive lock on the lifeline. Dropping the set (on
/// success, a failed assertion or a watchdog's panic alike) releases the
/// lifeline and waits, with a 60 s watchdog, until no member is left.
pub struct Lifeline {
    lock: Option<fs::File>,
    path: PathBuf,
    members: PathBuf,
}

fn flock(file: &fs::File, operation: libc::c_int) -> bool {
    // SAFETY: a valid open descriptor and a plain flock operation.
    unsafe { libc::flock(file.as_raw_fd(), operation) == 0 }
}

impl Lifeline {
    pub fn new(f: &Fixture) -> Self {
        let path = f.root.join("lifeline");
        let members = f.root.join("members");
        fs::write(&members, b"").unwrap();
        // Close-on-exec: no child ever holds the test's lock.
        let lock = fs::File::create(&path).unwrap();
        assert!(flock(&lock, libc::LOCK_EX | libc::LOCK_NB));
        Self {
            lock: Some(lock),
            path,
            members,
        }
    }
    /// Make `command`'s fixture processes members of this set.
    pub fn arm<'c>(&self, command: &'c mut Command) -> &'c mut Command {
        command
            .env("ARC_ATTRIBUTION_LIFELINE", &self.path)
            .env("ARC_ATTRIBUTION_MEMBERS", &self.members)
    }
    pub fn members(&self) -> &Path {
        &self.members
    }
    /// End the set and wait until every member exited.
    pub fn cancel(&mut self) -> Result<(), String> {
        drop(self.lock.take());
        let deadline = Instant::now() + Duration::from_secs(60);
        while members_present(&self.members) {
            if Instant::now() >= deadline {
                return Err(format!(
                    "fixture processes still hold {} after 60 s",
                    self.members.display()
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
}
impl Drop for Lifeline {
    fn drop(&mut self) {
        if self.lock.is_none() {
            return;
        }
        if let Err(error) = self.cancel() {
            if std::thread::panicking() {
                eprintln!("{error}");
            } else {
                panic!("{error}");
            }
        }
    }
}

/// Whether any fixture process of the set behind `members` is alive.
pub fn members_present(members: &Path) -> bool {
    let file = fs::File::open(members).unwrap();
    let free = flock(&file, libc::LOCK_EX | libc::LOCK_NB);
    if free {
        flock(&file, libc::LOCK_UN);
    }
    !free
}

/// The opt-in wall-clock shift file of the daemons started here.
pub fn wall_shift(f: &Fixture) -> PathBuf {
    f.root.join("wall-shift")
}

/// Start the production daemon with an opt-in self-observer socket, the
/// opt-in wall-clock shift of its attribution scanner ([`wall_shift`]) and
/// the policy's fastest valid active cadence (`ARC_METER_ACTIVE_1S`).
pub fn daemon(f: &Fixture, socket: &str) -> crate::runtime::OwnedChild {
    let mut command = f.isolated_command("");
    command
        .env("ARC_ATTRIBUTION_SELF_OBSERVE", f.root.join(socket))
        .env("ARC_ATTRIBUTION_WALL_SHIFT_FILE", wall_shift(f))
        // The policy's 1 s active cadence: half the wait per observed scan.
        .env("ARC_METER_ACTIVE_1S", "1");
    let mut daemon = crate::runtime::OwnedChild::new(command.spawn().unwrap());
    f.wait_for(&mut daemon, "arc-daemon ready.");
    journey::eventually("self-observer socket ready", || {
        f.root.join(socket).exists()
    });
    daemon
}

/// Start `exe args..` detached from the test's ancestry (see attribution.c),
/// as a member of `lifeline`'s set.
pub fn detach(
    f: &Fixture,
    s: &Sandbox,
    cwd: &Path,
    programs: &Programs,
    lifeline: &Lifeline,
    exe: &Path,
    args: &[&Path],
) {
    let mut command = s.command(f, &programs.burner, cwd);
    lifeline
        .arm(&mut command)
        .env("ARC_ATTRIBUTION_SELF_OBSERVE", f.root.join("self.sock"))
        .arg("detach")
        .arg(exe)
        .args(args)
        .stdin(std::process::Stdio::null());
    let output = child_watchdog::output(&mut command, "detached fixture launcher").unwrap();
    assert!(output.status.success(), "{output:?}");
}

/// A fixture process writes only the observations it receives about itself.
pub fn observation(directory: &Path, role: &str) -> PathBuf {
    directory.join(format!("{role}.observation"))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Class {
    pub confidence: String,
    pub vendor: String,
    pub cpu_core_seconds: f64,
}

/// The scanner's pass that read a fixture process: its sequence number and
/// the wall-clock bounds of its native scan, in whole ms (rounded outward).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scan {
    pub seq: u64,
    pub start_ms: u64,
    pub end_ms: u64,
}

/// One line the daemon sent a fixture process about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// A scan counted `delta_ns` since this identity's previous scan; the
    /// process's cumulative CPU then read `cpu_ns`. The delta was committed
    /// in the interval `[interval.0, interval.1)`.
    Counted {
        confidence: String,
        vendor: String,
        delta_ns: u64,
        cpu_ns: u64,
        scan: Scan,
        interval: (u64, u64),
    },
    /// A first sighting of this identity: it contributed nothing.
    First {
        confidence: String,
        vendor: String,
        cpu_ns: u64,
        scan: Scan,
    },
    /// Pass `seq` failed at `at_ms` and counted nothing.
    Failed { seq: u64, at_ms: u64 },
    /// This peer was present in a completed native baseline scan.
    BaselineScanned(Scan),
    /// The baseline persistence following that scan failed.
    BaselineWriteFailed { scan: Scan, cause: String },
}
impl Line {
    pub fn scan(&self) -> Option<Scan> {
        match self {
            Line::Counted { scan, .. } | Line::First { scan, .. } => Some(*scan),
            Line::Failed { .. } | Line::BaselineScanned(_) | Line::BaselineWriteFailed { .. } => {
                None
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    pub lines: Vec<Line>,
    /// Counted scans.
    pub scans: u64,
    /// The counted CPU per class.
    pub classes: Vec<Class>,
}
impl Probe {
    pub fn total(&self) -> f64 {
        self.classes.iter().map(|c| c.cpu_core_seconds).sum()
    }
    /// The one class of a probe that only ever saw one.
    #[track_caller]
    pub fn only(&self) -> (&str, &str) {
        assert_eq!(self.classes.len(), 1, "{self:?}");
        (&self.classes[0].confidence, &self.classes[0].vendor)
    }
    /// The counted nanoseconds over the lines from `from` on.
    pub fn counted_ns(&self, from: usize) -> u64 {
        self.lines[from..]
            .iter()
            .map(|line| match line {
                Line::Counted { delta_ns, .. } => *delta_ns,
                _ => 0,
            })
            .sum()
    }
    pub fn cpu_ns(line: &Line) -> Option<u64> {
        match line {
            Line::Counted { cpu_ns, .. } | Line::First { cpu_ns, .. } => Some(*cpu_ns),
            Line::Failed { .. } | Line::BaselineScanned(_) | Line::BaselineWriteFailed { .. } => {
                None
            }
        }
    }
}

fn vendor(field: &str) -> String {
    if field == "-" { "" } else { field }.to_owned()
}

/// What the daemon has reported for a request; `None` until it sent a line.
pub fn probe(path: &Path) -> Option<Probe> {
    let text = fs::read_to_string(path).ok()?;
    let mut totals: BTreeMap<(String, String), u64> = BTreeMap::new();
    let mut lines = Vec::new();
    for line in text
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
    {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let number = |field: &str| field.parse::<u64>().unwrap();
        let scan = |seq: &str, start: &str, end: &str| Scan {
            seq: number(seq),
            start_ms: number(start),
            end_ms: number(end),
        };
        let parsed = match fields.as_slice() {
            ["counted", confidence, vendor_field, delta, cpu, seq, start, end, from, to] => {
                Line::Counted {
                    confidence: (*confidence).to_owned(),
                    vendor: vendor(vendor_field),
                    delta_ns: number(delta),
                    cpu_ns: number(cpu),
                    scan: scan(seq, start, end),
                    interval: (number(from), number(to)),
                }
            }
            ["first", confidence, vendor_field, cpu, seq, start, end] => Line::First {
                confidence: (*confidence).to_owned(),
                vendor: vendor(vendor_field),
                cpu_ns: number(cpu),
                scan: scan(seq, start, end),
            },
            ["failed", seq, at] => Line::Failed {
                seq: number(seq),
                at_ms: number(at),
            },
            ["baseline_scanned", seq, start, end] => Line::BaselineScanned(scan(seq, start, end)),
            ["baseline_write_failed", seq, start, end, cause] => Line::BaselineWriteFailed {
                scan: scan(seq, start, end),
                cause: (*cause).to_owned(),
            },
            other => panic!("unexpected observation line {other:?}"),
        };
        if let Line::Counted {
            confidence,
            vendor,
            delta_ns,
            ..
        } = &parsed
        {
            *totals
                .entry((confidence.clone(), vendor.clone()))
                .or_default() += delta_ns;
        }
        lines.push(parsed);
    }
    let scans = lines
        .iter()
        .filter(|line| matches!(line, Line::Counted { .. }))
        .count() as u64;
    (!lines.is_empty()).then(|| Probe {
        lines,
        scans,
        classes: totals
            .into_iter()
            .map(|((confidence, vendor), ns)| Class {
                confidence,
                vendor,
                cpu_core_seconds: ns as f64 / 1e9,
            })
            .collect(),
    })
}

/// Wait until the daemon has counted `scans` scans of the request.
pub fn scanned(path: &Path, scans: u64) -> Probe {
    let mut read = None;
    journey::eventually(
        &format!("{scans} probed scans of {}", path.display()),
        || {
            read = probe(path).filter(|p| p.scans >= scans);
            read.is_some()
        },
    );
    read.unwrap()
}

/// The native CPU time of a live fixture process, read independently of the
/// daemon from its own identity; `None` once it is gone.
pub fn cpu_seconds(identity: &ProcessIdentity) -> Option<f64> {
    arc_platform::process_tree::read_tree(identity)
        .into_iter()
        .find(|p| p.identity == *identity)
        .map(|p| p.cpu_time_ns as f64 / 1e9)
}
