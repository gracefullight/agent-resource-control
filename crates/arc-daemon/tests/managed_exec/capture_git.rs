//! Offline, isolated built-CLI evidence for Git and GitHub endpoint capture.
use super::{cli, protocol, runtime::Fixture, support};
use arc_ipc::{
    jobs::{JobId, ProcessIdentity, StatusRequest},
    traffic::{JobRequest, JobResult, OPERATION_TRAFFIC_JOB},
};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[allow(clippy::duplicate_mod)]
#[path = "../support/child_watchdog.rs"]
mod child_watchdog;
#[path = "capture_git/system.rs"]
mod system;
#[path = "capture_git/transport.rs"]
mod transport;

struct Sandbox {
    root: PathBuf,
    home: PathBuf,
    repo: PathBuf,
    bin: PathBuf,
    marker: PathBuf,
}
impl Sandbox {
    fn new(f: &Fixture) -> Self {
        let root = f.root.join("offline-git");
        let home = root.join("home");
        let repo = root.join("repo");
        let bin = root.join("bin");
        for dir in [
            &home,
            &repo.join(".git/objects/info"),
            &repo.join(".git/objects/pack"),
            &repo.join(".git/refs/heads"),
            &repo.join(".git/refs/tags"),
            &bin,
            &home.join("gh"),
            &home.join("xdg"),
        ] {
            fs::create_dir_all(dir).unwrap();
        }
        fs::write(repo.join(".git/HEAD"), "ref: refs/heads/topic\n").unwrap();
        fs::write(home.join("global"), "").unwrap();
        let marker = root.join("calls");
        for name in ["git", "gh"] {
            let stub = bin.join(name);
            fs::write(&stub, "#!/bin/sh\nprintf 'call\\n' >> \"$MARKER\"\nprintf 'argc=%s\\n' \"$#\"\nfor arg do printf 'arg=<%s>\\n' \"$arg\"; done\nprintf 'cwd=<%s>\\n' \"$(pwd -P)\"\nprintf 'value=<%s>\\n' \"$VALUE\"\nIFS= read -r line\nprintf 'stdin=<%s>\\n' \"$line\"\nprintf 'fixture-stderr\\n' >&2\nexit 23\n").unwrap();
            fs::set_permissions(stub, fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self {
            root,
            home,
            repo,
            bin,
            marker,
        }
    }
    fn config(&self, value: &str) {
        fs::write(self.repo.join(".git/config"), value).unwrap();
    }
    fn base_command(&self, f: &Fixture) -> Command {
        let mut c = Command::new(cli::binary("arcctl"));
        c.env_clear()
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join("xdg"))
            .env("GH_CONFIG_DIR", self.home.join("gh"))
            .env("GIT_CONFIG_GLOBAL", self.home.join("global"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("PATH", &self.bin)
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env("MARKER", &self.marker)
            .env("VALUE", "value with spaces")
            .current_dir(&self.repo);
        c
    }
    fn command(&self, f: &Fixture, argv: &[&str]) -> Command {
        let mut c = self.base_command(f);
        c.args(["exec", "--"]).args(argv);
        c
    }
    fn execute(&self, mut c: Command) -> (i32, String, String) {
        let stdout = self.root.join("child-out");
        let stderr = self.root.join("child-err");
        c.stdin(Stdio::piped())
            .stdout(fs::File::create(&stdout).unwrap())
            .stderr(fs::File::create(&stderr).unwrap());
        let mut child = child_watchdog::Owned::spawn(&mut c).unwrap();
        child
            .child
            .stdin
            .take()
            .unwrap()
            .write_all(b"payload\n")
            .unwrap();
        let status = child.wait("offline Git/gh CLI child").unwrap();
        (
            status.code().unwrap(),
            fs::read_to_string(stdout).unwrap(),
            fs::read_to_string(stderr).unwrap(),
        )
    }
}
fn last_record(f: &Fixture) -> Value {
    let id = cli::recent(f);
    let response = protocol::call(
        f,
        arc_ipc::jobs::OPERATION_JOB_STATUS_LOCALITY,
        json!(StatusRequest::ById { job_id: id }),
    );
    assert_eq!(response.status, "ok", "{response:?}");
    response.result.unwrap()["job"].clone()
}
fn counts(f: &Fixture) -> (i64, i64, i64, i64) {
    let db = rusqlite::Connection::open(&f.database).unwrap();
    db.query_row("SELECT (SELECT count(*) FROM traffic_entries), (SELECT count(*) FROM traffic_entries WHERE state='QUEUED'), count(*), coalesce(sum(state='ACTIVE'), 0) FROM permits", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).unwrap()
}
fn host_config(home: &Path, content: &str) {
    fs::write(home.join("gh/hosts.yml"), content).unwrap();
}

type GitCase<'a> = (
    &'a str,
    &'a str,
    &'a [&'a str],
    &'a [(&'a str, &'a str)],
    bool,
);

#[test]
fn named_bare_push_and_github_config_cases_use_real_daemon_and_isolated_home() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let s = Sandbox::new(&f);
    host_config(&s.home, "github.com:\n  user: fixture\n");
    let cases: &[GitCase<'_>] = &[
        ("bare_remote_branch", "[branch \"topic\"]\n remote = origin\n[remote \"origin\"]\n url = https://github.com/example/app.git\n", &["git", "push"], &[], true),
        ("bare_local_branch", "[branch \"topic\"]\n remote = origin\n[remote \"origin\"]\n url = file:///fixture/local\n", &["git", "push"], &[], false),
        ("bare_ambiguous_pushurls", "[remote \"origin\"]\n url = https://github.com/example/app.git\n pushurl = https://github.com/example/app.git\n pushurl = file:///fixture/local\n", &["git", "push"], &[], false),
        ("bare_branch_pushremote_first", "[branch \"topic\"]\n remote = local\n pushRemote = public\n[remote]\n pushDefault = local\n[remote \"local\"]\n url = file:///fixture/local\n[remote \"public\"]\n url = https://github.com/example/app.git\n", &["git", "push"], &[], true),
        ("bare_default_before_branch", "[branch \"topic\"]\n remote = local\n[remote]\n pushDefault = public\n[remote \"local\"]\n url = file:///fixture/local\n[remote \"public\"]\n url = https://github.com/example/app.git\n", &["git", "push"], &[], true),
        ("bare_singular_remote", "[remote \"publish\"]\n url = https://github.com/example/app.git\n", &["git", "push"], &[], true),
        ("bare_multiple_no_origin", "[remote \"one\"]\n url = https://github.com/example/app.git\n[remote \"two\"]\n url = file:///fixture/local\n", &["git", "push"], &[], false),
        ("bare_missing_config", "", &["git", "push"], &[], false),
        ("pushurl_over_fetch", "[remote \"origin\"]\n url = file:///fixture/local\n pushurl = https://github.com/example/app.git\n", &["git", "push"], &[], true),
        ("pushurl_local_over_remote", "[remote \"origin\"]\n url = https://github.com/example/app.git\n pushurl = file:///fixture/local\n", &["git", "push"], &[], false),
        ("longest_push_rewrite", "[remote \"origin\"]\n url = alias:project\n[url \"https://github.com/example/\"]\n pushInsteadOf = alias:\n[url \"file:///fixture/\"]\n pushInsteadOf = alias:project\n", &["git", "push"], &[], false),
        ("identity_push_rewrite_before_instead_of", "[remote \"origin\"]\n url = file:///fixture/repo\n[url \"file:///fixture/\"]\n pushInsteadOf = file:///fixture/\n[url \"https://github.com/example/\"]\n insteadOf = file:///fixture/\n", &["git", "push"], &[], false),
        ("bare_single_remote_push_alias", "[remote \"origin\"]\n url = https://github.com/example/app.git\n url = file:///fixture/unmatched\n[url \"https://github.com/\"]\n pushInsteadOf = https://github.com/\n", &["git", "push"], &[], true),
        ("bare_single_local_push_alias", "[remote \"origin\"]\n url = https://github.com/example/app.git\n url = file:///fixture/local\n[url \"file:///fixture/\"]\n pushInsteadOf = file:///fixture/\n", &["git", "push"], &[], false),
        ("bare_multiple_matching_push_aliases", "[remote \"origin\"]\n url = https://github.com/example/app.git\n url = file:///fixture/local\n url = https://gitlab.com/example/unmatched.git\n[url \"https://github.com/\"]\n pushInsteadOf = https://github.com/\n[url \"file:///fixture/\"]\n pushInsteadOf = file:///fixture/\n", &["git", "push"], &[], false),
        ("bare_longest_push_alias_over_shorter_conflict", "[remote \"origin\"]\n url = alias:repo.git\n[url \"https://github.com/other/\"]\n pushInsteadOf = alias:\n[url \"file:///fixture/\"]\n pushInsteadOf = alias:\n[url \"https://github.com/example/repo\"]\n pushInsteadOf = alias:repo\n", &["git", "push"], &[], true),
        ("push_rewrite_remote", "[remote \"origin\"]\n url = alias:app.git\n[url \"https://github.com/example/\"]\n pushInsteadOf = alias:\n", &["git", "push"], &[], true),
        ("explicit_remote", "[remote \"upstream\"]\n url = https://github.com/example/app.git\n", &["git", "push", "upstream", "topic"], &[], true),
        ("explicit_url", "", &["git", "push", "https://github.com/example/app.git"], &[], true),
        ("command_directory", "[remote \"origin\"]\n url = https://github.com/example/app.git\n", &["git", "-C", ".", "push"], &[], true),
        ("git_dir_override", "[remote \"origin\"]\n url = https://github.com/example/app.git\n", &["git", "push"], &[("GIT_DIR", ".git")], true),
        ("command_config_override", "[remote]\n pushDefault = local\n[remote \"local\"]\n url = file:///fixture/local\n[remote \"public\"]\n url = https://github.com/example/app.git\n", &["git", "-c", "remote.pushDefault=public", "push"], &[], true),
        ("environment_config_override", "[remote]\n pushDefault = local\n[remote \"local\"]\n url = file:///fixture/local\n[remote \"public\"]\n url = https://github.com/example/app.git\n", &["git", "push"], &[("GIT_CONFIG_COUNT", "1"), ("GIT_CONFIG_KEY_0", "remote.pushDefault"), ("GIT_CONFIG_VALUE_0", "public")], true),
        ("command_beats_environment", "[remote \"local\"]\n url = file:///fixture/local\n[remote \"public\"]\n url = https://github.com/example/app.git\n", &["git", "-c", "remote.pushDefault=local", "push"], &[("GIT_CONFIG_COUNT", "1"), ("GIT_CONFIG_KEY_0", "remote.pushDefault"), ("GIT_CONFIG_VALUE_0", "public")], false),
        ("unsupported_config_parameters", "[remote \"origin\"]\n url = https://github.com/example/app.git\n", &["git", "push"], &[("GIT_CONFIG_PARAMETERS", "unsupported")], false),
        ("relative_global_config", "[remote \"origin\"]\n url = https://github.com/example/app.git\n", &["git", "push"], &[("GIT_CONFIG_GLOBAL", "relative-config")], false),
        ("unsupported_include", "[include]\n path = other\n[remote \"origin\"]\n url = https://github.com/example/app.git\n", &["git", "push"], &[], false),
        ("gh_repo_from_git", "[remote \"origin\"]\n url = https://github.com/example/app.git\n", &["gh", "workflow", "run", "build.yml"], &[], true),
        ("gh_conflicting_remote", "[remote \"origin\"]\n url = https://github.com/example/app.git\n[remote \"other\"]\n url = https://enterprise.invalid/example/app.git\n", &["gh", "workflow", "run", "build.yml"], &[], false),
        ("gh_explicit_repo_host", "", &["gh", "workflow", "run", "build.yml", "--repo", "github.com/example/app"], &[("GH_HOST", "enterprise.invalid")], true),
        ("gh_environment_host_conflict", "[remote \"origin\"]\n url = https://github.com/example/app.git\n", &["gh", "workflow", "run", "build.yml"], &[("GH_HOST", "enterprise.invalid")], false),
        ("gh_missing_repo", "", &["gh", "workflow", "run", "build.yml"], &[], false),
    ];
    let mut remote = 0;
    let mut unknown = 0;
    for (name, config, argv, env, expected_remote) in cases {
        s.config(config);
        let mut command = s.command(&f, argv);
        for (key, value) in *env {
            command.env(key, value);
        }
        let (code, _, _) = s.execute(command);
        assert_eq!(code, 23, "{name}");
        let job = last_record(&f);
        let classification = &job["classification"];
        assert_ne!(classification["locality"], "HOST_LOCAL", "{name}");
        if *expected_remote {
            remote += 1;
            assert_eq!(
                classification["classification"], "REMOTE_CI",
                "{name}: {job}"
            );
        } else {
            unknown += 1;
            assert_eq!(classification["classification"], "UNKNOWN", "{name}: {job}");
            assert_eq!(classification["uncertainty"], "OBSERVE", "{name}");
        }
        assert_eq!(job["outcome"]["code"], 23, "{name}");
    }
    assert_eq!((cases.len(), remote, unknown), (32, 16, 16));
    assert_eq!(
        fs::read_to_string(&s.marker).unwrap().lines().count(),
        cases.len()
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn detached_head_and_gh_general_host_config_stay_conservative() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let s = Sandbox::new(&f);
    fs::write(s.repo.join(".git/HEAD"), format!("{}\n", "1".repeat(40))).unwrap();
    s.config("[remote]\n pushDefault = public\n[branch \"topic\"]\n pushRemote = local\n[remote \"public\"]\n url = https://github.com/example/app.git\n[remote \"local\"]\n url = file:///fixture/local\n");
    assert_eq!(s.execute(s.command(&f, &["git", "push"])).0, 23);
    assert_eq!(
        last_record(&f)["classification"]["classification"],
        "REMOTE_CI"
    );
    s.config("[branch \"topic\"]\n pushRemote = public\n[remote \"public\"]\n url = https://github.com/example/app.git\n[remote \"local\"]\n url = file:///fixture/local\n");
    assert_eq!(s.execute(s.command(&f, &["git", "push"])).0, 23);
    assert_eq!(
        last_record(&f)["classification"]["classification"],
        "UNKNOWN"
    );

    fs::write(s.repo.join(".git/HEAD"), "ref: refs/heads/topic\n").unwrap();
    fs::create_dir_all(s.repo.join(".git/refs/heads")).unwrap();
    fs::write(
        s.repo.join(".git/refs/heads/topic"),
        "ref: refs/heads/other\n",
    )
    .unwrap();
    assert_eq!(s.execute(s.command(&f, &["git", "push"])).0, 23);
    assert_eq!(
        last_record(&f)["classification"]["classification"],
        "UNKNOWN"
    );

    fs::write(
        s.home.join("gh/config.yml"),
        "hosts:\n  enterprise.invalid:\n    user: fixture\n",
    )
    .unwrap();
    let mut command = s.command(&f, &["gh", "workflow", "run", "build.yml"]);
    command.env("GH_REPO", "example/app");
    assert_eq!(s.execute(command).0, 23);
    assert_eq!(
        last_record(&f)["classification"]["classification"],
        "UNKNOWN"
    );
    let mut command = s.command(&f, &["gh", "workflow", "run", "build.yml"]);
    command
        .env("GH_REPO", "example/app")
        .env("GH_HOST", "github.com");
    assert_eq!(s.execute(command).0, 23);
    assert_eq!(
        last_record(&f)["classification"]["classification"],
        "REMOTE_CI"
    );
    fs::write(
        s.home.join("gh/hosts.yml"),
        "github.com:\n  user: fixture\n",
    )
    .unwrap();
    let mut command = s.command(&f, &["gh", "workflow", "run", "build.yml"]);
    command.env("GH_REPO", "example/app");
    assert_eq!(s.execute(command).0, 23);
    assert_eq!(
        last_record(&f)["classification"]["classification"],
        "UNKNOWN"
    );
    {
        // An explicit missing system override replaces the build-time path.
        fs::write(s.repo.join(".git/HEAD"), format!("{}\n", "1".repeat(40))).unwrap();
        s.config("[remote \"origin\"]\n url = https://github.com/example/app.git\n");
        let mut command = s.command(&f, &["git", "push"]);
        command.env_remove("GIT_CONFIG_NOSYSTEM");
        command.env("GIT_CONFIG_SYSTEM", s.home.join("system"));
        assert_eq!(s.execute(command).0, 23);
        assert_eq!(
            last_record(&f)["classification"]["classification"],
            "REMOTE_CI"
        );
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn oversized_and_unreadable_configuration_never_proves_a_destination() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let s = Sandbox::new(&f);
    let remote = "[remote \"origin\"]\n url = https://github.com/example/app.git\n";
    s.config(&format!("{remote}#{}\n", "x".repeat(65_536)));
    assert_eq!(s.execute(s.command(&f, &["git", "push"])).0, 23);
    assert_eq!(
        last_record(&f)["classification"]["classification"],
        "UNKNOWN"
    );

    let config = s.repo.join(".git/config");
    let target = s.home.join("config-target");
    fs::write(&target, remote).unwrap();
    fs::remove_file(&config).unwrap();
    std::os::unix::fs::symlink(&target, &config).unwrap();
    assert_eq!(s.execute(s.command(&f, &["git", "push"])).0, 23);
    assert_eq!(
        last_record(&f)["classification"]["classification"],
        "UNKNOWN"
    );

    fs::remove_file(config).unwrap();
    s.config(remote);
    let mut command = s.command(&f, &["git", "push"]);
    command
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "remote.pushDefault");
    assert_eq!(s.execute(command).0, 23);
    assert_eq!(
        last_record(&f)["classification"]["classification"],
        "UNKNOWN"
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn gh_capture_keeps_native_execution_fail_open_when_daemon_is_unavailable() {
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    host_config(&s.home, "github.com:\n  user: fixture\n");
    s.config("[remote \"origin\"]\n url = https://github.com/example/app.git\n");
    let (code, out, err) = s.execute(s.command(&f, &["gh", "workflow", "run", "build.yml"]));
    assert_eq!(code, 23);
    assert_eq!(fs::read_to_string(&s.marker).unwrap(), "call\n");
    assert_eq!(out, format!("argc=3\narg=<workflow>\narg=<run>\narg=<build.yml>\ncwd=<{}>\nvalue=<value with spaces>\nstdin=<payload>\n", fs::canonicalize(&s.repo).unwrap().display()));
    assert!(err.ends_with("fixture-stderr\n"), "{err:?}");
    assert!(!f.socket.exists());
}

#[test]
fn same_gh_exec_is_recorded_remote_and_runs_once_without_local_reservation() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let s = Sandbox::new(&f);
    host_config(&s.home, "github.com:\n  user: fixture\n");
    s.config("[remote \"origin\"]\n url = https://github.com/example/app.git\n");
    let id = JobId::try_from("d123456789abcdef0123456789abcdef".to_owned()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let hold = JobRequest::Enqueue { job_id: id.clone(), observer: observer.clone(), classification: serde_json::from_value(json!({"classification":"BUILD","locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"})).unwrap(), lineage_context: None };
    let JobResult::Granted {
        permit_generation, ..
    } = serde_json::from_value(
        protocol::call(&f, OPERATION_TRAFFIC_JOB, json!(hold))
            .result
            .unwrap(),
    )
    .unwrap()
    else {
        panic!("local permit not granted")
    };
    let before = counts(&f);
    assert_eq!(before, (1, 0, 1, 1));
    let (code, out, err) = s.execute(s.command(&f, &["gh", "workflow", "run", "build.yml"]));
    assert_eq!(code, 23);
    assert_eq!(fs::read_to_string(&s.marker).unwrap(), "call\n");
    assert_eq!(out, format!("argc=3\narg=<workflow>\narg=<run>\narg=<build.yml>\ncwd=<{}>\nvalue=<value with spaces>\nstdin=<payload>\n", fs::canonicalize(&s.repo).unwrap().display()));
    assert_eq!(err, "fixture-stderr\n");
    assert_eq!(counts(&f), before);
    let job = last_record(&f);
    assert_eq!(
        job["classification"],
        json!({"classification":"REMOTE_CI","locality":"REMOTE","reason":"GH_WORKFLOW_RUN","uncertainty":"NONE"})
    );
    assert!(job["child"]["pid"].is_u64());
    assert_eq!(job["outcome"]["code"], 23);
    let cancel = JobRequest::Cancel {
        job_id: id,
        observer,
        permit_generation: Some(permit_generation),
    };
    assert!(matches!(
        serde_json::from_value(
            protocol::call(&f, OPERATION_TRAFFIC_JOB, json!(cancel))
                .result
                .unwrap()
        ),
        Ok(JobResult::Cancelled {})
    ));
    daemon.stop(libc::SIGTERM);
    f.report();
}
