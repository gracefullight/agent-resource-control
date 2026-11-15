#[path = "support/native.rs"]
mod support;
use arc_detector::Reason::{CmakeBuild, Ctest};

#[test]
fn cmake_grammar_positive() {
    for exe in ["cmake", "./cmake", "/tool path/cmake"] {
        support::known(&[exe, "--build", "build path"], CmakeBuild);
        for flag in ["--clean-first", "-v", "--verbose"] {
            support::known(&[exe, "--build", "build", flag], CmakeBuild);
        }
        for (flag, value) in [
            ("--config", "Release"),
            ("-j", "2"),
            ("--parallel", "1024"),
            ("-t", "all"),
            ("--target", "all"),
        ] {
            support::known(&[exe, "--build", "build", flag, value], CmakeBuild);
        }
        support::known(&[exe, "--build", "build", "-j2"], CmakeBuild);
    }
    for exe in ["ctest", "./ctest", "/tool path/ctest"] {
        support::known(&[exe], Ctest);
        for flag in [
            "--output-on-failure",
            "--stop-on-failure",
            "-V",
            "--verbose",
            "-VV",
            "--extra-verbose",
            "-Q",
            "--quiet",
        ] {
            support::known(&[exe, flag], Ctest);
        }
        for (flag, value) in [
            ("--test-dir", "build path"),
            ("-C", "Debug"),
            ("--build-config", "Release"),
            ("-j", "2"),
            ("--parallel", "1024"),
            ("-R", "^test.*$"),
            ("--tests-regex", "test"),
            ("-E", "slow"),
            ("--exclude-regex", "slow"),
            ("-L", "unit"),
            ("--label-regex", "unit"),
            ("-LE", "network"),
            ("--label-exclude", "slow"),
        ] {
            support::known(&[exe, flag, value], Ctest);
        }
        support::known(
            &[
                exe,
                "-j2",
                "-L",
                "unit",
                "--label-regex",
                "fast",
                "-LE",
                "network",
                "-LE",
                "slow",
            ],
            Ctest,
        );
    }
}
#[test]
fn cmake_grammar_negative() {
    for args in [
        vec!["cmake"],
        vec!["cmake/", "--build", "build"],
        vec!["cmake", "-S", ".", "-B", "build"],
        vec!["cmake", "-P", "script.cmake"],
        vec!["cmake", "-E", "env", "cmd"],
        vec!["cmake", "--build"],
        vec!["cmake", "--build", ""],
        vec!["cmake", "--build=build"],
        vec!["cmake", "--build", "build", "--", "custom"],
        vec!["cmake", "--build", "build", "--target", "test"],
        vec!["cmake", "--build", "build", "-t", "custom"],
        vec!["cmake", "--build", "build", "--target", "all", "other"],
        vec!["cmake", "--build", "build", "--config=Debug"],
        vec!["cmake", "--build", "build", "-j0"],
        vec!["cmake", "--build", "build", "--parallel"],
        vec!["cmake", "--build", "build", "-j2", "--parallel", "3"],
        vec!["cmake", "--build", "build", "--preset", "custom"],
        vec!["ctest", "-S", "script.cmake"],
        vec!["ctest", "-SP", "script.cmake"],
        vec!["ctest", "-D", "Experimental"],
        vec!["ctest", "-M", "Nightly", "-T", "Test"],
        vec!["ctest", "--build-and-test", ".", "build"],
        vec!["ctest", "-N"],
        vec!["ctest", "--show-only"],
        vec!["ctest", "--", "payload"],
        vec!["ctest", "--test-dir="],
        vec!["ctest", "--test-dir=build"],
        vec!["ctest", "-Rtest"],
        vec!["ctest", "-R", ""],
        vec!["ctest", "-R", "one", "-R", "two"],
        vec!["ctest", "-Q", "--verbose"],
        vec!["ctest", "-j1025"],
        vec!["ctest", "--unknown"],
        vec!["ctest", "build"],
        vec!["ctest", ""],
    ] {
        support::observe(&args);
    }
    support::limits(&["cmake", "--build", "build"]);
    support::limits(&["ctest"]);
}
#[test]
fn cmake_poison_no_execution() {
    support::poison(
        "cmake_poison_no_execution",
        &[
            (&["cmake", "--build", "build"], CmakeBuild),
            (
                &["cmake", "--build", "nonexistent tree", "--target", "all"],
                CmakeBuild,
            ),
            (&["ctest"], Ctest),
            (
                &[
                    "ctest",
                    "--test-dir",
                    "nonexistent tree",
                    "-R",
                    "[not evaluated",
                ],
                Ctest,
            ),
        ],
    );
}

#[test]
fn cmake_paths_selectors_and_configuration_are_literal() {
    for path in [
        ".",
        "../build",
        "/tmp/build space",
        "build ' $HOME 日本語",
        "./-build",
    ] {
        for config in ["Debug", "Release", "RelWithDebInfo", "MinSizeRel"] {
            support::known(
                &[
                    "/native path/cmake",
                    "--build",
                    path,
                    "--config",
                    config,
                    "--parallel",
                    "2",
                    "--clean-first",
                    "-v",
                    "--target",
                    "all",
                ],
                CmakeBuild,
            );
            support::known(
                &[
                    "../native path/ctest",
                    "-C",
                    config,
                    "--test-dir",
                    path,
                    "-R",
                    "literal [uncompiled regex",
                    "-E",
                    "skip.*",
                    "-j1",
                    "-L",
                    "unit",
                    "-LE",
                    "network",
                    "--output-on-failure",
                    "--stop-on-failure",
                    "-VV",
                ],
                Ctest,
            );
        }
    }
    for number in ["1", "2", "1024", "0001"] {
        support::known(&["cmake", "--build", ".", "-j", number], CmakeBuild);
        support::known(&["ctest", "--parallel", number], Ctest);
    }
}

#[test]
fn cmake_arity_alias_duplicates_and_unsupported_forms_bypass_whole_argv() {
    for prefix in [vec!["cmake", "--build", "build"], vec!["ctest"]] {
        for flag in ["--help", "--version", "--", "-n", "--dry-run", "--unknown"] {
            let mut args = prefix.clone();
            args.push(flag);
            support::observe(&args);
        }
        for number in [
            "0",
            "1025",
            "-1",
            "+2",
            "2.0",
            "2x",
            "２",
            "999999999999999999999",
        ] {
            let mut args = prefix.clone();
            args.extend(["--parallel", number]);
            support::observe(&args);
        }
        for flags in [vec!["-j", "2", "-j3"], vec!["--parallel", "1", "-j", "2"]] {
            let mut args = prefix.clone();
            args.extend(flags);
            support::observe(&args);
        }
    }
    for (prefix, flags) in [
        (
            vec!["cmake", "--build", "build"],
            vec!["--clean-first", "-v", "--config", "-j", "-t"],
        ),
        (
            vec!["ctest"],
            vec![
                "--output-on-failure",
                "--stop-on-failure",
                "-V",
                "--test-dir",
                "-C",
                "-j",
                "-R",
                "-E",
            ],
        ),
    ] {
        for flag in flags {
            let mut args = prefix.clone();
            // Valid values are provided first so rejection is about repetition.
            let value = match flag {
                "--config" | "-C" => Some("Debug"),
                "-j" => Some("2"),
                "-t" => Some("all"),
                "--test-dir" | "-R" | "-E" => Some("literal"),
                _ => None,
            };
            for _ in 0..2 {
                args.push(flag);
                args.extend(value);
            }
            support::observe(&args);
        }
    }
    for flag in ["--config", "-j", "--parallel", "-t", "--target"] {
        for value in [None, Some(""), Some("--other")] {
            let mut args = vec!["cmake", "--build", "build", flag];
            args.extend(value);
            support::observe(&args);
        }
    }
    for flag in [
        "--test-dir",
        "-C",
        "--build-config",
        "-j",
        "--parallel",
        "-R",
        "--tests-regex",
        "-E",
        "--exclude-regex",
        "-L",
        "--label-regex",
        "-LE",
        "--label-exclude",
    ] {
        for value in [None, Some(""), Some("--other")] {
            let mut args = vec!["ctest", flag];
            args.extend(value);
            support::observe(&args);
        }
    }
    for args in [
        vec!["cmake", "--build", "-build"],
        vec!["cmake", "--build", "build", "--config", "debug"],
        vec!["cmake", "--build", "build", "--config", "ASan"],
        vec![
            "cmake", "--build", "build", "--config", "Debug", "--config", "Release",
        ],
        vec!["cmake", "--build", "build", "-t", "all", "--target", "all"],
        vec!["cmake", "--build", "build", "-v", "--verbose"],
        vec!["cmake", "--build", "build", "--parallel=2"],
        vec!["cmake", "--build", "build", "--target=all"],
        vec!["cmake", "--build", "build", "--parallel2"],
        vec!["cmake", "--build", "build", "-tall"],
        vec!["cmake", "--build", "build", "-j=2"],
        vec!["ctest", "-C", "debug"],
        vec!["ctest", "-C", "Debug", "--build-config", "Release"],
        vec!["ctest", "-E", "one", "--exclude-regex", "two"],
        vec!["ctest", "-R", "one", "--tests-regex", "two"],
        vec!["ctest", "-V", "--verbose"],
        vec!["ctest", "-VV", "-Q"],
        vec!["ctest", "--parallel=2"],
        vec!["ctest", "-Sscript.cmake"],
        vec!["ctest", "-SPscript.cmake"],
        vec!["ctest", "-DExperimental"],
        vec!["ctest", "--dashboard", "Experimental"],
        vec![
            "ctest",
            "--test-dir",
            "build",
            "--build-and-test",
            ".",
            "build",
        ],
        vec!["ctest", "--test-action", "Test"],
        vec!["ctest", "--interactive-debug-mode", "1"],
        vec!["ctest/"],
        vec!["env", "ctest"],
        vec!["xcrun", "cmake", "--build", "build"],
        vec!["sh", "-c", "ctest"],
    ] {
        support::observe(&args);
    }
}

#[test]
fn cmake_byte_token_boundaries_are_checked_before_native_recognition() {
    use arc_detector::{analyze_argv, Reason, MAX_INPUT_BYTES, MAX_TOKENS, MAX_WORK};
    use std::ffi::OsString;
    let regex = "r".repeat(MAX_INPUT_BYTES - "ctest".len() - "-R".len());
    support::known(&["ctest", "-R", &regex], Ctest);
    let mut bytes = support::argv(&["ctest", "-R", &regex]);
    bytes[2].push("r");
    assert_eq!(analyze_argv(&bytes).0.reason, Reason::InputLimit);
    let mut tokens = support::argv(&["ctest", "--output-on-failure"]);
    for _ in 0..(MAX_TOKENS - 2) / 2 {
        tokens.extend([OsString::from("-L"), OsString::from("unit")]);
    }
    assert_eq!(tokens.len(), MAX_TOKENS);
    let (classification, budget) = analyze_argv(&tokens);
    assert_eq!(classification.reason, Ctest);
    assert_eq!(budget.tokens, MAX_TOKENS);
    assert_eq!(budget.invocations, 1);
    assert_eq!(budget.max_depth, 0);
    assert!(budget.work <= MAX_WORK);
    assert_eq!(analyze_argv(&tokens), (classification, budget));
    tokens.push(OsString::from("--stop-on-failure"));
    assert_eq!(analyze_argv(&tokens).0.reason, Reason::TokenLimit);
}
