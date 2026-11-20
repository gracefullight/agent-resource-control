//! Read native evidence in SwiftPM's selected plugin directory; never copy a marker.
use super::super::{executable, log, sha, write};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub fn directory(project: &Path) -> Option<PathBuf> {
    if log(&project.join("form")) == "swift-test" {
        return project
            .join("entry.json")
            .is_file()
            .then(|| project.to_owned());
    }
    let mut found = Vec::new();
    let mut pending = vec![(project.join("scratch path/plugins/outputs"), 0)];
    let mut visited = 0;
    while let Some((path, depth)) = pending.pop() {
        visited += 1;
        assert!(
            visited <= 4096 && depth <= 12,
            "Swift plugin evidence traversal bound"
        );
        let entries = match fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("Swift plugin evidence {}: {error}", path.display()),
        };
        if path.file_name().unwrap() == "arc-swift-evidence" {
            if path.join("entry.json").is_file() {
                found.push(path);
            }
            continue;
        }
        for entry in entries {
            let entry = entry.unwrap();
            // A symlink cannot redirect evidence discovery outside the owned scratch tree.
            if entry.file_type().unwrap().is_dir() {
                pending.push((entry.path(), depth + 1));
            }
        }
    }
    assert!(
        found.len() <= 1,
        "multiple Swift native BUILD entries: {found:?}"
    );
    found.pop()
}

pub fn body_count(project: &Path) -> usize {
    directory(project).map_or(0, |path| log(&path.join("bodies")).lines().count())
}

const PROJECT_ENVIRONMENT: [(&str, &str); 7] = [
    ("TBS_PROJECT", ""),
    ("TBS_BODY", "bodies"),
    ("TBS_RELEASE", "release"),
    ("CFFIXED_USER_HOME", "private-home"),
    ("XDG_CONFIG_HOME", "config"),
    ("SWIFTPM_MODULECACHE_OVERRIDE", "module-cache"),
    ("CLANG_MODULE_CACHE_PATH", "clang-cache"),
];

fn expected_environment(project: &Path) -> Value {
    let mut environment = json!({
        "SDKROOT": log(&project.join("sdk-path")),
        "TBS_VALUE": super::projects::VALUE,
        "ARC_SWIFT_FORM": log(&project.join("form")),
        "ARC_SWIFT_FAIL": if project.join("fail").exists() { "1" } else { "0" }
    });
    for (key, relative) in PROJECT_ENVIRONMENT {
        environment[key] = json!(if relative.is_empty() {
            project.to_owned()
        } else {
            project.join(relative)
        });
    }
    environment
}

fn inherited_coverage() -> Value {
    let mut coverage = serde_json::Map::new();
    for (key, value) in std::env::vars_os() {
        let Some(key) = key.to_str() else { continue };
        if key == "LLVM_PROFILE_FILE" || key.starts_with("ARC_COVERAGE") {
            coverage.insert(
                key.to_owned(),
                json!(value
                    .to_str()
                    .expect("Swift fixture coverage must be UTF-8")),
            );
        }
    }
    Value::Object(coverage)
}

fn assert_environment_with_coverage(project: &Path, entry: &Value, coverage: &Value) {
    assert_eq!(entry["form"], log(&project.join("form")));
    assert_eq!(
        entry["environment"],
        expected_environment(project),
        "native Swift fixture environment differs from this case's configured values"
    );
    assert_eq!(
        entry["coverage"], *coverage,
        "native Swift fixture lost or changed inherited coverage settings"
    );
}

pub fn assert_environment(project: &Path, entry: &Value) {
    assert_environment_with_coverage(project, entry, &inherited_coverage());
}

/// Validate literal per-case values before making direct/managed records comparable.
pub fn normalize_environment(project: &Path, record: &mut Value) {
    assert_environment(project, record);
    for (key, relative) in PROJECT_ENVIRONMENT {
        record["environment"][key] = json!(if relative.is_empty() { "." } else { relative });
    }
}

fn descendant(owner: u32, pid: u32, depth: usize) -> bool {
    assert!(depth < 128, "Swift native evidence ancestry bound");
    owner == pid
        || crate::cleanup::children(owner)
            .unwrap()
            .into_iter()
            .any(|child| descendant(child, pid, depth + 1))
}

/// FIFO readiness proves the observed native child is blocked before explicit release.
pub fn held(project: &Path, owner: u32) -> Option<(File, ProcessIdentity)> {
    let path = directory(project)?;
    assert_eq!(
        body_count(project),
        1,
        "native Swift body must enter exactly once"
    );
    let entry: Value = serde_json::from_str(&log(&path.join("entry.json"))).unwrap();
    assert_environment(project, &entry);
    let file = match OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path.join("gate.ts"))
    {
        Ok(file) => file,
        Err(error) if error.raw_os_error() == Some(libc::ENXIO) => return None,
        Err(error) => panic!("Swift native FIFO writer: {error}"),
    };
    let pid = u32::try_from(entry["pid"].as_u64().unwrap()).unwrap();
    let identity = ProcessIdentity::capture(pid).unwrap();
    assert_eq!(probe_live(&identity), IdentityStatus::VerifiedMatch);
    assert!(
        descendant(owner, pid, 0),
        "evidence must come from the owned native tree"
    );
    let image = executable(pid);
    let scratch = project.join("scratch path").canonicalize().unwrap();
    if entry["form"] == "swift-test" {
        let bundle = Path::new(entry["testBundle"].as_str().unwrap())
            .canonicalize()
            .unwrap();
        assert!(
            bundle.starts_with(&scratch),
            "Swift test bundle must come from this case's leased build tree: {entry}"
        );
    }
    let argv0 = PathBuf::from(entry["argv"][0].as_str().unwrap());
    let argv0 = if argv0.is_absolute() {
        argv0
    } else {
        Path::new(entry["cwd"].as_str().unwrap()).join(argv0)
    };
    assert_eq!(image, argv0.canonicalize().unwrap());
    assert_eq!(
        Path::new(entry["evidenceRoot"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        path.canonicalize().unwrap()
    );
    if entry["form"] == "swift-build" {
        assert!(image.starts_with(&scratch));
        assert_eq!(entry["argv"][1], entry["evidenceRoot"]);
        // SwiftPM can suffix host tools (for example, -tool); use the resolved
        // tool path passed by PluginContext instead of guessing its layout/name.
        assert_eq!(
            image,
            Path::new(entry["argv"][2].as_str().unwrap())
                .canonicalize()
                .unwrap()
        );
        let generated = Path::new(entry["argv"][3].as_str().unwrap());
        assert_eq!(generated.file_name().unwrap(), "ARCGenerated.swift");
        assert_eq!(
            generated.parent().unwrap().canonicalize().unwrap(),
            path.parent().unwrap().canonicalize().unwrap()
        );
        assert!(
            !generated.exists(),
            "Swift compile input must not exist before the native barrier is released"
        );
    }
    write(
        &project.join("native-child.json"),
        serde_json::to_vec(&json!({"entry":entry,"identity":identity,
            "executable":image,"sha256":sha(&image),"evidenceDirectory":path,
            "ownedAncestor":owner,"fifoBlocked":true}))
        .unwrap(),
    );
    Some((file, identity))
}

pub fn observation(project: &Path) -> Value {
    serde_json::from_str(&log(&project.join("native-child.json"))).unwrap()
}

pub fn wait_for_retirement(project: &Path) {
    let identity: ProcessIdentity =
        serde_json::from_value(observation(project)["identity"].clone()).unwrap();
    let end = Instant::now() + Duration::from_secs(60);
    wait_for_retirement_with(&identity, crate::cleanup::capture, || Instant::now() >= end)
        .unwrap_or_else(|error| panic!("Swift native body retirement for {identity:?}: {error}"));
}

fn wait_for_retirement_with(
    identity: &ProcessIdentity,
    mut capture: impl FnMut(u32) -> std::io::Result<Option<ProcessIdentity>>,
    mut expired: impl FnMut() -> bool,
) -> std::io::Result<()> {
    loop {
        if capture(identity.pid)?.as_ref() != Some(identity) {
            return Ok(());
        }
        if expired() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "native body remained present after its wrapper exited",
            ));
        }
        std::thread::yield_now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn swift_pair_waits_for_the_exact_native_body_before_reusing_its_tree() {
        use std::sync::mpsc;

        let identity = ProcessIdentity::capture(std::process::id()).unwrap();
        let observed = identity.clone();
        let (seen, waiting) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let (finished, result) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut calls = 0;
            wait_for_retirement_with(
                &observed,
                |_| {
                    calls += 1;
                    if calls == 1 {
                        return Ok(Some(observed.clone()));
                    }
                    seen.send(()).unwrap();
                    released.recv_timeout(Duration::from_secs(60)).unwrap();
                    Ok(None)
                },
                || false,
            )
            .unwrap();
            finished.send(()).unwrap();
        });
        waiting.recv_timeout(Duration::from_secs(60)).unwrap();
        assert!(matches!(result.try_recv(), Err(mpsc::TryRecvError::Empty)));
        release.send(()).unwrap();
        result.recv_timeout(Duration::from_secs(60)).unwrap();
        worker.join().unwrap();

        let error = wait_for_retirement_with(&identity, |_| Ok(Some(identity.clone())), || true)
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    }

    #[test]
    fn swift_native_environment_rejects_missing_changed_and_cross_case_values() {
        let project = super::super::super::root().join(format!(
            "target/traffic-native-tools/swift-environment/{}",
            std::process::id()
        ));
        fs::create_dir_all(&project).unwrap();
        write(&project.join("sdk-path"), "/installed SDK path");
        let coverage = json!({
            "LLVM_PROFILE_FILE": "/owned profiles/native-%p.profraw",
            "ARC_COVERAGE_RUN_DIR": "/owned coverage 日本語",
            "ARC_COVERAGE_FIXTURE": "literal $HOME ' value"
        });
        for (form, fail) in [("swift-build", false), ("swift-test", true)] {
            write(&project.join("form"), form);
            if fail {
                write(&project.join("fail"), "1");
            }
            let entry = json!({"form":form,"environment":expected_environment(&project),
                "coverage":coverage});
            assert_environment_with_coverage(&project, &entry, &coverage);
            for key in entry["environment"].as_object().unwrap().keys() {
                for value in [Value::Null, json!("another case's value")] {
                    let mut corrupted = entry.clone();
                    corrupted["environment"][key] = value;
                    assert!(std::panic::catch_unwind(|| {
                        assert_environment_with_coverage(&project, &corrupted, &coverage)
                    })
                    .is_err());
                }
            }
            for key in coverage.as_object().unwrap().keys() {
                let mut corrupted = entry.clone();
                corrupted["coverage"].as_object_mut().unwrap().remove(key);
                assert!(std::panic::catch_unwind(|| {
                    assert_environment_with_coverage(&project, &corrupted, &coverage)
                })
                .is_err());
                corrupted["coverage"][key] = json!("rewritten coverage value");
                assert!(std::panic::catch_unwind(|| {
                    assert_environment_with_coverage(&project, &corrupted, &coverage)
                })
                .is_err());
            }
            let mut corrupted = entry.clone();
            corrupted["form"] = json!("MISSING");
            assert!(std::panic::catch_unwind(|| {
                assert_environment_with_coverage(&project, &corrupted, &coverage)
            })
            .is_err());
            let empty = json!({});
            let mut uninstrumented = entry;
            uninstrumented["coverage"] = empty.clone();
            assert_environment_with_coverage(&project, &uninstrumented, &empty);
        }
        fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn swift_plugin_discovery_reads_only_native_output_and_rejects_duplicates() {
        let project = super::super::super::root().join(format!(
            "target/traffic-native-tools/swift-discovery/{}",
            std::process::id()
        ));
        fs::create_dir_all(&project).unwrap();
        write(&project.join("form"), "swift-build");
        // Stale/parent-written files at the old manifest path cannot prove BUILD entry.
        write(&project.join("bodies"), "body\n");
        write(&project.join("entry.json"), "{}");
        assert_eq!(directory(&project), None);
        assert_eq!(body_count(&project), 0);
        let outputs = project.join("scratch path/plugins/outputs");
        fs::create_dir_all(&outputs).unwrap();
        symlink(&project, outputs.join("outside")).unwrap();
        assert_eq!(directory(&project), None);
        let evidence =
            outputs.join("package/Fixture/destination/FixtureBuildPlugin/arc-swift-evidence");
        fs::create_dir_all(&evidence).unwrap();
        write(&evidence.join("bodies"), "body\n");
        assert_eq!(directory(&project), None);
        write(&evidence.join("entry.json"), "{}");
        assert_eq!(directory(&project), Some(evidence));
        assert_eq!(body_count(&project), 1);
        let duplicate = outputs.join("duplicate/arc-swift-evidence");
        fs::create_dir_all(&duplicate).unwrap();
        write(&duplicate.join("entry.json"), "{}");
        assert!(std::panic::catch_unwind(|| directory(&project)).is_err());
        fs::remove_dir_all(project).unwrap();
    }
}
