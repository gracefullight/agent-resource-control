//! Matching real installed Python/pytest readiness and offline venv setup.
use std::{path::PathBuf, process::Command};

/// Test-only readiness, never used by production classification. The console
/// script and module must select the same installed distribution/interpreter.
pub struct PytestRuntime {
    pub python: PathBuf,
    pub image: PathBuf,
    pub console: PathBuf,
    pub distribution: serde_json::Value,
}
impl PytestRuntime {
    pub fn installed() -> Self {
        let console = std::env::var_os("ARC_TEST_PYTEST")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::split_paths(&std::env::var_os("PATH")?)
                    .map(|p| p.join("pytest"))
                    .find(|p| p.is_file())
            })
            .expect("required real installed pytest console entry point on PATH (or ARC_TEST_PYTEST); no downloads or skips");
        let script = std::fs::read_to_string(&console)
            .expect("required readable installed pytest console script");
        assert!(
            script.contains("from pytest import console_main"),
            "pytest must be its installed console entry point: {console:?}"
        );
        let shebang = script
            .lines()
            .next()
            .unwrap()
            .strip_prefix("#!")
            .expect("pytest console script requires an interpreter shebang")
            .trim();
        let python = std::env::var_os("ARC_TEST_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(shebang));
        assert!(python.is_absolute() && python.is_file(), "pytest interpreter missing: {python:?}; use a matching ARC_TEST_PYTHON/ARC_TEST_PYTEST pair");
        let mut probe = Command::new(&python);
        isolate(&mut probe);
        let result = probe.args(["-c", "import ctypes,importlib.metadata as m,json,os,sys,pytest; d=m.distribution('pytest'); b=ctypes.create_string_buffer(4096); n=ctypes.CDLL(None).proc_pidpath(os.getpid(),b,len(b)) if sys.platform=='darwin' else 0; print(json.dumps(dict(python=sys.executable,image=os.fsdecode(b.value) if n>0 else sys.executable,version=pytest.__version__,module=pytest.__file__,entry=[e.value for e in d.entry_points if e.group=='console_scripts' and e.name=='pytest'])))"])
            .output().expect("required installed matching Python/pytest interpreter");
        assert!(
            result.status.success(),
            "required interpreter must import real pytest without installing it: {result:?}"
        );
        let distribution: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(
            distribution["entry"],
            serde_json::json!(["pytest:console_main"])
        );
        let mut console_probe = Command::new(&console);
        isolate(&mut console_probe);
        let version = console_probe.arg("--version").output().unwrap();
        assert!(
            version.status.success(),
            "real pytest entry point failed: {version:?}"
        );
        assert_eq!(
            String::from_utf8(version.stdout).unwrap().trim(),
            format!("pytest {}", distribution["version"].as_str().unwrap())
        );
        assert_eq!(
            std::fs::canonicalize(&python).unwrap(),
            std::fs::canonicalize(shebang).unwrap(),
            "console and module must select the same interpreter"
        );
        let image = PathBuf::from(distribution["image"].as_str().unwrap());
        assert!(image.is_absolute() && image.is_file());
        Self {
            python,
            image,
            console,
            distribution,
        }
    }
    /// Offline venv creation uses the verified installed pytest distribution;
    /// ensurepip and package installation are deliberately unnecessary.
    pub fn venv(&self, directory: &std::path::Path) -> PathBuf {
        let mut command = Command::new(&self.python);
        isolate(&mut command);
        let result = command
            .args(["-m", "venv", "--without-pip", "--system-site-packages"])
            .arg(directory)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "offline venv prerequisite failed: {result:?}"
        );
        let python = directory.join("bin/python");
        let mut command = Command::new(&python);
        isolate(&mut command);
        let result = command.args(["-c", "import json,sys,pytest; print(json.dumps([sys.prefix != sys.base_prefix, pytest.__version__]))"]).output().unwrap();
        assert!(
            result.status.success(),
            "venv cannot import installed pytest: {result:?}"
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&result.stdout).unwrap(),
            serde_json::json!([true, self.distribution["version"]])
        );
        python
    }
}

/// Fixture-only influence isolation, applied equally to native and ARC runs.
/// Do not clear LLVM_PROFILE_FILE/RUSTFLAGS or change production caller env.
pub fn isolate(command: &mut Command) {
    command
        .env("PYTEST_DISABLE_PLUGIN_AUTOLOAD", "1")
        .env("PYTEST_ADDOPTS", "")
        .env("PYTEST_PLUGINS", "")
        .env("PYTHONNOUSERSITE", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("PYTHONHASHSEED", "0")
        .env_remove("PYTEST_DEBUG")
        .env_remove("PYTHONINSPECT")
        .env_remove("PYTHONSAFEPATH")
        .env_remove("PYTHONWARNINGS")
        .env_remove("PYTHONHOME")
        .env_remove("PYTHONPATH");
}
