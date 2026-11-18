//! Dependency-free packages. Direct and managed children use this exact isolation.
pub use super::direct::direct;
use super::{super::*, tools::Tools};
use std::{
    fs,
    process::{Command, Output, Stdio},
    time::Duration,
};
pub const VALUE: &str = "literal $HOME ' value 日本語";
pub const INPUT: &[u8] = b"stdin\0native bytes\n";
#[cfg(test)]
static SEED_PREPARATIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub(super) fn build_tree(project: &Path, tools: &Tools, form: Form) {
    assert!(!project.exists(), "SwiftPM pool tree must be new");
    assert!(matches!(form, Form::SwiftBuild | Form::SwiftTest));
    create_sources(project, tools);
    // Prepare the tree with the same form as its borrowers, at its final path.
    // This preparation is not a scenario: release the fixture body immediately
    // and remove its observations before the tree is made available.
    write(&project.join("form"), form.id());
    write(&project.join("release"), "release");
    let mut command = Command::new(&tools.swift);
    command
        .arg(if form == Form::SwiftBuild {
            "build"
        } else {
            "test"
        })
        .args(["--package-path", "package path"])
        .arg("--scratch-path")
        .arg(project.join("scratch path"))
        .args(["--configuration=debug", "--jobs", "1"])
        .current_dir(project)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if form == Form::SwiftTest {
        command.args(["--filter", "FixtureTests.testNative"]);
    }
    configure(project, &mut command);
    let description = format!("{command:?}");
    let output = super::process::TreeChild::spawn(command)
        .and_then(|mut child| child.wait_with_output(Duration::from_secs(300)))
        .unwrap_or_else(|error| panic!("offline SwiftPM preparation {description}: {error}"));
    assert!(
        output.status.success(),
        "offline SwiftPM preparation {description}: {output:?}"
    );
    reset_tree(project).expect("reset SwiftPM preparation observations");
    #[cfg(test)]
    SEED_PREPARATIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

/// Restore scenario-owned observations, leaving the path-bound products in place.
pub(super) fn reset_tree(project: &Path) -> std::io::Result<()> {
    for name in [
        "form",
        "fail",
        "release",
        "launches",
        "release-witness",
        "native-child.json",
        "entry.json",
        "semantic.json",
        "bodies",
        "gate.ts",
        "stdout",
        "stderr",
        "direct-stdout",
        "direct-stderr",
        ".arc-swift-case-passed",
    ] {
        match fs::remove_file(project.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    let mut pending = vec![(project.join("scratch path/plugins/outputs"), 0)];
    let mut visited = 0;
    while let Some((directory, depth)) = pending.pop() {
        visited += 1;
        if visited > 4096 || depth > 12 {
            return Err(std::io::Error::other("Swift plugin reset traversal bound"));
        }
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            let kind = entry.file_type()?;
            let name = entry.file_name();
            if name == "arc-swift-evidence" && kind.is_dir() {
                fs::remove_dir_all(entry.path())?;
            } else if name == "ARCGenerated.swift" && (kind.is_file() || kind.is_symlink()) {
                fs::remove_file(entry.path())?;
            } else if kind.is_dir() {
                pending.push((entry.path(), depth + 1));
            }
        }
    }
    Ok(())
}

fn create_sources(project: &Path, tools: &Tools) {
    let package = project.join("package path");
    fs::create_dir_all(package.join("Sources/Fixture")).unwrap();
    fs::create_dir_all(package.join("Tests/FixtureTests")).unwrap();
    fs::create_dir_all(package.join("Tools/FixtureBuildTool")).unwrap();
    fs::create_dir_all(package.join("Plugins/FixtureBuildPlugin")).unwrap();
    let body = include_str!("sources/Body.swift.txt");
    write(
        &package.join("Package.swift"),
        include_str!("sources/Package.swift.txt"),
    );
    write(
        &package.join("Tools/FixtureBuildTool/main.swift"),
        include_str!("sources/BuildTool.swift.txt").replace("// BODY", body),
    );
    write(
        &package.join("Plugins/FixtureBuildPlugin/Plugin.swift"),
        include_str!("sources/BuildPlugin.swift.txt"),
    );
    write(
        &package.join("Tests/FixtureTests/FixtureTests.swift"),
        include_str!("sources/FixtureTests.swift.txt").replace("// BODY", body),
    );
    write(
        &package.join("Sources/Fixture/Fixture.swift"),
        "public func fixtureAnswer() -> Int { 42 }\n",
    );
    write(&project.join("sdk-path"), tools.sdk.to_str().unwrap());
    for cache in [
        "scratch path",
        "private-home",
        "module-cache",
        "clang-cache",
        "config",
    ] {
        writable_cache(&project.join(cache));
    }
}

pub fn prepare(
    directory: &Path,
    tools: &Tools,
    name: &str,
    form: Form,
    hold: bool,
    fail: bool,
) -> Prepared {
    assert!(matches!(form, Form::SwiftBuild | Form::SwiftTest));
    let destination = directory.join(name);
    let project = tools.lease(form, &destination);
    prepare_project(project, tools, form, hold, fail)
}

fn prepare_project(
    project: PathBuf,
    tools: &Tools,
    form: Form,
    hold: bool,
    fail: bool,
) -> Prepared {
    write(&project.join("form"), form.id());
    if fail {
        write(&project.join("fail"), "1");
    }
    if !hold {
        write(&project.join("release"), "release");
    }
    // Native BUILD/TEST bodies create their own evidence and FIFO, never the manifest.
    for cache in [
        "scratch path",
        "private-home",
        "module-cache",
        "clang-cache",
        "config",
    ] {
        writable_cache(&project.join(cache));
    }
    let mut argv = vec![
        "swift".to_owned(),
        if form == Form::SwiftBuild {
            "build"
        } else {
            "test"
        }
        .into(),
        "--package-path".into(),
        "package path".into(),
        "--scratch-path".into(),
        project.join("scratch path").to_str().unwrap().to_owned(),
        "--configuration=debug".into(),
        "--jobs".into(),
        "1".into(),
    ];
    if form == Form::SwiftTest {
        argv.extend(["--filter".into(), "FixtureTests.testNative".into()]);
    }
    Prepared {
        form,
        project,
        executable: tools.image.clone(),
        argv,
        ready,
        configure,
    }
}

pub fn configure(project: &Path, command: &mut Command) {
    // Per-command fixture settings; never mutate the parent or clear inherited coverage.
    command
        .env("SDKROOT", log(&project.join("sdk-path")))
        .env("CFFIXED_USER_HOME", project.join("private-home"))
        .env("XDG_CONFIG_HOME", project.join("config"))
        .env("SWIFTPM_MODULECACHE_OVERRIDE", project.join("module-cache"))
        .env("CLANG_MODULE_CACHE_PATH", project.join("clang-cache"))
        .env("TBS_PROJECT", project)
        .env("TBS_BODY", project.join("bodies"))
        .env("TBS_RELEASE", project.join("release"))
        .env("TBS_VALUE", VALUE)
        .env("ARC_SWIFT_FORM", log(&project.join("form")))
        .env(
            "ARC_SWIFT_FAIL",
            if project.join("fail").exists() {
                "1"
            } else {
                "0"
            },
        )
        .stdin(Stdio::null());
}

fn ready(run: &mut Run) -> bool {
    let Some((file, identity)) = super::evidence::held(&run.project, run.wrapper.child.id()) else {
        return false;
    };
    run.wrapper.own_process(identity);
    run.fifo = Some(file);
    true
}

pub fn record(project: &Path) -> serde_json::Value {
    let mut record: serde_json::Value =
        serde_json::from_str(&log(&super::evidence::directory(project)
            .unwrap()
            .join("semantic.json")))
        .unwrap();
    let cwd = PathBuf::from(record["cwd"].as_str().unwrap())
        .canonicalize()
        .unwrap();
    // A build-tool command's cwd belongs to SwiftPM. Keep its actual path in the
    // native entry, and normalize only the per-case root for direct/managed comparison.
    let entry: serde_json::Value = serde_json::from_str(&log(&super::evidence::directory(project)
        .unwrap()
        .join("entry.json")))
    .unwrap();
    super::evidence::assert_environment(project, &entry);
    if entry["form"] == "swift-build" {
        // The plugin output becomes a Swift compilation input only after the
        // native build tool has crossed the explicitly released barrier.
        let generated = Path::new(entry["argv"][3].as_str().unwrap());
        let expected = if project.join("fail").exists() {
            "#error(\"ARC_SWIFT_INTENDED_BUILD_FAILURE\")\n"
        } else {
            "// ARC Swift native build succeeded\n"
        };
        assert_eq!(fs::read_to_string(generated).unwrap(), expected);
    }
    super::evidence::normalize_environment(project, &mut record);
    assert_eq!(
        Path::new(entry["cwd"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        cwd
    );
    let root = project.canonicalize().unwrap();
    record["cwd"] = serde_json::json!(match cwd.strip_prefix(&root) {
        Ok(relative) if relative.as_os_str().is_empty() => ".",
        Ok(relative) => relative.to_str().unwrap(),
        Err(_) => cwd.to_str().unwrap(),
    });
    assert_eq!(record["value"], VALUE);
    // SwiftPM controls its own test child's stdin. Compare its observed native
    // behavior with the managed run instead of assuming vendor forwarding.
    for (key, directory) in [
        ("moduleCache", "module-cache"),
        ("clangCache", "clang-cache"),
    ] {
        assert_eq!(record[key], serde_json::json!(project.join(directory)));
        record[key] = serde_json::json!(directory);
    }
    assert!(fs::read_dir(project.join("module-cache"))
        .unwrap()
        .next()
        .is_some());
    assert!(fs::read_dir(project.join("scratch path"))
        .unwrap()
        .next()
        .is_some());
    record
}

pub fn assert_result(output: &Output, form: Form, fail: bool) {
    assert_eq!(output.status.code(), Some(i32::from(fail)), "{output:?}");
    if fail {
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            text.contains(if form == Form::SwiftBuild {
                "ARC_SWIFT_INTENDED_BUILD_FAILURE"
            } else {
                "ARC_SWIFT_INTENDED_TEST_FAILURE"
            }),
            "{output:?}"
        );
    }
}

#[cfg(test)]
#[path = "relocation_tests.rs"]
mod seed_tests;
