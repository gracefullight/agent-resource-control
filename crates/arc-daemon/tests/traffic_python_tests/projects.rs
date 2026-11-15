//! All sources are created offline; exec observers always delegate to real tools.
use super::*;
use std::{
    os::unix::fs::PermissionsExt,
    sync::atomic::{AtomicU64, Ordering},
};
pub struct Tools {
    pub python: python::PytestRuntime,
    pub cargo: PathBuf,
    pub bun: PathBuf,
    pub shims: PathBuf,
}
impl shared::RunnerTools for Tools {
    fn shims(&self) -> &Path {
        &self.shims
    }
}
fn output(program: &Path, args: &[&str]) -> String {
    let result = Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("required offline tool {}: {e}", program.display()));
    assert!(
        result.status.success(),
        "required tool {}: {result:?}",
        program.display()
    );
    String::from_utf8(result.stdout).unwrap().trim().to_owned()
}
fn quote(path: &Path) -> String {
    format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"))
}
impl Tools {
    pub fn installed() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let python = python::PytestRuntime::installed();
        let bun = PathBuf::from(output(Path::new("bun"), &["-p", "process.execPath"]));
        let cargo = PathBuf::from(env!("CARGO"));
        assert_eq!(output(&bun, &["--version"]), "1.4.4");
        assert!(output(&cargo, &["--version"]).contains("1.95.0"));
        let shims = shared::root().join(format!(
            "target/traffic-python-tests/observers/{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&shims).unwrap();
        let tools = Self {
            python,
            bun,
            cargo,
            shims,
        };
        for (name, real) in tools.programs() {
            // This observer does not parse, replace or add native arguments.
            // Shell exec retains PID/process group and all inherited ARC coverage.
            let path = tools.shims.join(name);
            write(
                &path,
                format!(
                    r##"#!/bin/sh
printf '%s %s %s\n' {name} "$$" "$PPID" >> "$TBS_PROJECT/launches" || exit 125
if [ -n "${{TBS_PREDECESSOR:-}}" ]; then
  case "$TBS_PREDECESSOR" in *[!0123456789abcdef]*|'') exit 125;; esac
  [ "${{#TBS_PREDECESSOR}}" -eq 32 ] || exit 125
  state=$(/usr/bin/sqlite3 -readonly -cmd '.timeout 2000' -cmd ".parameter set :id \"'$TBS_PREDECESSOR'\"" "$ARC_DB_PATH" 'SELECT state FROM permits WHERE job_id=:id;') || exit 125
  [ "$state" = RELEASED ] || exit 125
  printf '%s\n' "$state" > "$TBS_PROJECT/release-witness" || exit 125
fi
exec {} "$@"
"##,
                    quote(real)
                ),
            );
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        tools
    }
    fn programs(&self) -> [(&'static str, &Path); 5] {
        [
            ("python", &self.python.python),
            ("python3", &self.python.python),
            ("pytest", &self.python.console),
            ("cargo", &self.cargo),
            ("bun", &self.bun),
        ]
    }
    pub fn receipt(&self) -> Value {
        json!({"python":self.python.distribution,"console":self.python.console,"programs":self.programs().map(|(name,p)|json!({"name":name,"path":p,"sha256":sha(p)})),"offline":true,"venv":"--without-pip --system-site-packages"})
    }
    pub fn observers(&self) -> Value {
        json!(self
            .programs()
            .map(|(name, _)| json!({"name":name,"sha256":sha(&self.shims.join(name))})))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Pytest,
    Unittest,
    Cargo,
    Bun,
    Build,
}
impl Kind {
    pub fn class(self) -> &'static str {
        if self == Self::Build {
            "BUILD"
        } else {
            "TEST"
        }
    }
    pub fn reason(self) -> &'static str {
        match self {
            Self::Pytest => "PYTHON_PYTEST",
            Self::Unittest => "PYTHON_UNITTEST",
            Self::Cargo => "CARGO_TEST",
            Self::Bun => "BUN_TEST",
            Self::Build => "CARGO_BUILD",
        }
    }
    pub fn argv(self) -> Vec<&'static str> {
        match self {
            Self::Pytest => vec!["python", "-m", "pytest", "-s", "--tb=no", "--color=no"],
            Self::Unittest => vec!["python", "-m", "unittest"],
            Self::Cargo => vec!["cargo", "test", "--offline", "--", "--nocapture"],
            Self::Bun => vec!["bun", "test", "evidence.test.ts", "--timeout", "120000"],
            Self::Build => vec!["cargo", "build", "--offline"],
        }
    }
    pub fn executable(self, case: &Case) -> &Path {
        match self {
            Self::Pytest | Self::Unittest => &case.tools.python.image,
            Self::Cargo | Self::Build => &case.tools.cargo,
            Self::Bun => &case.tools.bun,
        }
    }
}
pub fn configure(case: &Case, path: &Path, command: &mut Command) {
    python::isolate(command);
    command
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .env_remove("ARC_PRESSURE_ACCEPTANCE_STAGE")
        .env_remove("ARC_PRESSURE_ACCEPTANCE_BARRIER")
        .env("TBS_PROJECT", path)
        .env("TBS_BODY", path.join("bodies"))
        .env("TBS_RELEASE", path.join("release"))
        .env("TBS_ARCCTL", binary("arcctl"))
        .env("TBS_VALUE", "literal $HOME ' value 日本語")
        .env("PYTHONPATH", path.join("imports"))
        .env(
            "PATH",
            format!(
                "{}:{}",
                case.tools.shims.display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", path.join("target"))
        // Fixture output is stable while the interpreter/runner remain native.
        .env(
            "PYTEST_ADDOPTS",
            "-qq --capture=no --tb=no --color=no --no-header --no-summary -p no:cacheprovider",
        );
}
pub fn project(case: &Case, name: &str, kind: Kind, hold: bool) -> PathBuf {
    let path = case.artifacts.join(name);
    fs::create_dir_all(path.join("imports")).unwrap();
    write(
        &path.join("imports/fixture_dependency.py"),
        "VALUE = 'loaded from caller PYTHONPATH'\n",
    );
    match kind {
        Kind::Pytest | Kind::Unittest => {
            write(&path.join("pytest.ini"), "[pytest]\ntestpaths = .\n");
            write(
                &path.join("test_fixture.py"),
                include_str!("sources/test_fixture.py"),
            );
        }
        Kind::Cargo | Kind::Build => {
            fs::create_dir(path.join("src")).unwrap();
            write(&path.join("Cargo.toml"), "[package]\nname=\"python_test_peer\"\nversion=\"0.1.0\"\nedition=\"2021\"\n[workspace]\n");
            write(&path.join("src/lib.rs"), include_str!("sources/lib.rs.txt"));
            if kind == Kind::Build {
                write(&path.join("build.rs"), include_str!("sources/build.rs.txt"));
            }
        }
        Kind::Bun => write(
            &path.join("evidence.test.ts"),
            include_str!("sources/evidence.test.ts"),
        ),
    }
    if !hold {
        write(&path.join("release"), "release");
    }
    path
}
