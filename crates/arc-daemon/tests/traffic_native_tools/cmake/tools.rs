//! Resolve and exercise installed prerequisites only; no provisioning or downloads.
use super::*;
use serde_json::json;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub struct Tools {
    pub cmake: PathBuf,
    pub ctest: PathBuf,
    pub compiler: PathBuf,
    pub make: PathBuf,
    pub cargo: PathBuf,
    pub scratch: PathBuf,
    pub shims: PathBuf,
    pub observers: Value,
    versions: Value,
}

fn resolve(name: &str) -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").expect("native fixture PATH"))
        .map(|path| path.join(name))
        .find(|path| path.is_file())
        .unwrap_or_else(|| panic!("required offline CMake fixture prerequisite: {name}"))
        .canonicalize()
        .unwrap()
}

fn version(path: &Path) -> Value {
    let out = Command::new(path)
        .arg("--version")
        .output()
        .unwrap_or_else(|e| panic!("native prerequisite {}: {e}", path.display()));
    assert!(out.status.success(), "{}: {out:?}", path.display());
    json!({"path":path,"sha256":sha(path),"version":String::from_utf8(out.stdout).unwrap().trim()})
}

impl Tools {
    pub fn installed() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let cmake = resolve("cmake");
        let ctest = resolve("ctest");
        let compiler = resolve("cc");
        let make = resolve("make");
        let cargo = PathBuf::from(env!("CARGO")).canonicalize().unwrap();
        let versions = json!({
            "cmake":version(&cmake),"ctest":version(&ctest),
            "compiler":version(&compiler),"make":version(&make),"cargo":version(&cargo)
        });
        let scratch = std::env::temp_dir().join(format!(
            "arc-cmake-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&scratch).unwrap();
        let shims = scratch.join("tool observers");
        let observers = json!([
            observer(&shims, "cmake", &cmake),
            observer(&shims, "ctest", &ctest),
            observer(&shims, "cargo", &cargo)
        ]);
        Self {
            cmake,
            ctest,
            compiler,
            make,
            cargo,
            scratch,
            shims,
            observers,
            versions,
        }
    }

    pub fn receipt(&self) -> Value {
        json!({"programs":self.versions,"offline":true,"generator":"Unix Makefiles",
            "dependencies":"C standard library only; configured and compiled by fixture",
            "privateWritableCache":true,"fixtureSources":[
                sha(&root().join("crates/arc-daemon/tests/traffic_native_tools/cmake/sources/CMakeLists.txt")),
                sha(&root().join("crates/arc-daemon/tests/traffic_native_tools/cmake/sources/fixture.c"))]})
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
impl Drop for Tools {
    fn drop(&mut self) {
        // Only this invocation's private observer/cache tree, after owned children.
        let _ = fs::remove_dir_all(&self.scratch);
    }
}
