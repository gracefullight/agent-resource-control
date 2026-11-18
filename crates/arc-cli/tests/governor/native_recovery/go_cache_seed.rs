//! Private, offline Go build cache shared only as a copy source within this binary.
use super::{super::native, OwnedTree, GO_TEST};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
    time::Duration,
};

struct Seed {
    fixture: native::Fixture,
    cache: PathBuf,
}

static SEED: OnceLock<Seed> = OnceLock::new();

extern "C" fn remove_seed() {
    if let Some(seed) = SEED.get() {
        remove_or_fail(&seed.fixture.root);
    }
}

fn remove_or_fail(root: &Path) {
    if let Err(error) = fs::remove_dir_all(root) {
        if error.kind() != std::io::ErrorKind::NotFound {
            eprintln!("cannot remove Governor Go seed {}: {error}", root.display());
            // A C exit callback cannot unwind. Propagate failed cleanup as a
            // failed binary instead of silently leaving the fixture behind.
            unsafe { libc::_exit(1) };
        }
    }
}

fn seed() -> &'static Seed {
    SEED.get_or_init(|| prepare_seed(native::Fixture::with_binary(PathBuf::new())))
}

fn prepare_seed(fixture: native::Fixture) -> Seed {
    let project = fixture.root.join("project");
    let cache = fixture.root.join("cache");
    fs::create_dir(&project).unwrap();
    for name in ["build", "modules", "path", "tmp"] {
        fs::create_dir_all(cache.join(name)).unwrap();
    }
    fs::write(
        project.join("go.mod"),
        "module example.org/lineage\n\ngo 1.20\n",
    )
    .unwrap();
    fs::write(project.join("lineage_test.go"), GO_TEST).unwrap();
    let mut command = Command::new("go");
    command
        .args(["test", "-c", "-p", "2", "-o"])
        .arg(fixture.root.join("lineage.test"))
        .current_dir(project)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env(
            super::super::owned_tree::REGISTRY_ENV,
            fixture.root.join("seed-owners"),
        )
        .env("GOCACHE", cache.join("build"))
        .env("GOMODCACHE", cache.join("modules"))
        .env("GOPATH", cache.join("path"))
        .env("GOTMPDIR", cache.join("tmp"))
        .env("GOPROXY", "off")
        .env("GOSUMDB", "off")
        .env("GOTOOLCHAIN", "local")
        .env("GOENV", "off")
        .env("GOWORK", "off")
        .env("GOFLAGS", "")
        .env("GOVCS", "*:off")
        .env("GOTELEMETRY", "off")
        .env("GOMAXPROCS", "2")
        .env("CGO_ENABLED", "0")
        .env_remove("GOROOT")
        .env_remove("GOCACHEPROG")
        .env_remove("GOEXPERIMENT");
    let output = OwnedTree::spawn(command, &fixture.root)
        .wait_with_output_for(Duration::from_secs(180))
        .expect("offline Governor Go preparation watchdog");
    assert!(
        output.status.success(),
        "offline Governor Go preparation: {output:?}"
    );
    assert_eq!(unsafe { libc::atexit(remove_seed) }, 0);
    Seed { fixture, cache }
}

pub(super) fn prepare() {
    let _ = seed();
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            assert!(
                kind.is_file(),
                "unexpected Go cache entry: {:?}",
                entry.path()
            );
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

pub(super) fn copy_to(project: &Path) {
    copy_tree(&seed().cache.join("build"), &project.join("cache/build"));
}

static TEST_CLEANUP_ROOT: OnceLock<PathBuf> = OnceLock::new();
extern "C" fn test_remove_seed() {
    remove_or_fail(TEST_CLEANUP_ROOT.get().unwrap());
}

#[test]
fn seed_cleanup_worker() {
    let Some(mode) = std::env::var_os("ARC_GO_SEED_CLEANUP_TEST") else {
        return;
    };
    let path = PathBuf::from(std::env::var_os("ARC_GO_SEED_PATH").unwrap());
    if mode == "preparation-fails" {
        let fixture = native::Fixture::with_binary(PathBuf::new());
        fs::write(&path, fixture.root.to_str().unwrap()).unwrap();
        assert!(std::panic::catch_unwind(|| prepare_seed(fixture)).is_err());
    } else {
        TEST_CLEANUP_ROOT.set(path).unwrap();
        assert_eq!(unsafe { libc::atexit(test_remove_seed) }, 0);
    }
}

#[test]
fn seed_preparation_failure_is_retained_and_cleanup_failure_fails_the_binary() {
    let fixture = native::Fixture::with_binary(PathBuf::new());
    let empty_path = fixture.root.join("empty-path");
    fs::create_dir(&empty_path).unwrap();
    for mode in ["success", "preparation-fails", "cleanup-fails"] {
        let path = fixture.root.join(mode);
        match mode {
            "success" => fs::create_dir(&path).unwrap(),
            "cleanup-fails" => fs::write(&path, "not a directory").unwrap(),
            _ => (),
        }
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "native_recovery::native_environment::go_cache_seed::seed_cleanup_worker",
                "--nocapture",
            ])
            .env("ARC_GO_SEED_CLEANUP_TEST", mode)
            .env("ARC_GO_SEED_PATH", &path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if mode == "preparation-fails" {
            command.env("PATH", &empty_path);
        }
        let result = native::OwnedChild::new(command.spawn().unwrap())
            .wait_with_output()
            .expect("Go seed cleanup worker watchdog");
        match mode {
            "success" => {
                assert!(result.status.success(), "{result:?}");
                assert!(!path.exists());
            }
            "preparation-fails" => {
                assert!(result.status.success(), "{result:?}");
                let root = PathBuf::from(fs::read_to_string(path).unwrap());
                assert!(root.join("project/go.mod").is_file());
                fs::remove_dir_all(root).unwrap();
            }
            "cleanup-fails" => {
                assert!(!result.status.success(), "{result:?}");
                assert!(String::from_utf8_lossy(&result.stderr)
                    .contains("cannot remove Governor Go seed"));
                assert!(path.is_file());
                fs::remove_file(path).unwrap();
            }
            _ => unreachable!(),
        }
    }
}
