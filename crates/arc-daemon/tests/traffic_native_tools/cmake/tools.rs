//! Resolve and exercise installed prerequisites only; no provisioning or downloads.
use super::*;
use serde_json::json;
use std::{
    fs,
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
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
    fixture_sources: [String; 2],
}

struct Installed {
    cmake: PathBuf,
    ctest: PathBuf,
    compiler: PathBuf,
    make: PathBuf,
    cargo: PathBuf,
    versions: Value,
    fixture_sources: [String; 2],
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
    let mut command = Command::new(path);
    command.arg("--version");
    let out = super::super::go::owned::fixture_output(&mut command)
        .unwrap_or_else(|e| panic!("native prerequisite {}: {e}", path.display()));
    assert!(out.status.success(), "{}: {out:?}", path.display());
    json!({"path":path,"sha256":sha(path),"version":String::from_utf8(out.stdout).unwrap().trim()})
}

impl Tools {
    pub fn installed() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        static INSTALLED: OnceLock<Installed> = OnceLock::new();
        let installed = INSTALLED.get_or_init(Self::probe);
        let scratch = std::env::temp_dir().join(format!(
            "arc-cmake-{}-{}-{}",
            std::process::id(),
            arc_core::process_identity::random_job_id().unwrap(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&scratch).unwrap();
        let shims = scratch.join("tool observers");
        let observers = json!([
            observer(&shims, "cmake", &installed.cmake),
            observer(&shims, "ctest", &installed.ctest),
            observer(&shims, "cargo", &installed.cargo)
        ]);
        Self {
            cmake: installed.cmake.clone(),
            ctest: installed.ctest.clone(),
            compiler: installed.compiler.clone(),
            make: installed.make.clone(),
            cargo: installed.cargo.clone(),
            scratch,
            shims,
            observers,
            versions: installed.versions.clone(),
            fixture_sources: installed.fixture_sources.clone(),
        }
    }

    fn probe() -> Installed {
        let cmake = resolve("cmake");
        let ctest = resolve("ctest");
        let compiler = resolve("cc");
        let make = resolve("make");
        let cargo = PathBuf::from(env!("CARGO")).canonicalize().unwrap();
        let versions = json!({
            "cmake":version(&cmake),"ctest":version(&ctest),
            "compiler":version(&compiler),"make":version(&make),"cargo":version(&cargo)
        });
        let fixture_sources = [
            sha(&root()
                .join("crates/arc-daemon/tests/traffic_native_tools/cmake/sources/CMakeLists.txt")),
            sha(&root()
                .join("crates/arc-daemon/tests/traffic_native_tools/cmake/sources/fixture.c")),
        ];
        Installed {
            cmake,
            ctest,
            compiler,
            make,
            cargo,
            versions,
            fixture_sources,
        }
    }

    pub fn receipt(&self) -> Value {
        json!({"programs":self.versions,"offline":true,"generator":"Unix Makefiles",
            "dependencies":"C standard library only; configured and compiled by fixture",
            "privateWritableCache":true,"fixtureSources":self.fixture_sources})
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
        if !std::thread::panicking() {
            if let Err(error) = fs::remove_dir_all(&self.scratch) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    panic!(
                        "cannot remove owned scratch {}: {error}",
                        self.scratch.display()
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_cmake_probe_is_shared_but_scratch_is_private() {
        let first = Tools::installed();
        let second = Tools::installed();
        assert_eq!(first.receipt(), second.receipt());
        assert_ne!(first.scratch, second.scratch);
        fs::write(first.scratch.join("private"), b"first").unwrap();
        assert!(!second.scratch.join("private").exists());
        let first_path = first.scratch.clone();
        let second_path = second.scratch.clone();
        drop(first);
        drop(second);
        assert!(!first_path.exists());
        assert!(!second_path.exists());
    }
}
