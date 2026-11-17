//! Fixture-only tool/cache isolation, identical for direct and managed commands.
use super::super::*;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
};

pub const FORMS: [Form; 3] = [Form::GoBuild, Form::GoTest, Form::GoVet];
const GATE: &str = "package nativefixture\n// ";
// Run::release supplies this line. It is a comment in the Go fixture, and the
// replacement regular source has exactly the same bytes as the released FIFO.
const RELEASE: &str = "export const gate: number = 42;\n";
static BINARY_CACHE: OnceLock<PathBuf> = OnceLock::new();
static SEED_READY: Mutex<bool> = Mutex::new(false);
static INSTALLED: OnceLock<Installed> = OnceLock::new();
#[cfg(test)]
static SEED_PREPARATIONS: AtomicU64 = AtomicU64::new(0);

struct Installed {
    go: PathBuf,
    cargo: PathBuf,
    version: String,
    go_sha256: String,
    cargo_clippy_sha256: String,
}

extern "C" fn remove_binary_cache() {
    if let Some(path) = BINARY_CACHE.get() {
        if let Err(error) = fs::remove_dir_all(path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!(
                    "could not remove owned Go test cache {}: {error}",
                    path.display()
                );
                // A successful test process must not hide a retained build tree.
                unsafe { libc::_exit(1) };
            }
        }
    }
}

pub struct Tools {
    pub go: PathBuf,
    pub cargo: PathBuf,
    pub shims: PathBuf,
    pub cache: PathBuf,
    pub version: String,
    directory: PathBuf,
    go_sha256: String,
    cargo_clippy_sha256: String,
}
impl RunnerTools for Tools {
    fn cpu_budget_millis(&self) -> u64 {
        4000
    }
    fn shims(&self) -> &Path {
        &self.shims
    }
}
fn output(command: &mut Command) -> String {
    let result =
        super::owned::GoChild::spawn(command.stdout(Stdio::piped()).stderr(Stdio::piped()))
            .unwrap()
            .wait_with_output()
            .unwrap();
    assert!(
        result.status.success(),
        "required installed native tool: {command:?}: {result:?}"
    );
    String::from_utf8(result.stdout).unwrap().trim().to_owned()
}
impl Tools {
    pub fn installed() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = BINARY_CACHE.get_or_init(|| {
            let path = root().join(format!(
                "target/traffic-native-tools/go/runtime-{}-{}",
                std::process::id(),
                arc_core::process_identity::random_job_id().unwrap()
            ));
            fs::create_dir_all(&path).unwrap();
            assert_eq!(unsafe { libc::atexit(remove_binary_cache) }, 0);
            path
        });
        let directory = root().join(format!(
            "target/traffic-native-tools/go/case-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            arc_core::process_identity::random_job_id().unwrap()
        ));
        let cache = directory.join("cache");
        fs::create_dir_all(&cache).unwrap();
        // Prepare the standard-library build once before independent cases
        // start. Each case receives a private writable clone of that seed.
        let mut ready = SEED_READY.lock().unwrap();
        if *ready {
            snapshot_build(&base.join("build-seed"), &cache.join("build"));
        }
        for name in ["build", "modules", "path", "tmp"] {
            writable_cache(&cache.join(name));
        }
        let installed = INSTALLED.get_or_init(|| Self::probe(&cache));
        if !*ready {
            prepare_seed(base, &cache, &installed.go);
            *ready = true;
        }
        drop(ready);
        let shims = directory.join("observers");
        Self {
            go: installed.go.clone(),
            cargo: installed.cargo.clone(),
            shims,
            cache,
            version: installed.version.clone(),
            directory,
            go_sha256: installed.go_sha256.clone(),
            cargo_clippy_sha256: installed.cargo_clippy_sha256.clone(),
        }
    }

    fn probe(cache: &Path) -> Installed {
        let mut probe = Command::new("go");
        isolate(cache, &mut probe);
        let goroot = output(probe.args(["env", "GOROOT"]));
        let go = PathBuf::from(goroot).join("bin/go").canonicalize().unwrap();
        assert!(go.is_absolute() && go.is_file(), "required real Go binary");
        let mut probe = Command::new(&go);
        isolate(cache, &mut probe);
        let version = output(probe.arg("version"));
        assert!(
            version.starts_with("go version go") && version.contains("darwin/"),
            "{version}"
        );
        let cargo = PathBuf::from(env!("CARGO")).canonicalize().unwrap();
        let go_sha256 = sha(&go);
        let cargo_clippy_sha256 = sha(&cargo.with_file_name("cargo-clippy"));
        Installed {
            go,
            cargo,
            version,
            go_sha256,
            cargo_clippy_sha256,
        }
    }
    pub fn observers(&self) -> serde_json::Value {
        serde_json::json!([
            observer(&self.shims, "go", &self.go),
            observer(&self.shims, "cargo", &self.cargo),
        ])
    }
    pub fn receipt(&self) -> serde_json::Value {
        serde_json::json!({"go":self.go,"goSha256":self.go_sha256,"version":self.version,
            "cargo":self.cargo,"cargoClippy":self.cargo.with_file_name("cargo-clippy"),
            "cargoClippySha256":self.cargo_clippy_sha256,
            "privateCache":self.cache,"dependencies":"Go standard library only",
            "GOPROXY":"off","GOSUMDB":"off","GOTOOLCHAIN":"local","GOENV":"off",
            "GOWORK":"off","CGO_ENABLED":"0","offlinePrepared":true})
    }
}

fn prepare_seed(base: &Path, cache: &Path, go: &Path) {
    let project = base.join("seed-project");
    fs::create_dir_all(&project).unwrap();
    write(
        &project.join("go.mod"),
        "module example.org/arc-fixture-seed\n\ngo 1.20\n",
    );
    write(
        &project.join("seed_test.go"),
        r#"package seed
import (_ "encoding/json"; _ "fmt"; _ "io"; _ "os"; _ "os/exec"; _ "path/filepath"; _ "strconv"; _ "syscall"; "testing")
func TestSeed(t *testing.T) {}
"#,
    );
    let mut command = Command::new(go);
    command
        .args(["test", "-p=2", "-count=1", "."])
        .current_dir(&project);
    isolate(cache, &mut command);
    output(&mut command);
    snapshot_build(&cache.join("build"), &base.join("build-seed"));
    fs::remove_dir_all(project).unwrap();
    #[cfg(test)]
    SEED_PREPARATIONS.fetch_add(1, Ordering::SeqCst);
}

fn snapshot_build(source: &Path, destination: &Path) {
    assert!(!destination.exists(), "case build cache must be new");
    // Darwin cp clones files on APFS and falls back to a regular copy on
    // filesystems without clone support. Neither path shares writable inodes.
    let mut command = Command::new("/bin/cp");
    command.arg("-cR").arg(source).arg(destination);
    let result = super::owned::fixture_output(&mut command).unwrap();
    assert!(
        result.status.success(),
        "cannot copy Go build seed: {result:?}"
    );
}

impl Drop for Tools {
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }
        fs::remove_dir_all(&self.directory).unwrap();
    }
}

fn isolate(cache: &Path, command: &mut Command) {
    command
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
        .env("GO111MODULE", "on")
        .env("GOVCS", "*:off")
        .env("GOTELEMETRY", "off")
        .env("GOMAXPROCS", "2")
        .env("CGO_ENABLED", "0")
        .env("GOOS", "darwin")
        .env(
            "GOARCH",
            if cfg!(target_arch = "aarch64") {
                "arm64"
            } else {
                "amd64"
            },
        )
        .env_remove("GOROOT")
        .env_remove("GOCACHEPROG")
        .env_remove("GOEXPERIMENT");
    // Deliberately inherit LLVM_PROFILE_FILE/RUSTFLAGS and all ARC coverage state.
}
pub fn configure(project: &Path, command: &mut Command) {
    let cache = fs::read_to_string(project.join("fixture-cache-path")).unwrap();
    isolate(Path::new(&cache), command);
    command
        .env("TBS_PROJECT", project)
        .env("TBS_VALUE", "literal $HOME ' value 日本語")
        .env_remove("TBS_PREDECESSOR")
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .env_remove("ARC_PRESSURE_ACCEPTANCE_STAGE")
        .env_remove("ARC_PRESSURE_ACCEPTANCE_BARRIER");
}

pub fn project(
    base: &Path,
    tools: &Tools,
    name: &str,
    form: Form,
    hold: bool,
    fail: bool,
) -> PathBuf {
    assert!(FORMS.contains(&form));
    let path = base.join(name);
    fs::create_dir_all(&path).unwrap();
    write(
        &path.join("fixture-cache-path"),
        tools.cache.to_str().unwrap(),
    );
    write(
        &path.join("go.mod"),
        "module example.org/nativefixture\n\ngo 1.20\n",
    );
    write(&path.join("library.go"), "package nativefixture\nimport \"fmt\"\nfunc Value() string { return fmt.Sprint(\"native\") }\n");
    if form == Form::GoTest {
        write(
            &path.join("native_test.go"),
            include_str!("sources/native_test.go.txt"),
        );
        if fail {
            write(&path.join("fail-test"), "intended assertion failure");
        }
    } else if fail {
        write(
            &path.join("library.go"),
            if form == Form::GoBuild {
                "package nativefixture\nvar Invalid string = 7\n"
            } else {
                "package nativefixture\nimport \"fmt\"\nfunc Invalid() { fmt.Printf(\"%d\", \"intended vet failure\") }\n"
            },
        );
    }
    if hold {
        let fifo = std::ffi::CString::new(path.join("gate.go").as_os_str().as_bytes()).unwrap();
        // SAFETY: new fixture-owned pathname; mode permits only the test owner.
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    }
    path
}
pub fn argv(form: Form) -> Vec<String> {
    let mut argv: Vec<_> = form.argv().iter().map(|s| (*s).to_owned()).collect();
    argv.extend(["-p=2".into(), "-mod=readonly".into()]);
    match form {
        Form::GoBuild => argv.extend(["-o".into(), "native output.a".into()]),
        Form::GoTest => argv.extend([
            "-count=1".into(),
            "-parallel=2".into(),
            "-run=TestNative".into(),
            "-timeout=30s".into(),
        ]),
        Form::GoVet => argv.push("-printf".into()),
        _ => unreachable!(),
    }
    argv.push(".".into());
    argv
}
pub fn prepared(project: PathBuf, tools: &Tools, form: Form) -> Prepared {
    Prepared {
        form,
        project,
        executable: tools.go.clone(),
        argv: argv(form),
        ready,
        configure,
    }
}

/// ENXIO means no reader has reached the native source open. A successful
/// O_NONBLOCK writer open witnesses that reader, rather than elapsed time.
pub fn gate(project: &Path, writer: &mut Option<File>) -> bool {
    if writer.is_some() {
        return true;
    }
    let path = project.join("gate.go");
    let mut file = match OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&path)
    {
        Ok(file) => file,
        Err(e) if e.raw_os_error() == Some(libc::ENXIO) => return false,
        Err(e) => panic!("real Go source barrier: {e}"),
    };
    file.write_all(GATE.as_bytes()).unwrap();
    fs::remove_file(&path).unwrap();
    write(&path, format!("{GATE}{RELEASE}"));
    write(&project.join("bodies"), "native Go opened source FIFO\n");
    *writer = Some(file);
    true
}
fn ready(run: &mut Run) -> bool {
    gate(&run.project, &mut run.fifo)
}
pub fn release(writer: &mut Option<File>) {
    writer
        .take()
        .unwrap()
        .write_all(RELEASE.as_bytes())
        .unwrap();
}

pub fn direct(project: &Path, executable: &Path, argv: &[String], input: &[u8]) -> Output {
    let mut command = Command::new(executable);
    command
        .args(&argv[1..])
        .current_dir(project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure(project, &mut command);
    let mut child = super::owned::GoChild::spawn(&mut command).unwrap();
    child.child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[test]
    fn go_cache_seed_is_independent_of_case_writes() {
        let root = std::env::temp_dir().join(format!(
            "arc-go-cache-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        let seed = root.join("seed");
        let first = root.join("first");
        let second = root.join("second");
        fs::create_dir_all(&seed).unwrap();
        fs::write(seed.join("entry"), b"seed").unwrap();
        snapshot_build(&seed, &first);
        snapshot_build(&seed, &second);
        fs::write(first.join("entry"), b"changed").unwrap();
        fs::write(first.join("case-only"), b"private").unwrap();
        assert_eq!(fs::read(seed.join("entry")).unwrap(), b"seed");
        assert_eq!(fs::read(second.join("entry")).unwrap(), b"seed");
        assert!(!second.join("case-only").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn go_native_seed_is_prepared_once_and_case_build_caches_are_isolated() {
        let first = Tools::installed();
        let second = Tools::installed();
        assert_eq!(SEED_PREPARATIONS.load(Ordering::SeqCst), 1);
        assert_ne!(first.cache, second.cache);
        let marker = first.cache.join("build/case-only");
        fs::write(&marker, b"private").unwrap();
        assert!(!second.cache.join("build/case-only").exists());
        assert!(!BINARY_CACHE
            .get()
            .unwrap()
            .join("build-seed/case-only")
            .exists());
        let first_root = first.directory.clone();
        let second_root = second.directory.clone();
        drop(first);
        drop(second);
        assert!(!first_root.exists());
        assert!(!second_root.exists());
    }
}
