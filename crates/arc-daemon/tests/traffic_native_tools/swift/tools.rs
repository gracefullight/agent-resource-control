//! Installed tools only. Observer entries delegate unchanged argv with exec.
use super::super::{observer, root, sha, writable_cache, RunnerTools};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
};

pub struct Tools {
    pub swift: PathBuf,
    pub image: PathBuf,
    pub cargo: PathBuf,
    pub sdk: PathBuf,
    pub shims: PathBuf,
    pub directory: PathBuf,
    pub observers: Value,
    leases: super::pool::Leases,
    _cleanup: super::cleanup::Projects,
    version: String,
    cargo_version: String,
    image_sha256: String,
}

struct Installed {
    swift: PathBuf,
    image: PathBuf,
    cargo: PathBuf,
    sdk: PathBuf,
    version: String,
    cargo_version: String,
    image_sha256: String,
}

fn output(program: &Path, args: &[&str]) -> String {
    let mut command = Command::new(program);
    command.args(args);
    let result = super::super::go::owned::fixture_output(&mut command)
        .unwrap_or_else(|e| panic!("required installed offline tool {}: {e}", program.display()));
    assert!(result.status.success(), "{}: {result:?}", program.display());
    String::from_utf8(result.stdout).unwrap().trim().to_owned()
}

impl Tools {
    pub fn installed() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        static INSTALLED: OnceLock<Installed> = OnceLock::new();
        let installed = INSTALLED.get_or_init(Self::probe);
        let directory = root().join(format!(
            "target/traffic-native-tools/swift/{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        writable_cache(&directory);
        let shims = directory.join("observers");
        let observers = json!([
            observer(&shims, "swift", &installed.swift),
            observer(&shims, "cargo", &installed.cargo)
        ]);
        Self {
            swift: installed.swift.clone(),
            image: installed.image.clone(),
            cargo: installed.cargo.clone(),
            sdk: installed.sdk.clone(),
            shims,
            leases: super::pool::Leases::default(),
            _cleanup: super::cleanup::Projects::new(directory.clone()),
            directory,
            observers,
            version: installed.version.clone(),
            cargo_version: installed.cargo_version.clone(),
            image_sha256: installed.image_sha256.clone(),
        }
    }

    fn probe() -> Installed {
        let xcrun = Path::new("/usr/bin/xcrun");
        // Keep the invocation's swift symlink. SwiftPM execs swift-package;
        // identity witnesses compare that running image, not swift-frontend.
        let swift = PathBuf::from(output(xcrun, &["--find", "swift"]));
        let image = PathBuf::from(output(xcrun, &["--find", "swift-build"]))
            .canonicalize()
            .unwrap();
        assert_eq!(
            PathBuf::from(output(xcrun, &["--find", "swift-test"]))
                .canonicalize()
                .unwrap(),
            image
        );
        let sdk = PathBuf::from(output(xcrun, &["--sdk", "macosx", "--show-sdk-path"]));
        assert!(sdk.join("SDKSettings.json").is_file());
        let version = output(&swift, &["--version"]);
        assert!(version.contains("Swift version") && version.contains("apple-macos"));
        let cargo = PathBuf::from(env!("CARGO"));
        let cargo_version = output(&cargo, &["--version"]);
        assert!(cargo_version.contains("1.95.0"));
        let image_sha256 = sha(&image);
        Installed {
            swift,
            image,
            cargo,
            sdk,
            version,
            cargo_version,
            image_sha256,
        }
    }

    pub fn receipt(&self) -> Value {
        json!({"family":"swift","swift":self.swift,"swiftImage":self.image,
            "swiftSha256":self.image_sha256,"version":self.version,"sdk":self.sdk,
            "cargo":self.cargo,"cargoVersion":self.cargo_version,"offline":true,
            "remoteDependencies":0,"privateCaches":true,"argvRewritten":false})
    }

    pub(super) fn lease(&self, form: super::Form, evidence: &Path) -> PathBuf {
        self.leases.acquire(self, form, evidence)
    }

    #[cfg(test)]
    pub(super) fn reacquire(&self, form: super::Form, evidence: &Path, project: &Path) -> PathBuf {
        self.leases.reacquire(self, form, evidence, project)
    }

    pub(super) fn release_baseline(&self, path: &Path) {
        self.leases.release_baseline(path);
    }

    pub(super) fn release_success(&self, path: &Path) {
        self.leases.release_success(path);
    }
}
impl RunnerTools for Tools {
    fn cpu_budget_millis(&self) -> u64 {
        4000
    }
    fn shims(&self) -> &Path {
        &self.shims
    }
}
