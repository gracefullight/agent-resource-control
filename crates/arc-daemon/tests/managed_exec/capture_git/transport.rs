use super::*;

type GhSelection<'a> = (&'a str, &'a [&'a str], &'a [(&'a str, &'a str)]);

#[test]
fn gh_public_host_respects_configured_transport_in_real_daemon_execution() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let s = Sandbox::new(&f);
    let cases = [
        ("public", "", "github.com:\n  user: fixture\n", true),
        (
            "empty_transport",
            "http_unix_socket:\n",
            "github.com:\n  api_host: ''\n",
            true,
        ),
        (
            "global_socket",
            "http_unix_socket: /fixture/gh.sock\n",
            "github.com:\n  user: fixture\n",
            false,
        ),
        (
            "invalid_socket_type",
            "http_unix_socket: [invalid]\n",
            "github.com:\n  user: fixture\n",
            false,
        ),
        (
            "duplicate_global_socket",
            "http_unix_socket: /fixture/gh.sock\nhttp_unix_socket: ''\n",
            "github.com:\n  user: fixture\n",
            false,
        ),
        (
            "host_socket",
            "",
            "github.com:\n  http_unix_socket: /fixture/gh.sock\n",
            false,
        ),
        (
            "host_api_override",
            "",
            "github.com:\n  api_host: localhost\n",
            false,
        ),
        (
            "duplicate_host_api",
            "",
            "github.com:\n  api_host: localhost\n  api_host: ''\n",
            false,
        ),
        (
            "general_host_socket",
            "hosts:\n  github.com:\n    http_unix_socket: /fixture/gh.sock\n",
            "",
            false,
        ),
        (
            "general_host_api",
            "hosts:\n  github.com:\n    api_host: localhost\n",
            "",
            false,
        ),
        (
            "conflicting_host_sources",
            "hosts:\n  github.com:\n    api_host: localhost\n",
            "github.com:\n  user: fixture\n",
            false,
        ),
        ("malformed", "hosts: [\n", "", false),
        (
            "unrelated_host_override",
            "",
            "github.com:\n  user: fixture\nenterprise.invalid:\n  api_host: localhost\n",
            true,
        ),
    ];
    let explicit_repo = [
        "gh",
        "workflow",
        "run",
        "build.yml",
        "--repo",
        "github.com/example/app",
    ];
    let environment_repo = ["gh", "workflow", "run", "build.yml"];
    let selections: &[GhSelection<'_>] = &[
        ("repo_flag", &explicit_repo, &[("GH_HOST", "github.com")]),
        (
            "gh_host",
            &environment_repo,
            &[("GH_HOST", "github.com"), ("GH_REPO", "example/app")],
        ),
        (
            "gh_repo_host",
            &environment_repo,
            &[
                ("GH_HOST", "enterprise.invalid"),
                ("GH_REPO", "github.com/example/app"),
            ],
        ),
    ];
    let (mut remote_count, mut unknown_count) = (0, 0);
    for (name, general, hosts, remote) in cases {
        fs::write(s.home.join("gh/config.yml"), general).unwrap();
        fs::write(s.home.join("gh/hosts.yml"), hosts).unwrap();
        for (selection, argv, environment) in selections {
            if remote {
                remote_count += 1;
            } else {
                unknown_count += 1;
            }
            let mut command = s.command(&f, argv);
            for (key, value) in *environment {
                command.env(key, value);
            }
            assert_eq!(s.execute(command).0, 23, "exec {name}");
            let actual = last_record(&f)["classification"].clone();
            assert_eq!(
                actual["classification"],
                if remote { "REMOTE_CI" } else { "UNKNOWN" },
                "exec {name}/{selection}: {actual}"
            );
            if !remote {
                assert_eq!(actual["uncertainty"], "OBSERVE", "exec {name}");
            }
            assert_ne!(actual["locality"], "HOST_LOCAL", "exec {name}");
        }
    }
    assert_eq!(
        fs::read_to_string(&s.marker).unwrap().lines().count(),
        cases.len() * selections.len()
    );
    assert_eq!((remote_count, unknown_count), (9, 30));

    // A symlinked/unreadable effective config is not proof of public transport.
    let config = s.home.join("gh/config.yml");
    let target = s.home.join("gh/config-target");
    fs::write(&target, "http_unix_socket: /fixture/gh.sock\n").unwrap();
    fs::remove_file(&config).unwrap();
    std::os::unix::fs::symlink(target, &config).unwrap();
    assert_eq!(s.execute(s.command(&f, &explicit_repo)).0, 23);
    let actual = last_record(&f)["classification"].clone();
    assert_eq!(actual["classification"], "UNKNOWN");
    assert_eq!(actual["locality"], "UNKNOWN");
    assert_eq!(actual["uncertainty"], "OBSERVE");
    daemon.stop(libc::SIGTERM);
    f.report();
}
