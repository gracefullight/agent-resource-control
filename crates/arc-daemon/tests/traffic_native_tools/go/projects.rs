//! Fixture-only tool/cache isolation, identical for direct and managed commands.
use super::super::*;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

pub const FORMS: [Form; 3] = [Form::GoBuild, Form::GoTest, Form::GoVet];
const GATE: &str = "package nativefixture\n// ";
// Run::release supplies this line. It is a comment in the Go fixture, and the
// replacement regular source has exactly the same bytes as the released FIFO.
const RELEASE: &str = "export const gate: number = 42;\n";

pub struct Tools {
    pub go: PathBuf,
    pub cargo: PathBuf,
    pub shims: PathBuf,
    pub cache: PathBuf,
    pub version: String,
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
        let base = root().join(format!(
            "target/traffic-native-tools/go/runtime-{}",
            std::process::id()
        ));
        let cache = base.join("cache");
        for name in ["build", "modules", "path", "tmp"] {
            writable_cache(&cache.join(name));
        }
        let mut probe = Command::new("go");
        isolate(&cache, &mut probe);
        let goroot = output(probe.args(["env", "GOROOT"]));
        let go = PathBuf::from(goroot).join("bin/go").canonicalize().unwrap();
        assert!(go.is_absolute() && go.is_file(), "required real Go binary");
        let mut probe = Command::new(&go);
        isolate(&cache, &mut probe);
        let version = output(probe.arg("version"));
        assert!(
            version.starts_with("go version go") && version.contains("darwin/"),
            "{version}"
        );
        let cargo = PathBuf::from(env!("CARGO")).canonicalize().unwrap();
        let shims = base.join(format!(
            "observers-{}",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Self {
            go,
            cargo,
            shims,
            cache,
            version,
        }
    }
    pub fn observers(&self) -> serde_json::Value {
        serde_json::json!([
            observer(&self.shims, "go", &self.go),
            observer(&self.shims, "cargo", &self.cargo),
        ])
    }
    pub fn receipt(&self) -> serde_json::Value {
        serde_json::json!({"go":self.go,"goSha256":sha(&self.go),"version":self.version,
            "cargo":self.cargo,"cargoClippy":self.cargo.with_file_name("cargo-clippy"),
            "cargoClippySha256":sha(&self.cargo.with_file_name("cargo-clippy")),
            "privateCache":self.cache,"dependencies":"Go standard library only",
            "GOPROXY":"off","GOSUMDB":"off","GOTOOLCHAIN":"local","GOENV":"off",
            "GOWORK":"off","CGO_ENABLED":"0","offlinePrepared":true})
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
