//! Fixture-only setup may configure/compile. The detector never imports this module.
use super::*;
use std::fs;

pub fn configure(project: &Path, command: &mut Command) {
    for cache in ["cache", "cache/clang", "tmp"] {
        writable_cache(&project.join(cache));
    }
    command
        .env("TBS_PROJECT", project)
        .env("TBS_BODY", project.join("bodies"))
        .env("TBS_RELEASE", project.join("release"))
        .env("TBS_VALUE", "literal $HOME ' 日本語")
        .env_remove("TBS_PREDECESSOR")
        .env("TMPDIR", project.join("tmp"))
        .env("XDG_CACHE_HOME", project.join("cache"))
        .env("CLANG_MODULE_CACHE_PATH", project.join("cache/clang"))
        .env("CCACHE_DISABLE", "1")
        // Both direct and managed fixture runs get exactly this isolation.
        // Retain all inherited ARC, Rust and LLVM coverage variables.
        .env_remove("MAKEFLAGS")
        .env_remove("MFLAGS")
        .env_remove("CMAKE_GENERATOR")
        .env_remove("CMAKE_TOOLCHAIN_FILE");
}

fn checked(mut command: Command, project: &Path) {
    configure(project, &mut command);
    let out = command.current_dir(project).output().unwrap();
    assert!(
        out.status.success(),
        "offline CMake preparation: {command:?}\n{out:?}"
    );
}

pub fn prepare(path: &Path, form: Form, tools: &Tools, hold: bool, fail: bool) -> Prepared {
    assert!(matches!(form, Form::CmakeBuild | Form::Ctest));
    fs::create_dir_all(path).unwrap();
    let project = path.canonicalize().unwrap();
    write(
        &project.join("CMakeLists.txt"),
        include_str!("sources/CMakeLists.txt"),
    );
    write(
        &project.join("fixture.c"),
        include_str!("sources/fixture.c"),
    );
    let build = project.join("build tree");
    let mut setup = Command::new(&tools.cmake);
    setup
        .arg("-S")
        .arg(&project)
        .arg("-B")
        .arg(&build)
        .args(["-G", "Unix Makefiles", "-DCMAKE_BUILD_TYPE=Debug"])
        .arg(format!(
            "-DCMAKE_MAKE_PROGRAM:FILEPATH={}",
            tools.make.display()
        ))
        .arg(format!(
            "-DCMAKE_C_COMPILER:FILEPATH={}",
            tools.compiler.display()
        ));
    checked(setup, &project);
    let cache = log(&build.join("CMakeCache.txt"));
    assert!(cache.contains("CMAKE_GENERATOR:INTERNAL=Unix Makefiles"));
    assert!(cache.contains(&format!(
        "CMAKE_MAKE_PROGRAM:FILEPATH={}",
        tools.make.display()
    )));
    // Prepare the actual C test dependency before either direct/managed route.
    // This target is fixture setup, never a classified/scheduled command.
    let mut compile = Command::new(&tools.cmake);
    compile
        .arg("--build")
        .arg(&build)
        .args(["--target", "fixture", "--parallel", "2"]);
    checked(compile, &project);
    assert!(
        build.join("fixture").is_file(),
        "native C fixture must be compiled"
    );
    assert!(
        !project.join("bodies").exists(),
        "preparation must not enter test/build barrier"
    );
    if !hold {
        write(&project.join("release"), "release");
    }
    if fail {
        write(&project.join("fail"), "intended native failure");
    }
    let mut argv: Vec<_> = match form {
        Form::CmakeBuild => vec![
            "cmake",
            "--build",
            "build tree",
            "--config",
            "Debug",
            "--parallel",
            "2",
            "--target",
            "all",
        ],
        Form::Ctest => vec![
            "ctest",
            "--test-dir",
            "build tree",
            "--build-config",
            "Debug",
            "-j2",
            "-R",
            "^native_case$",
            "-E",
            "^absent$",
            "-L",
            "unit",
            "-LE",
            "network",
            "-V",
        ],
        _ => unreachable!(),
    }
    .into_iter()
    .map(str::to_owned)
    .collect();
    // Exercise literal executable paths containing spaces as well as build paths.
    argv[0] = tools.shims.join(&argv[0]).to_str().unwrap().to_owned();
    Prepared {
        form,
        project,
        executable: if form == Form::CmakeBuild {
            tools.cmake.clone()
        } else {
            tools.ctest.clone()
        },
        argv,
        ready,
        configure,
    }
}

pub fn ready(run: &mut Run) -> bool {
    use arc_core::process_identity::ProcessIdentity;
    if !marker_ready(run) {
        return false;
    }
    let identity = ProcessIdentity::capture(payload_pid(&run.project)).unwrap();
    assert_eq!(
        executable(identity.pid),
        run.project
            .join("build tree/fixture")
            .canonicalize()
            .unwrap()
    );
    run.wrapper.own_process(identity);
    true
}

pub fn payload_pid(project: &Path) -> u32 {
    let bodies = log(&project.join("bodies"));
    assert_eq!(bodies.lines().count(), 1, "one native C payload execution");
    bodies.split_whitespace().nth(1).unwrap().parse().unwrap()
}

pub fn clear_records(project: &Path) {
    for name in ["bodies", "launches", "release-witness"] {
        let path = project.join(name);
        if path.exists() {
            fs::remove_file(path).unwrap();
        }
    }
}

/// Preserve every byte and record order, apart from CTest's measured durations.
/// Direct and managed runs share one project, so no paths need normalization.
pub fn records(form: Form, bytes: &[u8]) -> Vec<String> {
    std::str::from_utf8(bytes)
        .expect("native output must remain valid UTF-8")
        .split_inclusive('\n')
        .map(|line| {
            if form == Form::Ctest
                && (line.starts_with("1/1 Test #1: native_case ")
                    || line.starts_with("offline    =")
                    || line.starts_with("unit       =")
                    || line.starts_with("Total Test time (real) ="))
            {
                if let Some(end) = line.find(" sec") {
                    let start = line[..end]
                        .rfind(|c: char| !c.is_ascii_digit() && c != '.')
                        .map_or(0, |index| index + 1);
                    let number = &line[start..end];
                    if !number.is_empty()
                        && number.contains('.')
                        && number.chars().all(|c| c.is_ascii_digit() || c == '.')
                    {
                        return format!("{}<duration>{}", &line[..start], &line[end..]);
                    }
                }
            }
            line.to_owned()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmake_stream_records_detect_unmarked_loss_reordering_corruption_and_placement() {
        let stdout = b"ordinary stdout before\nNATIVE_PHASE=build\nordinary stdout after\n";
        let stderr = b"ordinary stderr before\nNATIVE_STDERR=build\nordinary stderr after\n";
        let expected = [
            records(Form::CmakeBuild, stdout),
            records(Form::CmakeBuild, stderr),
        ];
        for (out, err) in [
            (
                b"NATIVE_PHASE=build\nordinary stdout after\n".as_slice(),
                stderr.as_slice(),
            ),
            (
                b"ordinary stdout after\nNATIVE_PHASE=build\nordinary stdout before\n",
                stderr,
            ),
            (
                b"ordinary stdout befOre\nNATIVE_PHASE=build\nordinary stdout after\n",
                stderr,
            ),
            (
                stdout.as_slice(),
                b"NATIVE_STDERR=build\nordinary stderr after\n",
            ),
            (
                stdout,
                b"ordinary stderr after\nNATIVE_STDERR=build\nordinary stderr before\n",
            ),
            (
                stdout,
                b"ordinary stderr before\nNATIVE_STDERR=build\nordinary stderr afTer\n",
            ),
            (stderr, stdout),
        ] {
            assert_ne!(
                expected,
                [
                    records(Form::CmakeBuild, out),
                    records(Form::CmakeBuild, err)
                ]
            );
        }
    }

    #[test]
    fn cmake_stream_normalizes_only_ctest_duration_fields() {
        let a = b"1/1 Test #1: native_case ... Passed    0.12 sec\noffline    =   0.12 sec*proc (1 test)\nTotal Test time (real) =   0.13 sec\nordinary 0.12 sec\n";
        let b = b"1/1 Test #1: native_case ... Passed    1.23 sec\noffline    =   1.23 sec*proc (1 test)\nTotal Test time (real) =   1.24 sec\nordinary 0.12 sec\n";
        assert_eq!(records(Form::Ctest, a), records(Form::Ctest, b));
        assert_ne!(records(Form::CmakeBuild, a), records(Form::CmakeBuild, b));
        let changed = std::str::from_utf8(b)
            .unwrap()
            .replace("ordinary", "changed");
        assert_ne!(
            records(Form::Ctest, a),
            records(Form::Ctest, changed.as_bytes())
        );
    }
}
