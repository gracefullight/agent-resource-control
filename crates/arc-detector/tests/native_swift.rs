#[path = "support/native.rs"]
mod support;
use arc_detector::Reason::{SwiftBuild, SwiftTest};

#[test]
fn swift_grammar_positive() {
    for exe in [
        "swift",
        "./swift",
        "../tool/swift",
        "/tool path/日本語/swift",
    ] {
        for (verb, reason) in [("build", SwiftBuild), ("test", SwiftTest)] {
            support::known(&[exe, verb], reason);
            for flag in ["-v", "--verbose", "-q", "--quiet"] {
                support::known(&[exe, verb, flag], reason);
            }
            for (flag, value) in [
                ("-c", "debug"),
                ("--configuration", "release"),
                ("--package-path", "project path"),
                ("--scratch-path", "build path"),
                ("-j", "2"),
                ("--jobs", "1024"),
            ] {
                support::known(&[exe, verb, flag, value], reason);
                if flag.starts_with("--") {
                    support::known(&[exe, verb, &format!("{flag}={value}")], reason);
                }
            }
        }
    }
    support::known(&["swift", "build", "--build-tests"], SwiftBuild);
    for flag in ["--parallel", "--skip-build", "--enable-code-coverage"] {
        support::known(&["swift", "test", flag], SwiftTest);
    }
    support::known(
        &[
            "swift",
            "test",
            "--parallel",
            "--num-workers=2",
            "--filter",
            "Suite.test",
            "--filter=Other.*",
            "--skip",
            "Slow",
            "--skip=Flaky",
        ],
        SwiftTest,
    );
    support::known(
        &[
            "swift",
            "test",
            "--num-workers",
            "01",
            "--parallel",
            "--filter",
            "[",
            "--skip",
            "literal $HOME ' 日本語",
            "--package-path=missing project",
            "--scratch-path",
            "./out path",
            "-c",
            "release",
            "--jobs=0002",
            "--skip-build",
            "--enable-code-coverage",
            "--quiet",
        ],
        SwiftTest,
    );
    support::known(
        &[
            "swift",
            "build",
            "--configuration=debug",
            "--package-path",
            "../absent",
            "--scratch-path=~/out",
            "-j",
            "1",
            "--build-tests",
            "--verbose",
        ],
        SwiftBuild,
    );
}
#[test]
fn swift_grammar_negative() {
    for args in [
        vec!["swift"],
        vec!["swift/", "build"],
        vec!["swift", "run"],
        vec!["swift", "package", "build"],
        vec!["swift", "-c", "debug", "build"],
        vec!["swift", "build", "--target", "custom"],
        vec!["swift", "build", "--product=custom"],
        vec!["swift", "build", "--configuration", "profile"],
        vec!["swift", "build", "-cdebug"],
        vec!["swift", "build", "-c=debug"],
        vec!["swift", "build", "-j2"],
        vec!["swift", "build", "--jobs"],
        vec!["swift", "build", "--jobs=0"],
        vec!["swift", "build", "--jobs=1025"],
        vec!["swift", "test", "--num-workers=2"],
        vec!["swift", "test", "--filter="],
        vec!["swift", "test", "list"],
        vec!["swift", "test", "last"],
        vec!["swift", "test", "--list-tests"],
        vec!["swift", "test", "--debugger"],
        vec!["swift", "test", "--", "payload"],
        vec!["swift", "build", "-Xswiftc", "-custom"],
        vec!["swift", "build", "--package-path", ""],
        vec!["swift", "build", "-v", "--quiet"],
        vec!["swift", "test", "-j", "1", "--jobs=2"],
        vec!["swift", "build", "--help"],
        vec!["swift", "test", "-v=true"],
        vec!["swift", "test", ""],
        vec!["swift.exe", "build"],
        vec!["Swift", "test"],
        vec!["swift", "build", "--test"],
        vec!["swift", "build", "--parallel"],
        vec!["swift", "build", "--skip-build"],
        vec!["swift", "build", "--enable-code-coverage"],
        vec!["swift", "build", "--filter=anything"],
        vec!["swift", "test", "--build-tests"],
        vec!["swift", "test", "--target=unresolved"],
        vec!["swift", "test", "--product", "missing"],
        vec!["swift", "test", "--configuration=Debug"],
        vec!["swift", "test", "--configuration="],
        vec!["swift", "test", "-c", "debug", "--configuration=debug"],
        vec!["swift", "build", "--verbose", "-v"],
        vec!["swift", "test", "--parallel", "--parallel"],
        vec!["swift", "test", "--parallel=true"],
        vec!["swift", "test", "--skip-build=false"],
        vec![
            "swift",
            "test",
            "--parallel",
            "--num-workers",
            "1",
            "--num-workers=2",
        ],
        vec!["swift", "test", "--parallel", "--num-workers", "-1"],
        vec!["swift", "build", "--scratch-path=-out"],
        vec!["swift", "build", "--scratch-path="],
        vec!["swift", "build", "--package-path", "-project"],
        vec!["swift", "build", "--package-path=p", "--package-path", "p"],
        vec!["swift", "test", "--jobs", "+1"],
        vec!["swift", "test", "--jobs", "1.0"],
        vec!["swift", "test", "--jobs", "١"],
        vec!["swift", "test", "--jobs", "1 "],
        vec!["swift", "test", "--jobs", "9999999999999999999999999999"],
        vec!["swift", "test", "--skip", "--parallel"],
        vec!["swift", "test", "--filter", "-name"],
        vec!["swift", "test", "--filter"],
        vec!["swift", "test", "--skip="],
        vec!["swift", "test", "--"],
        vec!["swift", "test", "--version"],
        vec!["swift", "build", "--show-bin-path"],
        vec!["swift", "build", "--dry-run"],
        vec!["swift", "test", "--enable-xctest"],
        vec!["swift", "test", "-Xlinker", "-custom"],
        vec!["swift", "build", "--sdk", "path"],
        vec!["swift", "build", "--disable-sandbox"],
        vec!["swift", "build", "positional"],
        vec!["swift", "test", "Suite.test"],
        vec!["xcrun", "swift", "build"],
        vec!["env", "swift", "test"],
        vec!["sh", "-c", "swift test"],
        vec!["custom", "swift", "build"],
    ] {
        support::observe(&args);
    }
    support::limits(&["swift", "test"]);
}
#[test]
fn swift_byte_and_token_boundaries_are_checked_without_truncation() {
    use arc_detector::{analyze_argv, Reason, MAX_INPUT_BYTES, MAX_TOKENS, MAX_WORK};
    let prefix = ["swift", "test", "--package-path"];
    let available = MAX_INPUT_BYTES - prefix.iter().map(|s| s.len()).sum::<usize>();
    let path = "x".repeat(available);
    support::known(&["swift", "test", "--package-path", &path], SwiftTest);
    let mut args = support::argv(&["swift", "test", "--package-path", &(path + "x")]);
    assert_eq!(analyze_argv(&args).0.reason, Reason::InputLimit);
    args = support::argv(&["swift", "test"]);
    while args.len() < MAX_TOKENS {
        args.extend(support::argv(&["--filter", "x"]));
    }
    assert_eq!(args.len(), MAX_TOKENS);
    let (good, stats) = analyze_argv(&args);
    assert_eq!(good.reason, SwiftTest);
    assert_eq!(stats.tokens, MAX_TOKENS);
    args.push("--parallel".into());
    assert_eq!(analyze_argv(&args).0.reason, Reason::TokenLimit);
    let large_inline = format!("--filter={}", "a".repeat(MAX_INPUT_BYTES - 20));
    let argv = support::argv(&["swift", "test", &large_inline]);
    let (result, stats) = analyze_argv(&argv);
    assert_eq!(result.reason, Reason::WorkLimit);
    assert!(stats.work <= MAX_WORK);
    assert_eq!(analyze_argv(&argv), (result, stats));
}
#[test]
fn swift_poison_no_execution() {
    support::poison(
        "swift_poison_no_execution",
        &[
            (&["swift", "build"], SwiftBuild),
            (&["swift", "test"], SwiftTest),
        ],
    );
}
