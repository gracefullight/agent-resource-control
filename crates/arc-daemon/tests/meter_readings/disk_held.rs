//! The isolated built-CLI environment and the one held managed job of MTR-001.
//!
//! Nothing is compiled here: only the workspace's prebuilt `arcctl` and
//! `arc-exec-fixture` run, with a cleared environment, temporary HOME,
//! explicit socket and configuration paths and the forwarded coverage file.
//! Instead of an interface interposer, every `arcctl` argv is restricted to
//! `meter snapshot --json` and `exec -- cargo build`. The CLI's endpoint
//! discovery (`arc-cli/src/exec/endpoint_context.rs`), the only code that
//! inventories host network interfaces, runs only for `ssh`, `docker`,
//! `kubectl` and `act`, and enumerates interfaces only for `ssh`.
use crate::{
    journey,
    runtime::{child_watchdog, instrumentation, Fixture, OwnedChild},
    support,
};
use arc_core::{
    meter::{MeterSnapshot, MAX_SNAPSHOT_BYTES},
    process_identity::{probe, probe_live, IdentityStatus, ProcessIdentity},
};
use std::{
    fs::{self, File},
    io::{Read, Write},
    os::unix::{
        fs::symlink,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const SNAPSHOT: &[&str] = &["meter", "snapshot", "--json"];
const BUILD: &[&str] = &["exec", "--", "cargo", "build"];

pub struct Isolated {
    pub home: PathBuf,
    bin: PathBuf,
    project: PathBuf,
    socket: PathBuf,
    arcctl: PathBuf,
}

fn prebuilt(f: &Fixture, name: &str) -> PathBuf {
    let binary = f.binary(name);
    assert!(
        binary.is_file(),
        "prebuilt workspace {name} is required: {}",
        binary.display()
    );
    binary
}

impl Isolated {
    pub fn new(f: &Fixture) -> Self {
        let root = f.root.join("disk-cli");
        let (home, bin, project) = (root.join("home"), root.join("bin"), root.join("project"));
        for directory in [home.join("gh"), home.join("xdg"), bin.clone()] {
            fs::create_dir_all(directory).unwrap();
        }
        fs::write(home.join("global"), "").unwrap();
        for directory in ["objects/info", "objects/pack", "refs/heads", "refs/tags"] {
            fs::create_dir_all(project.join(".git").join(directory)).unwrap();
        }
        fs::write(project.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(project.join(".git/config"), "").unwrap();
        symlink(prebuilt(f, "arc-exec-fixture"), bin.join("cargo")).unwrap();
        Self {
            home,
            bin,
            project,
            socket: f.socket.clone(),
            arcctl: prebuilt(f, "arcctl"),
        }
    }

    fn arcctl(&self, args: &[&str], cwd: &Path) -> Command {
        assert!(
            [SNAPSHOT, BUILD].contains(&args),
            "argv outside the interface-free CLI paths: {args:?}"
        );
        let mut command = Command::new(&self.arcctl);
        command
            .args(args)
            .env_clear()
            .env(arc_core::ARC_SOCKET_PATH, &self.socket)
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join("xdg"))
            .env("GH_CONFIG_DIR", self.home.join("gh"))
            .env("GIT_CONFIG_GLOBAL", self.home.join("global"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("PATH", &self.bin)
            .current_dir(cwd);
        instrumentation::forward(&mut command);
        command
    }

    /// One typed `arcctl meter snapshot --json` document from the real daemon.
    pub fn snapshot(&self) -> MeterSnapshot {
        let output = child_watchdog::output(
            &mut self.arcctl(SNAPSHOT, &self.home),
            "disk meter snapshot",
        )
        .unwrap();
        assert!(
            output.status.success() && output.stderr.is_empty(),
            "{output:?}"
        );
        let json = output.stdout.strip_suffix(b"\n").expect("snapshot newline");
        assert!(json.len() <= MAX_SNAPSHOT_BYTES);
        let mut value: serde_json::Value = serde_json::from_slice(json).unwrap();
        if let Some(traffic) = value
            .as_object_mut()
            .unwrap()
            .remove(arc_ipc::meter::usage::SNAPSHOT_TRAFFIC_KEY)
        {
            serde_json::from_value::<arc_ipc::meter::usage::SnapshotTraffic>(traffic).unwrap();
        }
        serde_json::from_value(value).unwrap()
    }
}

/// Accept the target's raw `ready` on its control socket, under a 60 s watchdog.
fn ready(listener: &UnixListener) -> UnixStream {
    listener.set_nonblocking(true).unwrap();
    let end = Instant::now() + Duration::from_secs(60);
    let (mut stream, _) = loop {
        match listener.accept() {
            Ok(accepted) => break accepted,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < end, "held target never reached control");
                thread::sleep(Duration::from_millis(1));
            }
            Err(e) => panic!("{e}"),
        }
    };
    stream.set_nonblocking(false).unwrap();
    let timeout = Some(Duration::from_secs(60));
    stream.set_read_timeout(timeout).unwrap();
    stream.set_write_timeout(timeout).unwrap();
    let mut message = [0u8; 5];
    stream.read_exact(&mut message).unwrap();
    assert_eq!(&message, b"ready");
    stream
}

/// Keep Meter active with one real managed `cargo build`. The wrapper is
/// spawned with inherited cancel and lifetime channels, so the separately
/// grouped target is owned before it exists: dropping the owner cancels it
/// even before it publishes its identity, then waits until every inherited
/// lifetime writer, wrapper and target alike, has closed.
pub struct Held {
    wrapper: OwnedChild,
    owners: PathBuf,
    marker: PathBuf,
    stderr: PathBuf,
    control: Option<UnixStream>,
}

impl Held {
    fn launch(f: &Fixture, s: &Isolated, checkpoint: Option<&Path>) -> (Self, UnixListener) {
        let control = f.root.join("disk-held-control");
        let listener = UnixListener::bind(&control).unwrap();
        let (owners, marker, stderr) = (
            f.root.join("disk-held-owners"),
            f.root.join("disk-held-starts"),
            f.root.join("disk-held-stderr"),
        );
        let mut command = s.arcctl(BUILD, &s.project);
        command
            .env("ARC_FIXTURE_ACCOUNTING", "controlled-success")
            .env("ARC_METER_TEST_CONTROL", &control)
            .env("ARC_METER_TEST_MARKER", &marker)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(File::create(&stderr).unwrap());
        if let Some(checkpoint) = checkpoint {
            command.env("ARC_FIXTURE_BEFORE_IDENTITY", checkpoint);
        }
        let wrapper = OwnedChild::spawn_with_descendants(command, owners.clone()).unwrap();
        let held = Self {
            wrapper,
            owners,
            marker,
            stderr,
            control: None,
        };
        (held, listener)
    }

    pub fn start(f: &Fixture, s: &Isolated) -> Self {
        let (mut held, listener) = Self::launch(f, s, None);
        // The target publishes its identity before it reports ready.
        held.control = Some(ready(&listener));
        let db = journey::database(f);
        journey::eventually("daemon attaches the disk fixture job", || {
            assert!(
                held.wrapper.child.try_wait().unwrap().is_none(),
                "disk fixture wrapper exited before attachment"
            );
            match journey::queue_state(&db, held.wrapper.child.id()).as_deref() {
                Some("RUNNING") => true,
                Some("RESERVED") | None => false,
                other => panic!("unexpected disk fixture job state: {other:?}"),
            }
        });
        held
    }

    /// Identities the target published in the owner-supplied directory.
    fn published(&self) -> Vec<ProcessIdentity> {
        fs::read_dir(&self.owners)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|e| e == "json"))
            .map(|path| serde_json::from_slice(&fs::read(path).unwrap()).unwrap())
            .collect()
    }

    /// Release the target; the managed command succeeds and ran exactly once.
    pub fn finish(mut self) {
        self.control.as_mut().unwrap().write_all(b"r").unwrap();
        let output = self.wrapper.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{:?}: {:?}",
            output.status,
            fs::read_to_string(&self.stderr)
        );
        assert_eq!(journey::starts(&self.marker), 1);
    }
}

fn wait_gone(identity: &ProcessIdentity, label: &str) {
    journey::eventually(label, || {
        matches!(
            probe(identity),
            IdentityStatus::Missing | IdentityStatus::Mismatch
        )
    });
}

#[test]
fn disk_held_job_target_is_killed_on_drop() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Isolated::new(&f);
    let mut daemon = f.spawn_isolated("");
    let held = Held::start(&f, &s);
    let [target] = &held.published()[..] else {
        panic!("one published target: {:?}", held.published());
    };
    let target = target.clone();
    let wrapper = ProcessIdentity::capture(held.wrapper.child.id()).unwrap();
    assert_ne!(target.pid, wrapper.pid);
    assert_eq!(probe_live(&target), IdentityStatus::VerifiedMatch);
    drop(held);
    wait_gone(&target, "held target exits after its owner drops");
    wait_gone(&wrapper, "held wrapper exits after its owner drops");
    daemon.stop(libc::SIGTERM);
    f.report();
}

/// The test unwinds while the target is alive but has published nothing.
/// Ownership established before spawn still stops target and wrapper. The
/// target's own pre-publication watchdog would end it only after 60 s, so an
/// exit well inside that interval is the owner's cancellation, not expiry.
#[test]
fn disk_held_job_interrupted_before_identity_publication_exits() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Isolated::new(&f);
    let mut daemon = f.spawn_isolated("");
    let checkpoint = f.root.join("disk-before-identity");
    let seen = std::cell::RefCell::new(Vec::new());
    let reached = std::cell::Cell::new(None);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (held, _listener) = Held::launch(&f, &s, Some(&checkpoint));
        let mut pid = None;
        journey::eventually("target reaches its pre-identity checkpoint", || {
            pid = fs::read_to_string(&checkpoint)
                .ok()
                .and_then(|text| text.parse::<u32>().ok());
            pid.is_some()
        });
        reached.set(Some(Instant::now()));
        let target = ProcessIdentity::capture(pid.unwrap()).unwrap();
        let wrapper = ProcessIdentity::capture(held.wrapper.child.id()).unwrap();
        assert_ne!(target.pid, wrapper.pid);
        assert!(held.published().is_empty(), "identity already published");
        assert_eq!(probe_live(&target), IdentityStatus::VerifiedMatch);
        seen.borrow_mut().extend([target, wrapper]);
        panic!("interrupt the disk fixture before identity publication");
    }));
    assert!(result.is_err());
    let seen = seen.into_inner();
    assert_eq!(seen.len(), 2, "the fixture was interrupted too early");
    wait_gone(&seen[0], "unpublished target exits during unwind");
    wait_gone(&seen[1], "wrapper exits during unwind");
    let elapsed = reached.get().unwrap().elapsed();
    assert!(
        elapsed < Duration::from_secs(30),
        "target outlived its owner for {elapsed:?}; only its own watchdog ended it"
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}
