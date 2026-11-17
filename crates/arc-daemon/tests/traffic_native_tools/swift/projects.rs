//! Dependency-free packages. Direct and managed children use this exact isolation.
use super::{super::*, tools::Tools};
use std::{
    fs,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};
pub const VALUE: &str = "literal $HOME ' value 日本語";
pub const INPUT: &[u8] = b"stdin\0native bytes\n";

pub fn prepare(
    directory: &Path,
    tools: &Tools,
    name: &str,
    form: Form,
    hold: bool,
    fail: bool,
) -> Prepared {
    assert!(matches!(form, Form::SwiftBuild | Form::SwiftTest));
    let project = directory.join(name);
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

pub fn direct(tools: &Tools, prepared: &Prepared) -> Output {
    use std::io::Write;
    let mut command = Command::new(tools.shims.join("swift"));
    command
        .args(&prepared.argv[1..])
        .current_dir(&prepared.project)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure(&prepared.project, &mut command);
    command.stdin(Stdio::piped());
    // Parent-supplied fixture lineage cannot turn a direct baseline into a nested run.
    command.env_remove("TBS_PREDECESSOR");
    // Even direct baselines stop at the real native body so its live identity and
    // SwiftPM-selected evidence path are verified before explicit release.
    if prepared.project.join("release").exists() {
        fs::remove_file(prepared.project.join("release")).unwrap();
    }
    let mut child = super::process::TreeChild::spawn(command).unwrap();
    child.child.stdin.take().unwrap().write_all(INPUT).unwrap();
    let end = Instant::now() + Duration::from_secs(180);
    let mut gate = loop {
        if child.has_exited().unwrap() {
            let output = child.wait_with_output_until_exit().unwrap();
            panic!("SwiftPM exited before native evidence/barrier: {output:?}");
        }
        if let Some(gate) = super::evidence::held(&prepared.project, child.child.id()) {
            break gate;
        }
        assert!(
            Instant::now() < end,
            "SwiftPM never published native evidence and its release barrier"
        );
        std::thread::yield_now();
    };
    gate.0.write_all(b"release\n").unwrap();
    write(&prepared.project.join("release"), "release");
    let result = child.wait_with_output_until_exit().unwrap();
    assert_eq!(log(&prepared.project.join("launches")).lines().count(), 1);
    assert_eq!(
        super::evidence::body_count(&prepared.project),
        1,
        "required native Swift fixture did not enter exactly once: {result:?}"
    );
    super::streams::validate(&result.stdout, &result.stderr);
    result
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
