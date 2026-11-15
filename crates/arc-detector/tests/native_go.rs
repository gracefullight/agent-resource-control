#[path = "support/native.rs"]
mod support;
use arc_detector::Reason::{GoBuild, GoTest, GoVet};
#[path = "go/boundaries.rs"]
mod boundaries;

#[test]
fn go_grammar_positive() {
    for exe in ["go", "./go", "/tool path/go"] {
        support::known(&[exe, "build"], GoBuild);
        support::known(&[exe, "test", "./..."], GoTest);
        support::known(&[exe, "vet", "example.org/pkg"], GoVet);
    }
    for (verb, reason) in [("build", GoBuild), ("test", GoTest), ("vet", GoVet)] {
        for flag in ["-a", "-v", "-x", "-race", "-trimpath"] {
            support::known(&["go", verb, flag], reason);
        }
        for (flag, value) in [
            ("-p", "2"),
            ("-tags", "netgo,custom"),
            ("-mod", "readonly"),
            ("-modfile", "config/go.mod"),
        ] {
            support::known(&["go", verb, flag, value, "./..."], reason);
            support::known(&["go", verb, &format!("{flag}={value}")], reason);
        }
        support::known(&["go", "-C", "project path", verb], reason);
        support::known(&["go", "-C=project path", verb], reason);
        support::known(&["go", verb, "a.go", "source path/b.go"], reason);
    }
    for mode in ["readonly", "vendor", "mod"] {
        support::known(&["go", "build", "-mod", mode], GoBuild);
    }
    support::known(&["go", "build", "-o", "output path/bin", "."], GoBuild);
    support::known(&["go", "build", "-o=output path/bin", "."], GoBuild);
    for flag in ["-short", "-failfast", "-json"] {
        support::known(&["go", "test", flag], GoTest);
    }
    for (flag, value) in [
        ("-count", "1"),
        ("-parallel", "1024"),
        ("-run", "Test.*"),
        ("-skip", "Skip"),
        ("-bench", "."),
        ("-timeout", "2m"),
        ("-benchtime", "1s"),
    ] {
        support::known(&["go", "test", flag, value], GoTest);
        support::known(&["go", "test", &format!("{flag}={value}")], GoTest);
    }
    for flag in [
        "-all",
        "-composites",
        "-printf",
        "-structtag",
        "-unusedresult",
    ] {
        support::known(&["go", "vet", flag], GoVet);
    }
    for selector in [
        ".",
        "./...",
        "../pkg",
        "example.org/pkg",
        "all",
        "std",
        "cmd",
        "a_b/c+d~e",
    ] {
        for (verb, reason) in [("build", GoBuild), ("test", GoTest), ("vet", GoVet)] {
            support::known(&["go", verb, selector, selector], reason);
        }
    }
    for exe in ["../tools/go", "/usr/local/go/bin/go", "ツール/go"] {
        support::known(
            &[exe, "-C=project 日本語", "test", "-count=1", "a.go"],
            GoTest,
        );
    }
}
#[test]
fn go_grammar_negative() {
    for args in [
        vec!["go"],
        vec!["go/", "build"],
        vec!["go", "run", "main.go"],
        vec!["go", "build", "-p"],
        vec!["go", "build", "-p0"],
        vec!["go", "build", "-p", "0"],
        vec!["go", "build", "-p=1025"],
        vec!["go", "build", "-p=-1"],
        vec!["go", "build", "-p=999999999999999999999999"],
        vec!["go", "build", "-v=true"],
        vec!["go", "test", "-c"],
        vec!["go", "test", "-list", "."],
        vec!["go", "test", "-args", "-custom"],
        vec!["go", "test", "-exec", "sh"],
        vec!["go", "build", "-toolexec=sh"],
        vec!["go", "vet", "-vettool=custom"],
        vec!["go", "test", "-run"],
        vec!["go", "test", "-run="],
        vec!["go", "test", "-timeout=0s"],
        vec!["go", "test", "-count=-1"],
        vec!["go", "test", "--", "-x"],
        vec!["go", "test", ""],
        vec!["go", "-C", "", "build"],
        vec!["go", "test", "-C", "."],
        vec!["go", "-C", ".", "-C", "..", "build"],
        vec!["go", "build", "-p=1", "-p=2"],
        vec!["go", "build", "main.go", "./pkg"],
        vec!["go", "build", "pkg@v1"],
        vec!["go", "test", "./...", "-v"],
        vec!["go", "build", "-gcflags=-x"],
        vec!["go", "build", "--help"],
        vec!["go", "build", "-n"],
        vec!["go", "test", "-fuzz=Fuzz"],
        vec!["go", "test", "-coverprofile=coverage.out"],
        vec!["go", "test", "-test.run=Test"],
        vec!["go", "test", "-vet=off"],
        vec!["go", "build", "-overlay=overlay.json"],
        vec!["go", "build", "-compiler=gccgo"],
        vec!["go", "build", "-o", "-output"],
        vec!["go", "build", "-o="],
        vec!["go", "build", "-v", "-v"],
        vec!["go", "test", "-count=1", "-count", "2"],
        vec!["go", "vet", "-printf", "-printf"],
        vec!["go", "build", "-short"],
        vec!["go", "test", "-o=out"],
        vec!["go", "vet", "-run=Test"],
        vec!["go", "build", "-mod=invalid"],
        vec!["go", "build", "-p=+1"],
        vec!["go", "build", "-p=1.0"],
        vec!["go", "build", "-p=１"],
        vec!["go", "build", "./pkg", "main.go"],
        vec!["go", "build", "a file"],
        vec!["go", "build", "pkg;cargo"],
        vec!["go", "build", "$(cargo build)"],
        vec!["go", "build", "-p=2=3"],
        vec!["go", "build", "-C=."],
        vec!["go", "-C=."],
        vec!["go", "-C=.", "-C=..", "build"],
        vec!["go", "--version"],
        vec!["go.exe", "build"],
        vec!["gofmt", "build"],
    ] {
        support::observe(&args);
    }
    for prefix in [
        vec!["env"],
        vec!["sh", "-c"],
        vec!["custom-wrapper"],
        vec!["xcrun"],
    ] {
        let mut args = prefix;
        args.extend(["go", "build"]);
        support::observe(&args);
    }
    support::limits(&["go", "build"]);
}
#[test]
fn go_poison_no_execution() {
    support::poison(
        "go_poison_no_execution",
        &[
            (&["go", "build"], GoBuild),
            (&["go", "test"], GoTest),
            (&["go", "vet"], GoVet),
            (
                &["go", "-C=poison project", "build", "-o=output.a", "a.go"],
                GoBuild,
            ),
            (&["go", "test", "-run", "[opaque regex", "./..."], GoTest),
            (&["go", "vet", "-modfile=poison.mod", "-printf", "."], GoVet),
        ],
    );
}
