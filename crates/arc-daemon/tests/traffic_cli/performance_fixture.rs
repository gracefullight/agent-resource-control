//! Build the identical offline Cargo fixture once, then copy its private state.
use super::{fs, Fixture, OwnedChild, Path, PathBuf};
use std::{
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
    time::Duration,
};

static SEED: OnceLock<Fixture> = OnceLock::new();
static PREPARED: OnceLock<PathBuf> = OnceLock::new();
static MIXED_TEST: OnceLock<PathBuf> = OnceLock::new();
static LINEAGE_TESTS: OnceLock<[PathBuf; 2]> = OnceLock::new();
static PREPARATION_FAILED: AtomicBool = AtomicBool::new(false);

struct PreparationGuard(bool);
impl PreparationGuard {
    fn new() -> Self {
        Self(false)
    }
    fn complete(mut self) {
        self.0 = true;
    }
}
impl Drop for PreparationGuard {
    fn drop(&mut self) {
        if !self.0 {
            PREPARATION_FAILED.store(true, Ordering::SeqCst);
        }
    }
}

extern "C" fn remove_seed() {
    if let Some(seed) = SEED.get() {
        if PREPARATION_FAILED.load(Ordering::SeqCst) {
            eprintln!(
                "retaining failed Traffic Cargo seed {}",
                seed.root.display()
            );
            return;
        }
        if let Err(error) = fs::remove_dir_all(&seed.root) {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!(
                    "cannot remove Traffic Cargo seed {}: {error}",
                    seed.root.display()
                );
                // A C exit callback cannot unwind; a cleanup failure must make
                // the test binary fail rather than silently leave artifacts.
                unsafe { libc::_exit(1) };
            }
        }
    }
}

fn output(command: &mut Command, description: &str) -> std::process::Output {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = OwnedChild::new(command.spawn().unwrap());
    let result = child
        .wait_with_output_for(Duration::from_secs(60), true)
        .unwrap_or_else(|error| panic!("{description}: {error}"));
    assert!(result.status.success(), "{description}: {result:?}");
    result
}

fn seed_fixture() -> &'static Fixture {
    SEED.get_or_init(|| {
        let fixture = Fixture::new();
        // The seed is the only process-wide fixture; each test still owns
        // and retains its own copy if it fails. Static values do not drop.
        assert_eq!(unsafe { libc::atexit(remove_seed) }, 0);
        fixture
    })
}

fn prepared() -> &'static Path {
    PREPARED.get_or_init(|| {
        let fixture = seed_fixture();
        let guard = PreparationGuard::new();
        let root = fixture.root.join("seed");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"traffic_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n").unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn fixture() {}\n").unwrap();
        fs::write(root.join("build.rs"), r#"
use std::{fs::{self, OpenOptions}, io::Write, thread, time::Duration};
fn main() {
    println!("cargo:rerun-if-env-changed=TRAFFIC_TEST_MARKER");
    println!("cargo:rerun-if-env-changed=TRAFFIC_TEST_RELEASE");
    let marker = std::env::var_os("TRAFFIC_TEST_MARKER").unwrap();
    let release = std::env::var_os("TRAFFIC_TEST_RELEASE").unwrap();
    writeln!(OpenOptions::new().create(true).append(true).open(marker).unwrap(), "started").unwrap();
    while !std::path::Path::new(&release).exists() { thread::sleep(Duration::from_millis(10)); }
    let _ = fs::metadata(release).unwrap();
}
"#).unwrap();
        let release = fixture.root.join("seed-release");
        fs::write(&release, "release").unwrap();
        let mut cargo = Command::new(env!("CARGO"));
        cargo
            .args(["build", "--offline"])
            .current_dir(&root)
            .env("CARGO_TARGET_DIR", root.join("target"))
            .env("CARGO_INCREMENTAL", "0")
            .env("TRAFFIC_TEST_MARKER", fixture.root.join("seed-started"))
            .env("TRAFFIC_TEST_RELEASE", release);
        output(&mut cargo, "offline Traffic Cargo seed");
        guard.complete();
        root
    })
}

fn copy(source: &Path, path: &Path) {
    let mut copy = Command::new("/bin/cp");
    copy.args(["-pR"]).arg(source).arg(path);
    output(&mut copy, "copy Traffic Cargo seed");
}

pub(super) fn copy_to(path: &Path) {
    copy(prepared(), path);
}

pub(super) fn copy_mixed_test_to(path: &Path) {
    let source = MIXED_TEST.get_or_init(|| {
        let root = seed_fixture().root.join("mixed-test");
        let guard = PreparationGuard::new();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"mixed_traffic_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n").unwrap();
        fs::write(root.join("src/lib.rs"), r#"
#[test]
fn real_cargo_barrier() {
    use std::{fs::{self, OpenOptions}, io::Write, thread, time::Duration};
    let marker = std::env::var_os("MIXED_BODY_LOG").unwrap();
    let release = std::env::var_os("MIXED_RELEASE").unwrap();
    writeln!(OpenOptions::new().create(true).append(true).open(marker).unwrap(), "{}", std::process::id()).unwrap();
    while !std::path::Path::new(&release).exists() { thread::sleep(Duration::from_millis(10)); }
    fs::metadata(release).unwrap();
}
"#).unwrap();
        let mut cargo = Command::new(env!("CARGO"));
        cargo
            .args(["test", "--offline", "--no-run"])
            .current_dir(&root)
            .env("CARGO_TARGET_DIR", root.join("target"));
        output(&mut cargo, "offline mixed Cargo test seed");
        guard.complete();
        root
    });
    copy(source, path);
}

pub(super) fn copy_lineage_to(path: &Path, parent: bool) {
    let sources = LINEAGE_TESTS.get_or_init(|| {
        let root = &seed_fixture().root;
        let guard = PreparationGuard::new();
        let sources = [
            ("traffic_lineage_parent", r#"
#[test] fn parent() {
    use std::{fs::{self, OpenOptions}, io::Write, process::Command, thread, time::Duration};
    writeln!(OpenOptions::new().append(true).create(true).open(std::env::var("ROOT_MARKER").unwrap()).unwrap(), "root").unwrap();
    while !std::path::Path::new(&std::env::var("START_NESTED").unwrap()).exists() { thread::sleep(Duration::from_millis(10)); }
    let frozen = std::env::var_os("FROZEN_MODE").is_some();
    let mut cmd = Command::new(std::env::var(if frozen {"FROZEN_CLIENT"} else {"TEST_ARCCTL"}).unwrap());
    if frozen { cmd.env("ARC_FIXTURE_FROZEN_TRAFFIC_V1", "1").env("FROZEN_DROP_ENQUEUE_REPLY", "1"); }
    else if std::env::var("CHILD_TOOL").unwrap() == "cargo" { cmd.args(["exec", "--", "cargo", "test", "--offline"]); }
    else { cmd.args(["exec", "--", "bun", "test", "--timeout=60000"]); }
    let status = cmd.current_dir(std::env::var("NESTED_DIR").unwrap()).env("CARGO_TARGET_DIR", std::env::var("CHILD_TARGET").unwrap()).status().unwrap();
    assert!(status.success(), "nested exit: {status}");
    fs::write(std::env::var("ROOT_SUCCESS").unwrap(), "success\n").unwrap();
}
"#),
            ("traffic_lineage_child", r#"
#[test] fn child() {
    use std::{fs::OpenOptions, io::Write, thread, time::Duration};
    writeln!(OpenOptions::new().append(true).create(true).open(std::env::var("CHILD_MARKER").unwrap()).unwrap(), "child").unwrap();
    while !std::path::Path::new(&std::env::var("CHILD_RELEASE").unwrap()).exists() { thread::sleep(Duration::from_millis(10)); }
}
"#),
        ];
        let prepared = std::thread::scope(|scope| {
            let jobs = sources.map(|(name, body)| {
                scope.spawn(move || {
                    let project = root.join(name);
                    fs::create_dir_all(project.join("src")).unwrap();
                    fs::write(project.join("Cargo.toml"), format!("[package]\nname='{name}'\nversion='0.1.0'\nedition='2021'\n")).unwrap();
                    fs::write(project.join("src/lib.rs"), body).unwrap();
                    let mut cargo = Command::new(env!("CARGO"));
                    cargo.args(["test", "--offline", "--no-run"])
                        .current_dir(&project)
                        .env("CARGO_TARGET_DIR", project.join("target"));
                    output(&mut cargo, "offline lineage Cargo seed");
                    project
                })
            });
            jobs.map(|job| job.join().unwrap())
        });
        guard.complete();
        prepared
    });
    copy(&sources[usize::from(!parent)], path);
}

#[test]
fn seed_cleanup_worker() {
    let Some(mode) = std::env::var_os("ARC_CARGO_SEED_CLEANUP_TEST") else {
        return;
    };
    let seed = seed_fixture();
    fs::write(
        std::env::var_os("ARC_CARGO_SEED_PATH").unwrap(),
        seed.root.to_str().unwrap(),
    )
    .unwrap();
    if mode == "preparation-fails" {
        // Block the real preparation at its first mkdir, before any private
        // scenario copy exists, and verify that the seed survives exit.
        fs::write(seed.root.join("seed"), "failed preparation").unwrap();
        assert!(std::panic::catch_unwind(prepared).is_err());
    } else if mode == "cleanup-fails" {
        fs::remove_dir_all(&seed.root).unwrap();
        fs::write(&seed.root, "cannot remove as a directory").unwrap();
    }
}

#[test]
fn seed_preparation_failure_is_retained_and_cleanup_failure_fails_the_binary() {
    let fixture = Fixture::new();
    for mode in ["success", "preparation-fails", "cleanup-fails"] {
        let record = fixture.root.join(mode);
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "performance_fixture::seed_cleanup_worker",
                "--nocapture",
            ])
            .env("ARC_CARGO_SEED_CLEANUP_TEST", mode)
            .env("ARC_CARGO_SEED_PATH", &record)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let result = OwnedChild::new(command.spawn().unwrap())
            .wait_with_output()
            .expect("Cargo seed cleanup worker watchdog");
        let root = PathBuf::from(fs::read_to_string(record).unwrap());
        match mode {
            "success" => {
                assert!(result.status.success(), "{result:?}");
                assert!(!root.exists());
            }
            "preparation-fails" => {
                assert!(result.status.success(), "{result:?}");
                assert_eq!(
                    fs::read_to_string(root.join("seed")).unwrap(),
                    "failed preparation"
                );
                fs::remove_dir_all(root).unwrap();
            }
            "cleanup-fails" => {
                assert!(!result.status.success(), "{result:?}");
                assert!(String::from_utf8_lossy(&result.stderr)
                    .contains("cannot remove Traffic Cargo seed"));
                assert!(root.is_file());
                fs::remove_file(root).unwrap();
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn copied_cargo_fixtures_rerun_the_real_build_script_without_recompiling_it() {
    let fixture = Fixture::new();
    let projects = super::projects(&fixture.root, "prepared");
    for project in &projects {
        let build = project.join("target/debug/build");
        let script = fs::read_dir(build)
            .unwrap()
            .map(|entry| entry.unwrap().path().join("build-script-build"))
            .find(|path| path.is_file())
            .expect("prepared build script");
        let before = fs::read(&script).unwrap();
        fs::write(project.join("release"), "release").unwrap();
        let mut cargo = Command::new(env!("CARGO"));
        cargo
            .args(["build", "--offline", "-vv"])
            .current_dir(project)
            .env("CARGO_TARGET_DIR", project.join("target"))
            .env("CARGO_INCREMENTAL", "0")
            .env("TRAFFIC_TEST_MARKER", project.join("started"))
            .env("TRAFFIC_TEST_RELEASE", project.join("release"));
        let result = output(&mut cargo, "copied Traffic Cargo fixture");
        let stderr = String::from_utf8(result.stderr).unwrap();
        assert!(
            !stderr.contains("--crate-name build_script_build"),
            "{stderr}"
        );
        assert_eq!(fs::read(&script).unwrap(), before);
        assert_eq!(
            fs::read_to_string(project.join("started")).unwrap(),
            "started\n"
        );
    }
    assert_ne!(projects[0].join("target"), projects[1].join("target"));
}

#[test]
fn copied_mixed_fixture_runs_its_real_test_without_recompiling() {
    let fixture = Fixture::new();
    let project = fixture.root.join("mixed");
    copy_mixed_test_to(&project);
    fs::write(project.join("release"), "release").unwrap();
    let mut cargo = Command::new(env!("CARGO"));
    cargo
        .args(["test", "--offline", "-vv", "--", "--nocapture"])
        .current_dir(&project)
        .env("CARGO_TARGET_DIR", project.join("target"))
        .env("MIXED_BODY_LOG", project.join("bodies"))
        .env("MIXED_RELEASE", project.join("release"));
    let result = output(&mut cargo, "copied mixed Cargo test");
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(stderr.contains("Fresh mixed_traffic_fixture"), "{stderr}");
    assert!(
        !stderr.contains("Compiling mixed_traffic_fixture"),
        "{stderr}"
    );
    assert!(String::from_utf8(result.stdout)
        .unwrap()
        .contains("1 passed"));
    let pid = fs::read_to_string(project.join("bodies")).unwrap();
    assert!(pid.trim().parse::<u32>().unwrap() > 0);
}

#[test]
fn copied_lineage_fixtures_reuse_both_test_binaries() {
    let fixture = Fixture::new();
    for (parent, name) in [
        (true, "traffic_lineage_parent"),
        (false, "traffic_lineage_child"),
    ] {
        let project = fixture.root.join(name);
        copy_lineage_to(&project, parent);
        let mut cargo = Command::new(env!("CARGO"));
        cargo
            .args(["test", "--offline", "--no-run", "-vv"])
            .current_dir(&project)
            .env("CARGO_TARGET_DIR", project.join("target"));
        let stderr =
            String::from_utf8(output(&mut cargo, "copied lineage Cargo fixture").stderr).unwrap();
        assert!(stderr.contains(&format!("Fresh {name}")), "{stderr}");
        assert!(!stderr.contains(&format!("Compiling {name}")), "{stderr}");
    }
}
