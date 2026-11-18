use super::*;

#[test]
fn each_relocated_cmake_case_checks_cache_and_unentered_body() {
    let tools = Tools::installed();
    let prepared = prepare(
        &tools.scratch.join("validated case"),
        Form::CmakeBuild,
        &tools,
        true,
        false,
    );
    let cache = prepared.project.join("build tree/CMakeCache.txt");
    let original = fs::read_to_string(&cache).unwrap();
    for corrupt in [
        original.replace(
            "CMAKE_GENERATOR:INTERNAL=Unix Makefiles",
            "CMAKE_GENERATOR:INTERNAL=Other",
        ),
        original.replace(
            &format!("CMAKE_MAKE_PROGRAM:FILEPATH={}", tools.make.display()),
            "CMAKE_MAKE_PROGRAM:FILEPATH=/wrong/make",
        ),
    ] {
        assert_ne!(corrupt, original);
        fs::write(&cache, corrupt).unwrap();
        assert!(std::panic::catch_unwind(|| assert_prepared(&prepared.project, &tools)).is_err());
    }
    fs::write(&cache, original).unwrap();
    write(&prepared.project.join("bodies"), "unexpected body\n");
    assert!(std::panic::catch_unwind(|| assert_prepared(&prepared.project, &tools)).is_err());
    fs::remove_file(prepared.project.join("bodies")).unwrap();
    assert_prepared(&prepared.project, &tools);
}

#[test]
fn relocated_cmake_tree_executes_without_its_copy_source() {
    let tools = Tools::installed();
    let source = tools.scratch.join("intermediate source");
    let target = tools.scratch.join("relocated project");
    clone_seed(seed(&tools), &source);
    clone_seed(&source.canonicalize().unwrap(), &target);
    fs::remove_dir_all(&source).unwrap();
    let target = target.canonicalize().unwrap();
    assert_eq!(
        SEED_PREPARATIONS.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    write(&target.join("release"), "release");
    let mut build = Command::new(&tools.cmake);
    build.args(["--build", "build tree", "--target", "all"]);
    checked(build, &target);
    let mut test = Command::new(&tools.ctest);
    test.args(["--test-dir", "build tree", "-R", "^native_case$", "-V"]);
    checked(test, &target);
    let records = log(&target.join("bodies"));
    assert_eq!(records.lines().count(), 2);
    assert!(records.lines().any(|line| line.starts_with("build ")));
    assert!(records.lines().any(|line| line.starts_with("test ")));
    assert!(!seed(&tools).join("bodies").exists());
}

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
