use super::*;

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
fn bare_push_branch_precedence_is_consistent_across_both_cli_routes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let mut daemon = f.spawn("test");
    let remotes = "[remote \"local\"]\n url = file:///fixture/local\n[remote \"public\"]\n url = https://github.com/example/app.git\n";
    let cases = [
        ("branch_pushremote_remote_over_local_defaults", "[branch \"topic\"]\n pushRemote = public\n remote = local\n[remote]\n pushDefault = local\n", "REMOTE_CI"),
        ("branch_pushremote_local_over_remote_defaults", "[branch \"topic\"]\n pushRemote = local\n remote = public\n[remote]\n pushDefault = public\n", "UNKNOWN"),
        ("default_remote_over_local_branch", "[branch \"topic\"]\n remote = local\n[remote]\n pushDefault = public\n", "REMOTE_CI"),
        ("default_local_over_remote_branch", "[branch \"topic\"]\n remote = public\n[remote]\n pushDefault = local\n", "UNKNOWN"),
        ("bare_branch_remote", "[branch \"topic\"]\n remote = public\n", "REMOTE_CI"),
        ("bare_branch_local", "[branch \"topic\"]\n remote = local\n", "UNKNOWN"),
        ("missing_branch_pushremote_does_not_fall_back", "[branch \"topic\"]\n pushRemote = missing\n remote = public\n[remote]\n pushDefault = public\n", "UNKNOWN"),
        ("ambiguous_selected_branch_pushurls", "[branch \"topic\"]\n pushRemote = public\n remote = local\n[remote \"public\"]\n pushurl = https://github.com/example/app.git\n pushurl = file:///fixture/local\n", "UNKNOWN"),
    ];
    let (mut remote, mut unknown, mut differences) = (0, 0, 0);
    for (name, selection, expected) in cases {
        write(&s.repo.join(".git/config"), format!("{remotes}{selection}"));
        let inspection = s.classify(&f, PUSH, &[]);
        let (execution, _) = s.exec(&f, PUSH, &[]);
        differences += usize::from(assert_routes(name, &inspection, &execution, expected));
        if expected == "REMOTE_CI" {
            remote += 1;
        } else {
            unknown += 1;
        }
    }
    assert_eq!((remote, unknown, s.calls()), (3, 5, 8));
    eprintln!("bare-push branch fixtures: 8 unique cases, {remote} remote, {unknown} UNKNOWN, zero false-local per route, {differences} reason differences");
    assert_eq!(differences, 0, "branch classification reasons differ");
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn git_system_configuration_is_consistent_across_both_cli_routes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let mut daemon = f.spawn("test");
    let system_path = s.home.join("system");
    #[rustfmt::skip]
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
    let (mut remote, mut unknown, mut differences) = (0, 0, 0);
    for mode in [None, Some("false")] {
        for case in &cases {
            write(&system_path, case.system);
            write(&s.home.join("global"), case.global);
            write(&s.repo.join(".git/config"), case.repo);
            let mut env = vec![
                ("GIT_CONFIG_SYSTEM", Some(system_path.to_str().unwrap())),
                ("GIT_CONFIG_NOSYSTEM", mode),
            ];
            env.extend(
                case.environment
                    .iter()
                    .map(|(key, value)| (*key, Some(*value))),
            );
            let inspection = s.classify(&f, case.argv, &env);
            let (execution, _) = s.exec(&f, case.argv, &env);
            let expected = if case.remote { "REMOTE_CI" } else { "UNKNOWN" };
            differences += usize::from(assert_routes(case.name, &inspection, &execution, expected));
            if case.remote {
                remote += 1;
            } else {
                unknown += 1;
            }
        }
    }
    assert_eq!((cases.len() * 2, remote, unknown), (46, 20, 26));

    write(&system_path, "[include]\n path = unsupported\n");
    let link = s.home.join("system-link");
    symlink(&system_path, &link).unwrap();
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
    write(&s.repo.join(".git/config"), REMOTE);
    for (name, no_system, system, argv, is_remote) in cases {
        let env = [
            ("GIT_CONFIG_NOSYSTEM", no_system),
            ("GIT_CONFIG_SYSTEM", system.map(|p| p.to_str().unwrap())),
        ];
        let inspection = s.classify(&f, argv, &env);
        let (execution, _) = s.exec(&f, argv, &env);
        let expected = if is_remote { "REMOTE_CI" } else { "UNKNOWN" };
        differences += usize::from(assert_routes(name, &inspection, &execution, expected));
        if is_remote {
            remote += 1;
        } else {
            unknown += 1;
        }
    }
    write(&s.home.join("global"), "[include]\n path = unsupported\n");
    let env = [
        ("GIT_CONFIG_NOSYSTEM", None),
        ("GIT_CONFIG_SYSTEM", Some(missing.to_str().unwrap())),
        ("GIT_CONFIG_GLOBAL", Some("/dev/null")),
    ];
    let inspection = s.classify(&f, EXPLICIT, &env);
    let (execution, _) = s.exec(&f, EXPLICIT, &env);
    differences += usize::from(assert_routes(
        "null_global_override",
        &inspection,
        &execution,
        "REMOTE_CI",
    ));
    remote += 1;
    write(&s.home.join("global"), "");
    write(
        &system_path,
        "[url \"file:///fixture/\"]\n pushInsteadOf = https://github.com/example/\n",
    );
    for (mode, expected) in [("true", "REMOTE_CI"), ("false", "UNKNOWN")] {
        let env = [
            ("GIT_CONFIG_NOSYSTEM", Some(mode)),
            ("GIT_CONFIG_SYSTEM", Some(system_path.to_str().unwrap())),
        ];
        let inspection = s.classify(&f, PUSH, &env);
        let (execution, _) = s.exec(&f, PUSH, &env);
        differences += usize::from(assert_routes(
            "system_rewrite",
            &inspection,
            &execution,
            expected,
        ));
        if expected == "REMOTE_CI" {
            remote += 1;
        } else {
            unknown += 1;
        }
    }
    eprintln!("combined Git system fixtures: 64 unique cases, 28 remote, 36 UNKNOWN, zero false-local per route; {differences} UNKNOWN reason differences");
    assert_eq!((remote, unknown, differences, s.calls()), (28, 36, 0, 64));
    daemon.stop(libc::SIGTERM);
    f.report();
}
