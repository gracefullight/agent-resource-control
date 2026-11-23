//! Workspace fan-out and the project analysis cache through built `arcctl
//! classify` and `arcctl exec` against a real daemon (lane C: DET-009 item
//! 2, DET-014 items 1 and 2). Classification executes nothing; tools are
//! offline stubs. Every wait is on observed state; the 60 s deadlines are
//! watchdogs only.
#[path = "detector_workspace/cases.rs"]
mod cases;
#[path = "detector_workspace/directories.rs"]
mod directories;
#[path = "support/journey.rs"]
mod journey;
#[path = "detector_workspace/legacy.rs"]
mod legacy;
#[path = "detector_workspace/native_bytes.rs"]
mod native_bytes;
#[path = "detector_workspace/review.rs"]
mod review;
#[allow(dead_code)] // The shared fixture also serves unrelated workloads.
#[path = "support/runtime.rs"]
mod runtime;
mod support;
#[path = "detector_workspace/workspace.rs"]
mod workspace;

use journey::{output_with_input, Sandbox};
use runtime::Fixture;
use std::{fs, os::unix::fs::PermissionsExt};
use workspace::{cache, cache_dir, classify, exec, member, members, result, Workspace, CI_A, CI_B};

/// A single-package `bun run test` keeps today's resolution on the exec
/// path: its script body `bun test` is resolved from the package manifest
/// and the job is recorded as TEST with reason BUN_SCRIPT_TEST.
#[test]
fn seam_single_package_bun_script_is_resolved_on_the_exec_path() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    std::fs::write(
        project.join("package.json"),
        r#"{"name":"single","scripts":{"test":"bun test"}}"#,
    )
    .unwrap();
    s.stub("bun");
    let mut daemon = f.spawn_isolated("");

    let mut exec = s.arcctl(&f, &project);
    exec.args(["exec", "--", "bun", "run", "test"])
        .env("ARC_FIXTURE_MODE", "echo");
    let output = output_with_input(&mut exec, b"input\n", "arcctl exec");
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    let (class, reason): (String, String) = journey::database(&f)
        .query_row(
            "SELECT j.job_class,l.classification_reason FROM jobs j
             JOIN job_lifecycle l ON l.job_id=j.id",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        (class.as_str(), reason.as_str()),
        ("TEST", "BUN_SCRIPT_TEST")
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}

/// DET-009 item 2 and DET-014 items 1 and 2 through built `arcctl classify`
/// and `arcctl exec` against a real daemon. A two-member Bun workspace (`a`
/// runs a test, `b` a build under the same script name) is classified member
/// by member; both paths share the user-owned or in-memory cache,
/// reused while every read file and the glob membership are unchanged, and
/// invalidated by an edited script, a replaced manifest and an added or
/// removed member. Every package-manager form, a selector excluding a member
/// and the UNKNOWN/OBSERVE boundaries are table cases, as are pnpm script
/// shorthand, root participation by selection and npm configuration
/// (including the global file a user `.npmrc` names and file keys as
/// written, with values decoded as npm decodes them), one project entry
/// across the workspace root, a member directory and a member subdirectory
/// with per-directory results whichever populates it first, including the
/// discovery membership and declaration that selected it, proved remote
/// members of every runner and a `?` glob over a non-ASCII member.
/// Runner stubs record any execution; classification runs none.
#[test]
fn workspace_fanout_and_cache_follow_production_paths() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    for runner in ["bun", "npm", "pnpm", "yarn"] {
        s.stub(runner);
    }
    let ws = Workspace::new(
        &s,
        "workspace",
        &[
            ("a", &[CI_A, ("test", "bun test")]),
            ("b", &[CI_B, ("test", "vitest run")]),
        ],
    );
    let mut daemon = f.spawn_isolated("");
    let fan_out = "bun run --filter '*' ci";
    let both = vec![member("a", "ci", "TEST"), member("b", "ci", "BUILD")];
    let compound = [
        "UNKNOWN".to_owned(),
        "UNKNOWN".to_owned(),
        "UNRESOLVED_COMPOUND".to_owned(),
        "OBSERVE".to_owned(),
    ];

    // Exec negotiates the complete member graph, while the existing job
    // record uses its conservative observation projection.
    let recorded = review::compound_exec(&f, &s, &ws.root, fan_out);
    let first = classify(&f, &s, &ws.root, fan_out, &[]);
    assert_eq!(members(&first), both, "{first}");
    assert_eq!(result(&first), compound);
    assert_eq!(first["analysis"]["compound"], recorded);
    assert_eq!(cache(&first), "hit", "exec and classify share the analysis");
    let repeated = classify(&f, &s, &ws.root, fan_out, &[]);
    assert_eq!(repeated, first, "a repeated classification is identical");
    legacy::unsupported_compound(&f, &s, &ws.root, fan_out, &both, &compound);
    // The project's analysis, not one invocation's, is reused: another
    // selector and another runner reading the same declaration are hits.
    for (command, expected) in [
        ("bun run --filter a ci", vec![member("a", "ci", "TEST")]),
        ("npm run ci -w b", vec![member("b", "ci", "BUILD")]),
        ("yarn workspaces foreach -A run ci", both.clone()),
    ] {
        let value = classify(&f, &s, &ws.root, command, &[]);
        assert_eq!(cache(&value), "hit", "{command}: {value}");
        assert_eq!(members(&value), expected, "{command}");
    }
    // pnpm declares its members in pnpm-workspace.yaml, which the root's
    // discovery already read and validated into the same project entry.
    assert_eq!(
        cache(&classify(&f, &s, &ws.root, "pnpm -r run ci", &[])),
        "hit"
    );
    assert_eq!(
        cache(&classify(&f, &s, &ws.root, "pnpm --filter a run ci", &[])),
        "hit"
    );

    // Identical bytes with a new mtime keep the result.
    for path in [ws.manifest_path("a"), ws.root.join("package.json")] {
        ws.touch_identical(&path);
        let same = classify(&f, &s, &ws.root, fan_out, &[]);
        assert_eq!(same, first, "{}", path.display());
    }
    // An edited member script is a miss with the new result.
    ws.member("b", &[("ci", "bun test")]);
    let edited = classify(&f, &s, &ws.root, fan_out, &[]);
    assert_eq!(cache(&edited), "miss", "{edited}");
    assert_eq!(
        members(&edited),
        [member("a", "ci", "TEST"), member("b", "ci", "TEST")]
    );
    assert_eq!(
        result(&edited)[..3],
        ["TEST", "HOST_LOCAL", "BUN_SCRIPT_TEST"]
    );
    assert_eq!(cache(&classify(&f, &s, &ws.root, fan_out, &[])), "hit");
    let other = classify(&f, &s, &ws.root, "npm run ci -w b", &[]);
    assert_eq!(cache(&other), "hit", "{other}");
    assert_eq!(members(&other), [member("b", "ci", "TEST")]);
    // A replaced manifest (new inode, new content) is a miss.
    let path = ws.manifest_path("a");
    ws.replace(
        &path,
        &workspace::manifest("a", &[("ci", "bun build x.ts")]),
    );
    let replaced = classify(&f, &s, &ws.root, fan_out, &[]);
    assert_eq!(cache(&replaced), "miss", "{replaced}");
    assert_eq!(
        members(&replaced),
        [member("a", "ci", "BUILD"), member("b", "ci", "TEST")]
    );
    assert_eq!(result(&replaced), compound);
    // Glob membership is re-evaluated: an added and a removed member.
    ws.member("c", &[("ci", "bun test")]);
    let added = classify(&f, &s, &ws.root, fan_out, &[]);
    assert_eq!(cache(&added), "miss", "{added}");
    assert_eq!(members(&added).len(), 3, "{added}");
    fs::remove_dir_all(ws.root.join("packages/c")).unwrap();
    let removed = classify(&f, &s, &ws.root, fan_out, &[]);
    assert_eq!(cache(&removed), "miss", "{removed}");
    assert_eq!(members(&removed), members(&replaced));
    // Restore the TEST/BUILD members for the table cases.
    ws.member(
        "a",
        &[CI_A, ("test", "bun test"), ("verify:ci", "bun test")],
    );
    ws.member(
        "b",
        &[
            CI_B,
            ("test", "vitest run"),
            ("verify:ci", "bun build x.ts"),
        ],
    );

    // Every requested form fans out over the same members.
    for command in [
        "bun run --filter '*' ci",
        "bun --filter './packages/*' ci",
        "npm run ci --workspaces",
        "npm run ci -w a -w b",
        "pnpm -r run ci",
        "pnpm --filter a --filter b run ci",
        "yarn workspaces foreach -A run ci",
    ] {
        let value = classify(&f, &s, &ws.root, command, &[]);
        assert_eq!(members(&value), both, "{command}: {value}");
        assert_eq!(result(&value), compound, "{command}");
        assert_eq!(
            value["analysis"]["compound"]["kind"], "WORKSPACE_COMPOUND",
            "{command}: {value}"
        );
        assert_eq!(
            value["analysis"]["compound"]["members"][0]["classification"]["classification"], "TEST",
            "{command}: {value}"
        );
        assert_eq!(
            value["analysis"]["compound"]["members"][1]["classification"]["classification"],
            "BUILD",
            "{command}: {value}"
        );
    }
    // pnpm resolves a declared script name that is not a built-in command.
    let shorthand = classify(&f, &s, &ws.root, "pnpm -r verify:ci", &[]);
    assert_eq!(
        members(&shorthand),
        [
            member("a", "verify:ci", "TEST"),
            member("b", "verify:ci", "BUILD")
        ],
        "{shorthand}"
    );
    assert_eq!(result(&shorthand), compound);
    let filtered = classify(&f, &s, &ws.root, "pnpm --filter a verify:ci", &[]);
    assert_eq!(
        members(&filtered),
        [member("a", "verify:ci", "TEST")],
        "{filtered}"
    );
    assert_eq!(
        result(&filtered)[..3],
        ["TEST", "HOST_LOCAL", "BUN_SCRIPT_TEST"]
    );
    assert_eq!(
        exec(&f, &s, &ws.root, "pnpm --filter a verify:ci"),
        ("TEST".to_owned(), "BUN_SCRIPT_TEST".to_owned())
    );
    // A selector that excludes a member classifies only the selected one.
    for command in [
        "bun run --filter a ci",
        "npm run ci -w a",
        "pnpm --filter '!b' run ci",
        "yarn workspaces foreach -A --exclude b run ci",
    ] {
        let value = classify(&f, &s, &ws.root, command, &[]);
        assert_eq!(members(&value), [member("a", "ci", "TEST")], "{command}");
        assert_eq!(
            result(&value)[..3],
            ["TEST", "HOST_LOCAL", "BUN_SCRIPT_TEST"]
        );
    }
    // arcctl exec admits each form whose members agree as that class.
    for command in [
        "bun run --filter '*' test",
        "npm run test --workspaces",
        "npm run test -w b",
        "pnpm -r run test",
        "pnpm -r test",
        "pnpm --filter a run test",
        "pnpm --filter a test",
        "yarn workspaces foreach -A run test",
    ] {
        assert_eq!(
            exec(&f, &s, &ws.root, command),
            ("TEST".to_owned(), "BUN_SCRIPT_TEST".to_owned()),
            "{command}"
        );
    }

    cases::root_participation(&f, &s, &ws, &compound);
    directories::npm_configuration(&f, &s, &ws, &compound);
    directories::directories(
        &f,
        &s,
        &ws,
        &[CI_A, ("test", "bun test"), ("verify:ci", "bun test")],
    );
    directories::ordinary_first(&f, &s);
    directories::non_utf8_directory(&f, &s);

    workspace::ordinary_cache_reuse(&f, &s);

    // Recursive globs visit bounded matching directories. Adding and then
    // removing a deeper package invalidates the membership.
    fs::write(
        ws.root.join("package.json"),
        serde_json::json!({"name":"root", "workspaces":["packages/**"]}).to_string(),
    )
    .unwrap();
    let recursive = classify(&f, &s, &ws.root, fan_out, &[]);
    assert_eq!(members(&recursive), both);
    assert_eq!(cache(&classify(&f, &s, &ws.root, fan_out, &[])), "hit");
    let nested = ws.root.join("packages/group/nested");
    fs::create_dir_all(&nested).unwrap();
    fs::write(
        nested.join("package.json"),
        workspace::manifest("nested", &[CI_A]),
    )
    .unwrap();
    let added = classify(&f, &s, &ws.root, fan_out, &[]);
    assert_eq!(cache(&added), "miss", "{added}");
    assert_eq!(members(&added).len(), 3);
    fs::remove_dir_all(ws.root.join("packages/group")).unwrap();
    let removed = classify(&f, &s, &ws.root, fan_out, &[]);
    assert_eq!(cache(&removed), "miss", "{removed}");
    assert_eq!(members(&removed), both);
    fs::write(
        ws.root.join("package.json"),
        serde_json::json!({"name":"root", "workspaces":["packages/*"]}).to_string(),
    )
    .unwrap();

    // The explicitly hosted remote member is checked against the isolated gh
    // configuration, independently of the root repository. Execution keeps
    // the original fan-out argv and the two members retain their locality.
    ws.member(
        "b",
        &[(
            "ci",
            "gh workflow run ci.yml --repo github.com/example/project",
        )],
    );
    let mixed = classify(&f, &s, &ws.root, fan_out, &[]);
    assert_eq!(result(&mixed), ["MIXED", "MIXED", "COMPOUND_MIXED", "NONE"]);
    assert_eq!(
        members(&mixed),
        [member("a", "ci", "TEST"), member("b", "ci", "REMOTE_CI")]
    );
    assert_eq!(mixed["analysis"]["members"][0]["locality"], "HOST_LOCAL");
    assert_eq!(mixed["analysis"]["members"][1]["locality"], "REMOTE");
    assert_eq!(exec(&f, &s, &ws.root, fan_out).0, "MIXED");
    cases::remote_members(&f, &s, &ws);
    // A configured transport override invalidates that proof, even after the
    // successful read; no stale remote result may be reused.
    fs::write(
        s.home.join("gh/config.yml"),
        "http_unix_socket: /unproved\n",
    )
    .unwrap();
    let unproved = classify(&f, &s, &ws.root, fan_out, &[]);
    assert_eq!(result(&unproved)[0], "UNKNOWN", "{unproved}");
    assert_eq!(result(&unproved)[3], "OBSERVE");
    fs::remove_file(s.home.join("gh/config.yml")).unwrap();
    ws.member("b", &[CI_B, ("test", "vitest run")]);

    cases::unicode_glob(&f, &s, &compound);

    // Dynamic, unresolved and over-limit graphs are UNKNOWN/OBSERVE.
    let dynamic = Workspace::new(&s, "dynamic", &[("a", &[("ci", "bun test $ARGS")])]);
    let wide_members: Vec<String> = (0..17).map(|at| format!("m{at:02}")).collect();
    let wide = Workspace::new(&s, "wide", &[]);
    for name in &wide_members {
        wide.member(name, &[CI_A]);
    }
    let inside = ws.root.join("packages/a");
    for (cwd, command) in [
        (&ws.root, "pnpm --filter '...a' run ci"),
        (&ws.root, "pnpm -r ci"),
        (&ws.root, "yarn workspaces foreach -R run ci"),
        (&ws.root, "npm run test -w nobody"),
        (&inside, "bun run --filter '*' ci"),
        (&dynamic.root, "bun run --filter '*' ci"),
        (&wide.root, "bun run --filter '*' ci"),
    ] {
        let value = classify(&f, &s, cwd, command, &[]);
        assert_eq!(result(&value)[0], "UNKNOWN", "{command}: {value}");
        assert_eq!(result(&value)[3], "OBSERVE", "{command}");
        assert!(value["analysis"].is_object(), "{command}: {value}");
        assert!(
            value["analysis"].get("compound").is_none(),
            "{command}: {value}"
        );
    }
    // Classification never ran a runner stub.
    assert_eq!(journey::starts(&workspace::marker(&s)), 0);

    // The disk cache is bounded and owner-only, with no absolute path or
    // script body. Database overrides do not select its location.
    let dir = cache_dir(&s);
    let mode = fs::metadata(&dir).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700, "{}", dir.display());
    let entries: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert!(!entries.is_empty() && entries.len() <= 64, "{entries:?}");
    for entry in &entries {
        let text = fs::read_to_string(entry).unwrap();
        assert!(!text.contains(ws.root.to_str().unwrap()), "{text}");
        assert!(
            !text.contains("bun build") && !text.contains("vitest"),
            "{text}"
        );
    }
    review::cache_overrides(&f, &s, fan_out, &both);
    // Ordinary output keeps its four fields.
    let mut plain = s.arcctl(&f, &ws.root);
    plain.args(["classify", "--json", "--", fan_out]);
    let output = output_with_input(&mut plain, b"", "arcctl classify");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 4, "{value}");
    assert_eq!(result(&value), compound);

    daemon.stop(libc::SIGTERM);
    f.report();
}
