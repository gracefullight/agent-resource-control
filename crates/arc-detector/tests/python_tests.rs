//! Deterministic, offline acceptance of the frozen native Python TEST grammar.
use arc_detector::{
    analyze_argv, classify, classify_argv, Classification, ClassifyRequest, JobClass, Locality,
    Reason, Uncertainty, MAX_DEPTH, MAX_INPUT_BYTES, MAX_INVOCATIONS, MAX_TOKENS, MAX_WORK,
};
use std::ffi::OsString;

fn argv(words: &[&str]) -> Vec<OsString> {
    words.iter().map(OsString::from).collect()
}
fn checked(words: &[&str]) -> Classification {
    let input = argv(words);
    let original = input.clone();
    let (result, stats) = analyze_argv(&input);
    assert_eq!(analyze_argv(&input), (result, stats), "{words:?}");
    assert_eq!(input, original);
    assert!(result.is_consistent(), "{words:?}: {result:?}");
    assert!(stats.work <= MAX_WORK && stats.tokens <= MAX_TOKENS);
    assert!(stats.invocations <= MAX_INVOCATIONS && stats.max_depth <= MAX_DEPTH);
    result
}
fn is_test(words: &[&str], reason: Reason) {
    assert_eq!(
        checked(words),
        Classification {
            classification: JobClass::Test,
            locality: Locality::HostLocal,
            reason,
            uncertainty: Uncertainty::None,
        },
        "{words:?}"
    );
}

#[test]
fn python_entrypoints_and_core_selectors_are_test() {
    use Reason::{PythonPytest as P, PythonUnittest as U};
    for exe in [
        "pytest",
        "./pytest",
        ".venv/bin/pytest",
        "/work tree/.venv/bin/pytest",
    ] {
        for tail in [
            vec![],
            vec!["tests"],
            vec!["[tests]/test_a.py", "tests[a].py::Case::test_one"],
            vec!["tests/test_native.py::TestNative::test_case[a::b]"],
            vec!["a.py::test_one[a::b]::other[c::d]"],
            vec!["tests with spaces/test_é.py::test_case[--help]", "other.py"],
            vec![
                "-q",
                "tests",
                "-k",
                "slow or fast",
                "-m",
                "not external",
                "-vv",
            ],
        ] {
            let mut words = vec![exe];
            words.extend(tail);
            is_test(&words, P);
        }
    }
    for exe in [
        "python",
        "python3",
        "python3.8",
        "python3.9",
        "python3.10",
        "python3.11",
        "python3.12",
        "python3.13",
        "python3.14",
        "python3.15",
        "./python3",
        ".venv/bin/python",
        "/a path/venv/bin/python3.13",
        "../env/bin/python",
    ] {
        is_test(&[exe, "-m", "pytest"], P);
        is_test(&[exe, "-m", "pytest", "tests/a.py::test_one", "-s"], P);
        is_test(&[exe, "-m", "unittest"], U);
        is_test(&[exe, "-m", "unittest", "discover"], U);
        is_test(
            &[exe, "-m", "unittest", "pkg.test_case.Case.test_one", "-v"],
            U,
        );
        is_test(
            &[
                exe,
                "-m",
                "unittest",
                "./a path/test_é.py",
                "/tmp/test_a.py",
            ],
            U,
        );
    }
    for flag in [
        "-q",
        "--quiet",
        "-qq",
        "-v",
        "--verbose",
        "-vv",
        "-x",
        "--exitfirst",
        "-s",
        "--disable-warnings",
        "--collect-only",
        "--co",
    ] {
        is_test(&["pytest", flag, "tests", flag], P);
    }
    for (option, values) in [
        ("--maxfail", vec!["1", "0001", "4294967295"]),
        (
            "--tb",
            vec!["auto", "long", "short", "line", "native", "no"],
        ),
        ("--capture", vec!["fd", "sys", "no", "tee-sys"]),
        ("--color", vec!["yes", "no", "auto"]),
    ] {
        for value in values {
            is_test(&["pytest", option, value, "tests"], P);
            is_test(&["python", "-m", "pytest", &format!("{option}={value}")], P);
        }
    }
    for flag in [
        "-v",
        "--verbose",
        "-q",
        "--quiet",
        "-f",
        "--failfast",
        "-b",
        "--buffer",
        "-c",
        "--catch",
    ] {
        is_test(
            &[
                "python",
                "-m",
                "unittest",
                flag,
                "test_case",
                "-k",
                "*native*",
            ],
            U,
        );
        is_test(
            &["python", "-m", "unittest", "discover", flag, "-k", "native"],
            U,
        );
    }
    for (option, value) in [
        ("-s", "tests"),
        ("--start-directory", "a path"),
        ("-p", "test_*.py"),
        ("--pattern", "*_case.py"),
        ("-t", "."),
        ("--top-level-directory", "/work tree"),
    ] {
        is_test(&["python", "-m", "unittest", "discover", option, value], U);
        if option.starts_with("--") {
            is_test(
                &[
                    "python",
                    "-m",
                    "unittest",
                    "discover",
                    &format!("{option}={value}"),
                ],
                U,
            );
        }
    }
}

#[test]
fn unresolved_python_forms_observe_within_budget() {
    for words in [
        vec!["pytest", ""],
        vec!["pytest", "::case"],
        vec!["pytest", "a.py::"],
        vec!["pytest", "a.py::::case"],
        vec!["pytest", "tests[a].py::"],
        vec!["pytest", "a.py::test_one[param]::"],
        vec!["pytest", "a.py::test_one[param]::::other"],
        vec!["pytest", "a.py::test_one[a::b]::::other[c::d]"],
        vec!["pytest", "a.py::test_one[param]::[other]"],
        vec!["pytest", "a.py::test_one[param"],
        vec!["pytest", "--"],
        vec!["pytest", "--help"],
        vec!["pytest", "--version"],
        vec!["pytest", "--watch"],
        vec!["pytest", "-h"],
        vec!["pytest", "-qv"],
        vec!["pytest", "-qqq"],
        vec!["pytest", "-kfoo"],
        vec!["pytest", "-k=foo"],
        vec!["pytest", "-m=x"],
        vec!["pytest", "-k"],
        vec!["pytest", "-k", ""],
        vec!["pytest", "-m", "-q"],
        vec!["pytest", "--quiet=yes"],
        vec!["pytest", "--maxfail=0"],
        vec!["pytest", "--maxfail=-1"],
        vec!["pytest", "--maxfail=4294967296"],
        vec!["pytest", "--maxfail=+1"],
        vec!["pytest", "--maxfail= 1"],
        vec!["pytest", "--maxfail=1.0"],
        vec!["pytest", "--maxfail"],
        vec!["pytest", "--tb="],
        vec!["pytest", "--tb=verbose"],
        vec!["pytest", "--capture=unknown"],
        vec!["pytest", "--color", "YES"],
        vec!["pytest", "--tb", "--color=auto"],
        vec!["pytest", "--unknown", "tests"],
        vec!["pytest/"],
        vec![".venv/bin/pytest/"],
        vec!["python3.7", "-m", "pytest"],
        vec!["python3.16", "-m", "pytest"],
        vec!["python3.013", "-m", "pytest"],
        vec!["python.exe", "-m", "pytest"],
        vec!["python3", "pytest"],
        vec!["python3", "-I", "-m", "pytest"],
        vec!["python3", "-mpytest"],
        vec!["python3", "-m"],
        vec!["python3", "-c", "import pytest"],
        vec!["python3", "-m", "pytest.__main__"],
        vec!["python3", "-m", "other"],
        vec!["python", "-m", "unittest", "discover", "test_case"],
        vec!["python", "-m", "unittest", "-v", "discover"],
        vec!["python", "-m", "unittest", "discover", "-s"],
        vec!["python", "-m", "unittest", "discover", "-s=tests"],
        vec!["python", "-m", "unittest", "discover", "--pattern="],
        vec![
            "python",
            "-m",
            "unittest",
            "discover",
            "--start-directory",
            "-q",
        ],
        vec!["python", "-m", "unittest", "--start-directory=tests"],
        vec!["python", "-m", "unittest", "-k=x"],
        vec!["python", "-m", "unittest", "--"],
        vec!["python", "-m", "unittest", "--help"],
        vec!["python", "-m", "unittest", "test.py::case"],
        vec!["python", "-m", "unittest", "pkg..case"],
        vec!["python", "-m", "unittest", "case-name"],
        vec!["python", "-m", "unittest", "1case"],
        vec!["python", "-m", "unittest", "검사"],
        vec!["python", "-m", "unittest", "tests/case"],
        vec!["pytest", "bad\0arg"],
    ] {
        let actual = checked(&words);
        assert_eq!(actual.classification, JobClass::Unknown, "{words:?}");
        assert_eq!(actual.locality, Locality::Unknown);
        assert_eq!(actual.uncertainty, Uncertainty::Observe);
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        for at in 0..3 {
            let mut input = argv(&["python", "-m", "pytest"]);
            input[at] = OsString::from_vec(vec![0xff]);
            assert_eq!(classify_argv(&input).reason, Reason::UnsupportedSyntax);
        }
    }
    let long = "x".repeat(MAX_INPUT_BYTES - "pytest".len());
    assert_eq!(checked(&["pytest", &long]).reason, Reason::WorkLimit);
    assert_eq!(
        checked(&["pytest", &(long + "x")]).reason,
        Reason::InputLimit
    );
    let opaque = "x".repeat(MAX_INPUT_BYTES - "pytest-k".len());
    is_test(&["pytest", "-k", &opaque], Reason::PythonPytest);
    assert_eq!(
        checked(&["pytest", "-k", &(opaque + "x")]).reason,
        Reason::InputLimit
    );
    let at_tokens = vec![OsString::from("-q"); MAX_TOKENS];
    let mut input = at_tokens;
    input[0] = "pytest".into();
    assert_eq!(classify_argv(&input).reason, Reason::PythonPytest);
    input.push("-q".into());
    assert_eq!(classify_argv(&input).reason, Reason::TokenLimit);
    // Python syntax is additive to native argv only, including shell compounds.
    for command in [
        "pytest",
        "python -m pytest",
        "python -m unittest discover",
        "pytest && cargo test",
    ] {
        let result = classify(&ClassifyRequest {
            command: command.into(),
            shell_kind: "sh".into(),
        });
        assert_eq!(result.classification, JobClass::Unknown);
        assert_eq!(result.uncertainty, Uncertainty::Observe);
    }
}

#[test]
fn python_classification_never_executes_or_imports() {
    const CHILD: &str = "ARC_PYTHON_CLASSIFICATION_POISON";
    if let Some(path) = std::env::var_os(CHILD) {
        use arc_detector::scripts::{
            MetadataError, MetadataStats, PackageMetadata, PackageSnapshot,
        };
        struct PoisonMetadata;
        impl PackageMetadata for PoisonMetadata {
            fn package(
                &mut self,
                _: &std::path::Path,
                _: &mut MetadataStats,
            ) -> Result<PackageSnapshot, MetadataError> {
                panic!("Python recognition read project configuration")
            }
            fn revalidate(
                &mut self,
                _: &[PackageSnapshot],
                _: &mut MetadataStats,
            ) -> Result<(), MetadataError> {
                panic!("Python recognition revalidated project configuration")
            }
        }
        let root = std::path::PathBuf::from(path);
        for exe in ["python", "python3", "python3.13"] {
            is_test(
                &[exe, "-m", "pytest", "tests::test_one"],
                Reason::PythonPytest,
            );
            is_test(
                &[
                    root.join(exe).to_str().unwrap(),
                    "-m",
                    "unittest",
                    "discover",
                ],
                Reason::PythonUnittest,
            );
            let native = argv(&[exe, "-m", "pytest"]);
            let result = arc_detector::analyze_script_argv(&native, &root, &mut PoisonMetadata);
            assert_eq!(result.classification.reason, Reason::PythonPytest);
            assert!(result.packages.is_empty());
            assert_eq!(result.metadata, MetadataStats::default());
        }
        is_test(&["pytest", "-q"], Reason::PythonPytest);
        is_test(
            &[root.join("pytest").to_str().unwrap()],
            Reason::PythonPytest,
        );
        return;
    }
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    let root = std::env::temp_dir().join(format!("arc-python-poison-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    for exe in ["pytest", "python", "python3", "python3.13"] {
        let file = root.join(exe);
        fs::write(&file, "#!/bin/sh\nprintf poison > LAUNCHED\nexit 97\n").unwrap();
        fs::set_permissions(file, fs::Permissions::from_mode(0o700)).unwrap();
    }
    for module in ["pytest", "unittest", "sitecustomize", "conftest"] {
        fs::write(root.join(format!("{module}.py")), "open('IMPORTED', 'w').write('poison')\nraise RuntimeError('classification imported project code')\n").unwrap();
    }
    let result = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "python_classification_never_executes_or_imports",
            "--nocapture",
        ])
        .current_dir(&root)
        .env(CHILD, &root)
        .env("PATH", &root)
        .env("PYTHONPATH", &root)
        .output()
        .unwrap();
    let launched = root.join("LAUNCHED").exists();
    let imported = root.join("IMPORTED").exists();
    fs::remove_dir_all(root).unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        !launched && !imported,
        "classification executed a poison launcher or imported a module"
    );
}

#[test]
fn python_classification_consistency_and_legacy_projection_are_exact() {
    for reason in [Reason::PythonPytest, Reason::PythonUnittest] {
        let value = Classification {
            classification: JobClass::Test,
            locality: Locality::HostLocal,
            reason,
            uncertainty: Uncertainty::None,
        };
        assert!(value.is_consistent() && value.is_python_test());
        for invalid in [
            Classification {
                classification: JobClass::Build,
                ..value
            },
            Classification {
                locality: Locality::Remote,
                ..value
            },
            Classification {
                uncertainty: Uncertainty::Observe,
                ..value
            },
        ] {
            assert!(!invalid.is_consistent());
        }
        assert_eq!(
            value.legacy(),
            Classification {
                classification: JobClass::Unknown,
                locality: Locality::Unknown,
                reason: Reason::UnresolvedCommand,
                uncertainty: Uncertainty::Observe
            }
        );
        assert_eq!(
            serde_json::from_str::<Classification>(&serde_json::to_string(&value).unwrap())
                .unwrap(),
            value
        );
    }
}
