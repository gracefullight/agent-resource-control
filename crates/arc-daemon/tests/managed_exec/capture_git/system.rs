//! Isolated system/global/repository Git configuration precedence fixtures.
//!
//! `GIT_CONFIG_SYSTEM` replaces the build-time system path; without that
//! override the installation path cannot be inferred from the executable.
//! Source: https://git-scm.com/docs/git
//! The fixture `git` and `gh` programs are offline stubs; capture itself must
//! discover the effective configuration without executing either program.
//!
//! Report: Git's configuration documentation specifies system, global,
//! repository, environment and `-c` ordering; values such as URLs may be
//! multivalued: https://git-scm.com/docs/git-config. The selected system
//! override is read under the same byte/file/time limits as other sources;
//! missing overrides are distinct from an unproved build-time installation.
//! Local, conflicting, unsupported and unreadable sources are conservative.
//! Focused commands: `cargo test -p arc-cli --bin arcctl exec::ci_context::
//! --offline --locked` and `cargo test -p arc-daemon --test managed_exec
//! capture_git:: --offline --locked`. No host Git/gh configuration is a fixture.
//!
//! Integration handoff: the named tables below are the exact fixtures to
//! preserve in task-6's `capture_integration.rs`. The first table runs all 23
//! cases with `GIT_CONFIG_NOSYSTEM` absent and `false`; the second runs its
//! 15 named path cases plus `null_global_override`, `system_rewrite_true`,
//! and `system_rewrite_false`. Each fixture uses the temporary home, repo,
//! global and system paths established by `Sandbox`; the unproved-default
//! cases remove `GIT_CONFIG_SYSTEM` and must not open a real system config.
//!
//! Handoff report: the earlier combined-route observation used a repository
//! fixture that Git did not recognize. It does not prove this matrix. In the
//! local worktree at snapshot HEAD f9e21b81f738023ba5950704b5bd325896378376,
//! with the uncommitted repository-fixture repair, an independent Git
//! preflight recognizes the temporary repo and its layered configuration.
//! A temporary combined-route extension then exercised all 64 combinations
//! against the built CLI and daemon: both routes reported 28 REMOTE_CI and
//! 36 UNKNOWN/OBSERVE, with zero HOST_LOCAL. In 28 uncertain Git cases exec
//! reported ENDPOINT_AMBIGUOUS and classify reported UNRESOLVED_COMMAND.
//! These were the following ten cases in both unset and false modes:
//! system_bare_local, system_explicit_local, system_missing_destination,
//! system_unsupported_include, system_malformed,
//! system_local_pushurl_over_remote_fetch, system_and_repo_pushurls_conflict,
//! system_rewrite_to_local, system_rewrite_conflicts_repo, and
//! command_over_env_default; plus bare_unproved_installation_unset,
//! bare_unproved_installation_false, explicit_unproved_installation_unset,
//! symlink_override_unset, directory_override_false, relative_override,
//! invalid_nosystem, and system_rewrite_false. Shared
//! `exec/capture.rs::merge_invocation` discards an UNKNOWN invocation's
//! context; task-6 should assess the discrepancy without lending one shell
//! leaf's facts to another.
//! The first temporary comparison mistakenly restored a removed
//! GIT_CONFIG_NOSYSTEM and failed. Copying the exec environment onto an
//! empty command fixed that test setup and produced the results above.
//! To retain the classify regressions, construct a fresh CLI command with
//! `env_clear`, copy the exec command's `get_envs` entries, preserve the
//! fixture cwd, and pass `classify`, `--json` and the fixture argv joined
//! with spaces. Compare all four JSON fields. Do not reintroduce the default
//! GIT_CONFIG_NOSYSTEM from `Sandbox::base_command` when the case removes it.
//! The temporary extension itself is not part of this task's permanent tests
//! because task-1 alone does not provide the endpoint-aware classify route.
//!
//! Task-1-only check: an archive of task-1 input snapshot
//! 05204a4563b3cfc8deb143206b607e10ba7aaf33 was overlaid with this lane's
//! current ci_context, remote_ci and capture_git files, and task-1's child.rs
//! and shared_execution_contract.rs from snapshot
//! f9e21b81f738023ba5950704b5bd325896378376. No endpoint or classify lane
//! files were added. Offline build passed; focused CI-context, detector and
//! real-daemon Git checks passed 11, 2 and 9 tests, respectively.

use super::*;

#[test]
fn sandbox_is_a_git_repository_with_layered_configuration() {
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let system = s.home.join("system");
    fs::write(
        &system,
        "[remote]\n pushDefault = system\n[url \"https://github.com/example/\"]\n pushInsteadOf = alias:\n",
    )
    .unwrap();
    fs::write(s.home.join("global"), "[remote]\n pushDefault = global\n").unwrap();
    s.config("[remote]\n pushDefault = repository\n[remote \"origin\"]\n url = alias:app.git\n");

    // This is an independent fixture preflight, before any ARC classification.
    // Git sees only temporary config sources; ARC executions still use the stubs.
    let check = |args: &[&str], no_system: Option<&str>, environment: &[(&str, &str)]| {
        let mut command = Command::new("git");
        command
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", &s.home)
            .env("XDG_CONFIG_HOME", s.home.join("xdg"))
            .env("GIT_CEILING_DIRECTORIES", &s.root)
            .env("GIT_CONFIG_SYSTEM", &system)
            .env("GIT_CONFIG_GLOBAL", s.home.join("global"))
            .env("LC_ALL", "C")
            .current_dir(&s.repo)
            .args(args);
        if let Some(value) = no_system {
            command.env("GIT_CONFIG_NOSYSTEM", value);
        }
        for (key, value) in environment {
            command.env(key, value);
        }
        let result =
            child_watchdog::output(&mut command, "isolated Git fixture preflight").unwrap();
        assert!(
            result.status.success(),
            "isolated Git fixture preflight failed"
        );
        assert!(result.stderr.is_empty());
        result.stdout
    };

    for mode in [None, Some("false")] {
        assert_eq!(
            check(&["rev-parse", "--is-inside-work-tree"], mode, &[]),
            b"true\n"
        );
        assert_eq!(
            check(
                &["config", "--local", "--get", "remote.origin.url"],
                mode,
                &[]
            ),
            b"alias:app.git\n"
        );
        assert_eq!(
            check(&["config", "--get", "remote.pushDefault"], mode, &[]),
            b"repository\n"
        );
        assert_eq!(
            check(&["remote", "get-url", "--push", "origin"], mode, &[]),
            b"https://github.com/example/app.git\n"
        );
        let environment = [
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "remote.pushDefault"),
            ("GIT_CONFIG_VALUE_0", "environment"),
        ];
        assert_eq!(
            check(
                &["config", "--get", "remote.pushDefault"],
                mode,
                &environment
            ),
            b"environment\n"
        );
        assert_eq!(
            check(
                &[
                    "-c",
                    "remote.pushDefault=command",
                    "config",
                    "--get",
                    "remote.pushDefault"
                ],
                mode,
                &environment
            ),
            b"command\n"
        );
    }
    s.config("[remote \"origin\"]\n url = alias:app.git\n");
    for mode in [None, Some("false")] {
        assert_eq!(
            check(&["config", "--get", "remote.pushDefault"], mode, &[]),
            b"global\n"
        );
    }
    fs::write(s.home.join("global"), "").unwrap();
    for mode in [None, Some("false")] {
        assert_eq!(
            check(&["config", "--get", "remote.pushDefault"], mode, &[]),
            b"system\n"
        );
    }
    assert!(
        !s.marker.exists(),
        "fixture preflight must not invoke the execution stubs"
    );
}

struct Case {
    name: &'static str,
    system: &'static str,
    global: &'static str,
    repo: &'static str,
    argv: &'static [&'static str],
    environment: &'static [(&'static str, &'static str)],
    remote: bool,
}

const REMOTE: &str = "[remote \"origin\"]\n url = https://github.com/example/app.git\n";
const LOCAL: &str = "[remote \"origin\"]\n url = file:///fixture/local\n";
const PUSH: &[&str] = &["git", "push"];
const EXPLICIT: &[&str] = &["git", "push", "origin"];
const GH: &[&str] = &["gh", "workflow", "run", "build.yml"];

#[test]
fn system_override_is_ordered_and_isolated_for_git_and_repository_gh() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let s = Sandbox::new(&f);
    let system_path = s.home.join("system");
    let cases = [
        Case { name: "system_bare_remote", system: REMOTE, global: "", repo: "", argv: PUSH, environment: &[], remote: true },
        Case { name: "system_explicit_remote", system: REMOTE, global: "", repo: "", argv: EXPLICIT, environment: &[], remote: true },
        Case { name: "system_repository_gh", system: REMOTE, global: "", repo: "", argv: GH, environment: &[], remote: true },
        Case { name: "system_bare_local", system: LOCAL, global: "", repo: "", argv: PUSH, environment: &[], remote: false },
        Case { name: "system_explicit_local", system: LOCAL, global: "", repo: "", argv: EXPLICIT, environment: &[], remote: false },
        Case { name: "system_gh_local", system: LOCAL, global: "", repo: "", argv: GH, environment: &[], remote: false },
        Case { name: "system_missing_destination", system: "", global: "", repo: "", argv: PUSH, environment: &[], remote: false },
        Case { name: "system_unsupported_include", system: "[include]\n path = ignored\n", global: "", repo: REMOTE, argv: PUSH, environment: &[], remote: false },
        Case { name: "system_malformed", system: "[remote \"origin\"\n url = bad\n", global: "", repo: REMOTE, argv: EXPLICIT, environment: &[], remote: false },
        Case { name: "system_gh_unsupported_include", system: "[include]\n path = ignored\n", global: "", repo: REMOTE, argv: GH, environment: &[], remote: false },
        Case { name: "system_pushurl_over_local_fetch", system: "[remote \"origin\"]\n pushurl = https://github.com/example/app.git\n", global: "", repo: LOCAL, argv: PUSH, environment: &[], remote: true },
        Case { name: "system_local_pushurl_over_remote_fetch", system: "[remote \"origin\"]\n pushurl = file:///fixture/local\n", global: "", repo: REMOTE, argv: EXPLICIT, environment: &[], remote: false },
        Case { name: "system_and_repo_pushurls_conflict", system: "[remote \"origin\"]\n pushurl = file:///fixture/local\n", global: "", repo: "[remote \"origin\"]\n url = https://github.com/example/app.git\n pushurl = https://github.com/example/app.git\n", argv: PUSH, environment: &[], remote: false },
        Case { name: "system_rewrite_to_remote", system: "[url \"https://github.com/example/\"]\n pushInsteadOf = file:///fixture/\n", global: "", repo: LOCAL, argv: PUSH, environment: &[], remote: true },
        Case { name: "system_rewrite_to_local", system: "[url \"file:///fixture/\"]\n pushInsteadOf = https://github.com/example/\n", global: "", repo: REMOTE, argv: EXPLICIT, environment: &[], remote: false },
        Case { name: "system_rewrite_gh_remote", system: "[url \"https://github.com/example/\"]\n insteadOf = alias:\n", global: "", repo: "[remote \"origin\"]\n url = alias:app.git\n", argv: GH, environment: &[], remote: true },
        Case { name: "system_rewrite_gh_local", system: "[url \"file:///fixture/\"]\n insteadOf = https://github.com/example/\n", global: "", repo: REMOTE, argv: GH, environment: &[], remote: false },
        Case { name: "system_rewrite_conflicts_repo", system: "[url \"https://github.com/example/\"]\n pushInsteadOf = alias:\n", global: "", repo: "[remote \"origin\"]\n url = alias:app.git\n[url \"file:///fixture/\"]\n pushInsteadOf = alias:\n", argv: PUSH, environment: &[], remote: false },
        Case { name: "repo_longer_rewrite_beats_system", system: "[url \"file:///fixture/\"]\n pushInsteadOf = alias:\n", global: "", repo: "[remote \"origin\"]\n url = alias:app.git\n[url \"https://github.com/example/app\"]\n pushInsteadOf = alias:app\n", argv: PUSH, environment: &[], remote: true },
        Case { name: "global_over_system_default", system: "[remote]\n pushDefault = local\n[remote \"local\"]\n url = file:///fixture/local\n[remote \"public\"]\n url = https://github.com/example/app.git\n", global: "[remote]\n pushDefault = public\n", repo: "", argv: PUSH, environment: &[], remote: true },
        Case { name: "repo_over_global_default", system: "[remote \"local\"]\n url = file:///fixture/local\n[remote \"public\"]\n url = https://github.com/example/app.git\n", global: "[remote]\n pushDefault = local\n", repo: "[remote]\n pushDefault = public\n", argv: PUSH, environment: &[], remote: true },
        Case { name: "env_over_repo_default", system: "[remote \"local\"]\n url = file:///fixture/local\n[remote \"public\"]\n url = https://github.com/example/app.git\n", global: "", repo: "[remote]\n pushDefault = local\n", argv: PUSH, environment: &[("GIT_CONFIG_COUNT", "1"), ("GIT_CONFIG_KEY_0", "remote.pushDefault"), ("GIT_CONFIG_VALUE_0", "public")], remote: true },
        Case { name: "command_over_env_default", system: "[remote \"local\"]\n url = file:///fixture/local\n[remote \"public\"]\n url = https://github.com/example/app.git\n", global: "", repo: "", argv: &["git", "-c", "remote.pushDefault=local", "push"], environment: &[("GIT_CONFIG_COUNT", "1"), ("GIT_CONFIG_KEY_0", "remote.pushDefault"), ("GIT_CONFIG_VALUE_0", "public")], remote: false },
    ];
    let (mut remote_count, mut unknown_count) = (0, 0);
    for mode in [None, Some("false")] {
        for case in &cases {
            fs::write(&system_path, case.system).unwrap();
            fs::write(s.home.join("global"), case.global).unwrap();
            s.config(case.repo);
            let mut command = s.command(&f, case.argv);
            command.env("GIT_CONFIG_SYSTEM", &system_path);
            if let Some(mode) = mode {
                command.env("GIT_CONFIG_NOSYSTEM", mode);
            } else {
                command.env_remove("GIT_CONFIG_NOSYSTEM");
            }
            for (key, value) in case.environment {
                command.env(key, value);
            }
            assert_eq!(s.execute(command).0, 23, "{} / {mode:?}", case.name);
            let actual = last_record(&f)["classification"].clone();
            assert_eq!(
                actual["classification"],
                if case.remote { "REMOTE_CI" } else { "UNKNOWN" },
                "{} / {mode:?}: {actual}",
                case.name
            );
            if case.remote {
                remote_count += 1;
            } else {
                unknown_count += 1;
                assert_eq!(actual["uncertainty"], "OBSERVE", "{} / {mode:?}", case.name);
            }
            assert_ne!(actual["locality"], "HOST_LOCAL", "{} / {mode:?}", case.name);
        }
    }
    assert_eq!((cases.len() * 2, remote_count, unknown_count), (46, 20, 26));
    assert_eq!(
        fs::read_to_string(&s.marker).unwrap().lines().count(),
        cases.len() * 2
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn unproved_system_paths_missing_files_and_nosystem_are_conservative() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let s = Sandbox::new(&f);
    let system_path = s.home.join("system");
    fs::write(&system_path, "[include]\n path = unsupported\n").unwrap();
    let link = s.home.join("system-link");
    std::os::unix::fs::symlink(&system_path, &link).unwrap();
    let missing = s.home.join("missing-system");

    let cases = [
        ("bare_unproved_installation_unset", None, None, PUSH, false),
        (
            "bare_unproved_installation_false",
            Some("false"),
            None,
            PUSH,
            false,
        ),
        (
            "explicit_unproved_installation_unset",
            None,
            None,
            EXPLICIT,
            false,
        ),
        (
            "gh_unproved_installation_false",
            Some("false"),
            None,
            GH,
            false,
        ),
        (
            "missing_override_unset",
            None,
            Some(missing.as_path()),
            PUSH,
            true,
        ),
        (
            "missing_override_false",
            Some("false"),
            Some(missing.as_path()),
            GH,
            true,
        ),
        (
            "symlink_override_unset",
            None,
            Some(link.as_path()),
            PUSH,
            false,
        ),
        (
            "directory_override_false",
            Some("false"),
            Some(s.home.as_path()),
            EXPLICIT,
            false,
        ),
        (
            "relative_override",
            None,
            Some(Path::new("system")),
            PUSH,
            false,
        ),
        (
            "empty_override",
            Some("false"),
            Some(Path::new("")),
            GH,
            false,
        ),
        (
            "null_override",
            None,
            Some(Path::new("/dev/null")),
            EXPLICIT,
            true,
        ),
        (
            "nosystem_skips_unsupported_system",
            Some("true"),
            Some(system_path.as_path()),
            PUSH,
            true,
        ),
        ("nosystem_skips_unproved_path", Some("1"), None, GH, true),
        (
            "nosystem_skips_relative_path",
            Some("yes"),
            Some(Path::new("system")),
            EXPLICIT,
            true,
        ),
        (
            "invalid_nosystem",
            Some("maybe"),
            Some(missing.as_path()),
            PUSH,
            false,
        ),
    ];
    s.config(REMOTE);
    let (mut remote_count, mut unknown_count) = (0, 0);
    for (name, no_system, system, argv, remote) in cases {
        let mut command = s.command(&f, argv);
        if let Some(value) = no_system {
            command.env("GIT_CONFIG_NOSYSTEM", value);
        } else {
            command.env_remove("GIT_CONFIG_NOSYSTEM");
        }
        if let Some(path) = system {
            command.env("GIT_CONFIG_SYSTEM", path);
        } else {
            command.env_remove("GIT_CONFIG_SYSTEM");
        }
        assert_eq!(s.execute(command).0, 23, "{name}");
        let actual = last_record(&f)["classification"].clone();
        assert_eq!(
            actual["classification"],
            if remote { "REMOTE_CI" } else { "UNKNOWN" },
            "{name}: {actual}"
        );
        assert_ne!(actual["locality"], "HOST_LOCAL", "{name}");
        if remote {
            remote_count += 1;
        } else {
            unknown_count += 1;
            assert_eq!(actual["uncertainty"], "OBSERVE", "{name}");
        }
    }
    assert_eq!((remote_count, unknown_count), (6, 9));

    // The documented null-device override skips a poisoned global config.
    fs::write(s.home.join("global"), "[include]\n path = unsupported\n").unwrap();
    let mut command = s.command(&f, EXPLICIT);
    command
        .env_remove("GIT_CONFIG_NOSYSTEM")
        .env("GIT_CONFIG_SYSTEM", &missing)
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
    assert_eq!(s.execute(command).0, 23, "null_global_override");
    let actual = last_record(&f)["classification"].clone();
    assert_eq!(actual["classification"], "REMOTE_CI", "{actual}");
    assert_ne!(actual["locality"], "HOST_LOCAL");

    // NOSYSTEM=true must ignore even an applicable system rewrite.
    fs::write(s.home.join("global"), "").unwrap();
    fs::write(
        &system_path,
        "[url \"file:///fixture/\"]\n pushInsteadOf = https://github.com/example/\n",
    )
    .unwrap();
    for (mode, expected) in [("true", "REMOTE_CI"), ("false", "UNKNOWN")] {
        let mut command = s.command(&f, PUSH);
        command
            .env("GIT_CONFIG_NOSYSTEM", mode)
            .env("GIT_CONFIG_SYSTEM", &system_path);
        assert_eq!(s.execute(command).0, 23, "system_rewrite_{mode}");
        let actual = last_record(&f)["classification"].clone();
        assert_eq!(
            actual["classification"], expected,
            "system_rewrite_{mode}: {actual}"
        );
        assert_ne!(actual["locality"], "HOST_LOCAL");
        if expected == "UNKNOWN" {
            assert_eq!(actual["uncertainty"], "OBSERVE");
        }
    }
    assert_eq!(fs::read_to_string(&s.marker).unwrap().lines().count(), 18);
    daemon.stop(libc::SIGTERM);
    f.report();
}
