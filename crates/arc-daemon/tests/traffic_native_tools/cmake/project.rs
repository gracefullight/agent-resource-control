//! Fixture-only setup may configure/compile. The detector never imports this module.
use super::*;
use std::{fs, sync::OnceLock};

static SEED_ROOT: OnceLock<PathBuf> = OnceLock::new();
static SEED: OnceLock<PathBuf> = OnceLock::new();
#[cfg(test)]
static SEED_PREPARATIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

extern "C" fn remove_seed() {
    if let Some(path) = SEED_ROOT.get() {
        if let Err(error) = fs::remove_dir_all(path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!("cannot remove CMake seed {}: {error}", path.display());
                unsafe { libc::_exit(1) };
            }
        }
    }
}

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
    command.current_dir(project);
    let out = super::super::go::owned::fixture_output(&mut command)
        .unwrap_or_else(|error| panic!("offline CMake preparation {command:?}: {error}"));
    assert!(
        out.status.success(),
        "offline CMake preparation: {command:?}\n{out:?}"
    );
}

fn seed(tools: &Tools) -> &Path {
    SEED.get_or_init(|| {
        let base = SEED_ROOT.get_or_init(|| {
            let base = root().join(format!(
                "target/traffic-native-tools/cmake/runtime-{}-{}",
                std::process::id(),
                arc_core::process_identity::random_job_id().unwrap()
            ));
            fs::create_dir_all(&base).unwrap();
            assert_eq!(unsafe { libc::atexit(remove_seed) }, 0);
            base.canonicalize().unwrap()
        });
        let project = base.join("seed project");
        fs::create_dir(&project).unwrap();
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
        #[cfg(test)]
        SEED_PREPARATIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        project
    })
}

/// CMake's Makefiles and CTest manifests contain absolute source/build paths.
/// Clone the prepared tree and relocate only text files, retaining binary bytes
/// and private writable inodes. The real build and test commands exercise it.
fn clone_seed(source: &Path, destination: &Path) {
    assert!(!destination.exists(), "CMake case directory must be new");
    fs::create_dir_all(destination.parent().unwrap()).unwrap();
    let mut copy = Command::new("/bin/cp");
    copy.args(["-cRp"]).arg(source).arg(destination);
    let output = super::super::go::owned::fixture_output(&mut copy).unwrap();
    assert!(output.status.success(), "CMake seed copy: {output:?}");
    let destination = destination.canonicalize().unwrap();
    let old = source.to_str().unwrap();
    let new = destination.to_str().unwrap();
    fn relocate(directory: &Path, old: &str, new: &str) {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                relocate(&entry.path(), old, new);
            } else if kind.is_file() {
                let bytes = fs::read(entry.path()).unwrap();
                if !bytes.contains(&0) {
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        let rewritten = text
                            .replace(&old.replace(' ', "\\ "), &new.replace(' ', "\\ "))
                            .replace(old, new);
                        if rewritten != text {
                            // CMake regenerates when its cache appears newer than
                            // cmake.check_cache. Relocation changes paths, not the
                            // inputs, so retain the copied metadata's times.
                            let metadata = entry.metadata().unwrap();
                            let times = fs::FileTimes::new()
                                .set_accessed(metadata.accessed().unwrap())
                                .set_modified(metadata.modified().unwrap());
                            let mut file = fs::OpenOptions::new()
                                .write(true)
                                .truncate(true)
                                .open(entry.path())
                                .unwrap();
                            use std::io::Write;
                            file.write_all(rewritten.as_bytes()).unwrap();
                            file.set_times(times).unwrap();
                        }
                    }
                }
            }
        }
    }
    relocate(&destination, old, new);
}

pub fn prepare(path: &Path, form: Form, tools: &Tools, hold: bool, fail: bool) -> Prepared {
    assert!(matches!(form, Form::CmakeBuild | Form::Ctest));
    clone_seed(seed(tools), path);
    let project = path.canonicalize().unwrap();
    assert_prepared(&project, tools);
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

fn assert_prepared(project: &Path, tools: &Tools) {
    let build = project.join("build tree");
    let cache = log(&build.join("CMakeCache.txt"));
    assert!(cache.contains("CMAKE_GENERATOR:INTERNAL=Unix Makefiles"));
    assert!(cache.contains(&format!(
        "CMAKE_MAKE_PROGRAM:FILEPATH={}",
        tools.make.display()
    )));
    assert!(
        build.join("fixture").is_file(),
        "native C fixture must be compiled"
    );
    assert!(
        !project.join("bodies").exists(),
        "preparation must not enter test/build barrier"
    );
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
#[path = "project_tests.rs"]
mod tests;
