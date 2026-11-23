//! Lane C cases of the named target: one project entry across the workspace
//! root, a member directory and a member subdirectory, whichever populates
//! it first, results kept apart by invocation directory, and npm's
//! effective configuration files as npm decodes them, through built
//! `arcctl classify` and `arcctl exec`.
use crate::journey::Sandbox;
use crate::runtime::Fixture;
use crate::workspace::{
    cache, classify, exec, exec_with_env, manifest, member, members, result, Workspace, CI_A, CI_B,
};
use std::{ffi::OsStr, fs, io::Write, os::unix::ffi::OsStrExt};

fn local(class: &str, reason: &str) -> [String; 4] {
    [class, "HOST_LOCAL", reason, "NONE"].map(str::to_owned)
}

/// A workspace selector at the root and ordinary scripts in a member and its
/// subdirectory share one validated project entry; each directory keeps its
/// own results, and every change is a miss with the new result.
/// `scripts` restores member `a` afterwards.
pub fn directories(f: &Fixture, s: &Sandbox, ws: &Workspace, scripts: &[(&str, &str)]) {
    let a = ws.root.join("packages/a");
    let nested = a.join("src/deep");
    fs::create_dir_all(&nested).unwrap();
    let test = local("TEST", "BUN_SCRIPT_TEST");
    let build = local("BUILD", "BUN_SCRIPT_BUILD");
    let selected = classify(f, s, &ws.root, "bun run --filter a ci", &[]);
    assert_eq!(
        members(&selected),
        [member("a", "ci", "TEST")],
        "{selected}"
    );
    // The member's manifest was read by the root's selector: the member and
    // a subdirectory without a manifest reuse that analysis.
    for cwd in [&a, &nested] {
        let value = classify(f, s, cwd, "bun run ci", &[]);
        assert_eq!(cache(&value), "hit", "{}: {value}", cwd.display());
        assert_eq!(result(&value), test, "{}", cwd.display());
        assert_eq!(
            classify(f, s, cwd, "bun run ci", &[]),
            value,
            "a repeated classification is identical"
        );
        assert_eq!(
            exec(f, s, cwd, "bun run ci"),
            ("TEST".to_owned(), "BUN_SCRIPT_TEST".to_owned())
        );
    }
    // A result never answers another directory: from the member, the
    // selector has no workspace declaration and stays observed.
    let inside = classify(f, s, &a, "bun run --filter a ci", &[]);
    assert_eq!(result(&inside)[0], "UNKNOWN", "{inside}");
    assert_eq!(result(&inside)[3], "OBSERVE");

    // An edited script is a miss with the new result in the first directory
    // that classifies, whose fresh analysis the others then reuse.
    ws.member("a", &[("ci", "bun build x.ts")]);
    let edited = classify(f, s, &nested, "bun run ci", &[]);
    assert_eq!(cache(&edited), "miss", "{edited}");
    assert_eq!(result(&edited), build);
    let reused = classify(f, s, &a, "bun run ci", &[]);
    assert_eq!((cache(&reused), result(&reused)), ("hit", build.clone()));
    assert_eq!(
        exec(f, s, &a, "bun run ci"),
        ("BUILD".to_owned(), "BUN_SCRIPT_BUILD".to_owned())
    );
    let root = classify(f, s, &ws.root, "bun run --filter a ci", &[]);
    assert_eq!(members(&root), [member("a", "ci", "BUILD")], "{root}");
    assert_eq!(result(&root), build);
    // A replaced manifest is a miss; identical bytes with a new mtime keep
    // every directory's result.
    let path = ws.manifest_path("a");
    ws.replace(&path, &manifest("a", &[("ci", "bun test")]));
    let replaced = classify(f, s, &a, "bun run ci", &[]);
    assert_eq!(
        (cache(&replaced), result(&replaced)),
        ("miss", test.clone())
    );
    let below = classify(f, s, &nested, "bun run ci", &[]);
    assert_eq!((cache(&below), result(&below)), ("hit", test.clone()));
    ws.touch_identical(&path);
    assert_eq!(classify(f, s, &nested, "bun run ci", &[]), below);
    let again = classify(f, s, &a, "bun run ci", &[]);
    assert_eq!((cache(&again), result(&again)), ("hit", test.clone()));
    // A nearer manifest created in the subdirectory is a miss there, and is
    // used by exec; removed again, the recorded workspace analysis applies.
    let src = a.join("src/package.json");
    fs::write(&src, manifest("src", &[("ci", "bun build x.ts")])).unwrap();
    let nearer = classify(f, s, &nested, "bun run ci", &[]);
    assert_eq!((cache(&nearer), result(&nearer)), ("miss", build.clone()));
    assert_eq!(exec(f, s, &nested, "bun run ci").0, "BUILD");
    fs::remove_file(&src).unwrap();
    let restored = classify(f, s, &nested, "bun run ci", &[]);
    assert_eq!((cache(&restored), result(&restored)), ("hit", test));
    assert_eq!(exec(f, s, &nested, "bun run ci").0, "TEST");
    fs::remove_dir_all(a.join("src")).unwrap();
    ws.member("a", scripts);
}

/// A root manifest declaring `workspaces` with the given globs and a BUILD
/// `ci` script of its own.
fn build_root(ws: &Workspace, globs: &[&str]) {
    fs::write(
        ws.root.join("package.json"),
        serde_json::json!({"name":"root", "workspaces": globs, "scripts":{"ci":"bun build x.ts"}})
            .to_string(),
    )
    .unwrap();
}

/// An ordinary member script populates a new project's entry before any
/// workspace selector. The discovery that selected the workspace root is
/// recorded with it: the member, a member subdirectory and the root reuse
/// the entry, each with its own result for the same command, and an added
/// or removed sibling, or an edited declaration that still names the
/// member, is a miss. A package the globs do not name is its own project.
pub fn ordinary_first(f: &Fixture, s: &Sandbox) {
    let ws = Workspace::new(s, "ordinary-first", &[("a", &[CI_A]), ("b", &[CI_B])]);
    build_root(&ws, &["packages/*"]);
    let a = ws.root.join("packages/a");
    let nested = a.join("src/deep");
    fs::create_dir_all(&nested).unwrap();
    let test = local("TEST", "BUN_SCRIPT_TEST");
    let build = local("BUILD", "BUN_SCRIPT_BUILD");
    let first = classify(f, s, &a, "bun run ci", &[]);
    assert_eq!((cache(&first), result(&first)), ("miss", test.clone()));
    let reuse = |why: &str| {
        for (cwd, expected) in [(&a, &test), (&nested, &test), (&ws.root, &build)] {
            let value = classify(f, s, cwd, "bun run ci", &[]);
            assert_eq!(
                (cache(&value), &result(&value)),
                ("hit", expected),
                "{why}: {}: {value}",
                cwd.display()
            );
        }
        assert_eq!(exec(f, s, &a, "bun run ci").0, "TEST", "{why}");
        assert_eq!(exec(f, s, &ws.root, "bun run ci").0, "BUILD", "{why}");
    };
    reuse("populated by the member");
    // Each change is a miss in the member with its unchanged result, whose
    // fresh analysis the other directories then reuse.
    let miss = |why: &str| {
        let value = classify(f, s, &a, "bun run ci", &[]);
        assert_eq!(
            (cache(&value), result(&value)),
            ("miss", test.clone()),
            "{why}: {value}"
        );
        reuse(why);
    };
    ws.member("c", &[CI_A]);
    miss("an added sibling");
    fs::remove_dir_all(ws.root.join("packages/c")).unwrap();
    miss("a removed sibling");
    build_root(&ws, &["packages/a", "packages/b"]);
    miss("an edited declaration that still names the member");
    // A package the globs do not name keeps its own entry and result.
    let tools = ws.root.join("tools");
    fs::create_dir_all(&tools).unwrap();
    fs::write(tools.join("package.json"), manifest("tools", &[CI_B])).unwrap();
    let own = classify(f, s, &tools, "bun run ci", &[]);
    assert_eq!((cache(&own), result(&own)), ("miss", build.clone()));
    let again = classify(f, s, &tools, "bun run ci", &[]);
    assert_eq!((cache(&again), result(&again)), ("hit", build.clone()));
    reuse("beside an unnamed package");
}

/// The same command from the workspace root and from a member directory
/// whose name is not UTF-8 never shares a result: such a directory is
/// classified afresh and nothing is reused or kept for it. If the native
/// filesystem rejects those names, an isolated optional volume or an
/// explicitly supplied fixture can provide a host-visible directory.
pub fn non_utf8_directory(f: &Fixture, s: &Sandbox) {
    let external = std::env::var_os("ARC_DETECTOR_NATIVE_BYTES_ROOT");
    let mut volume = None;
    let (ws, source) = if let Some(root) = &external {
        (Workspace { root: root.into() }, "supplied fixture")
    } else {
        let ws = Workspace::new(s, "bytes", &[("a", &[CI_A])]);
        build_root(&ws, &["packages/*"]);
        let dir = ws
            .root
            .join("packages/a")
            .join(OsStr::from_bytes(b"\xff\xfe"));
        match fs::create_dir(&dir) {
            Ok(()) => (ws, "native filesystem"),
            Err(error)
                if matches!(
                    error.raw_os_error(),
                    Some(libc::EPERM | libc::EILSEQ | libc::EINVAL)
                ) =>
            {
                match crate::native_bytes::Volume::create(s) {
                    Ok(fixture) => {
                        let ws = Workspace {
                            root: fixture.root().to_path_buf(),
                        };
                        volume = Some(fixture);
                        (ws, "isolated container volume")
                    }
                    Err(reason) => {
                        writeln!(
                            std::io::stderr().lock(),
                            "UNVERIFIED non-UTF-8 CLI comparison: native filesystem rejected the byte name ({error}); isolated fixture unavailable: {reason}; alternatively supply ARC_DETECTOR_NATIVE_BYTES_ROOT"
                        )
                        .expect("write fixture status");
                        return;
                    }
                }
            }
            Err(error) => panic!("{}: {error}", dir.display()),
        }
    };
    let dir = ws
        .root
        .join("packages/a")
        .join(OsStr::from_bytes(b"\xff\xfe"));
    if external.is_some() || volume.is_some() {
        // Some network filesystems require enumeration before native byte
        // names can be looked up by the host. The fixture is read-only here.
        let entries: Vec<_> = fs::read_dir(dir.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert!(entries.iter().any(|name| name.as_bytes() == b"\xff\xfe"));
        fs::read_dir(&dir).unwrap().for_each(|entry| {
            entry.unwrap();
        });
    }
    let root = classify(f, s, &ws.root, "bun run ci", &[]);
    assert_eq!(result(&root), local("BUILD", "BUN_SCRIPT_BUILD"), "{root}");
    assert_eq!(
        exec(f, s, &ws.root, "bun run ci"),
        ("BUILD".to_owned(), "BUN_SCRIPT_BUILD".to_owned())
    );
    let value = classify(f, s, &dir, "bun run ci", &[]);
    assert_eq!(result(&value), local("TEST", "BUN_SCRIPT_TEST"), "{value}");
    assert_eq!(cache(&value), "none", "{value}");
    assert_eq!(
        exec(f, s, &dir, "bun run ci"),
        ("TEST".to_owned(), "BUN_SCRIPT_TEST".to_owned())
    );
    let again = classify(f, s, &ws.root, "bun run ci", &[]);
    assert_eq!((cache(&again), result(&again)), ("hit", result(&root)));
    if let Some(fixture) = volume.take() {
        fixture
            .verify_unwind_cleanup()
            .expect("remove isolated native-byte fixture on failure");
    }
    // Bypass libtest capture so a runner log distinguishes an executed
    // comparison from an unsupported-fixture return even on a passing test.
    writeln!(
        std::io::stderr().lock(),
        "VERIFIED non-UTF-8 CLI comparison ({source}): classify root BUILD/member TEST; exec root BUILD/member TEST"
    )
    .expect("write fixture status");
}

/// npm's root participation comes from the configuration files npm itself
/// would read: the global file a user `.npmrc` names, and file keys as
/// written. In each case the selected member is TEST and the root BUILD, so
/// a TEST-only result would be the mistaken one.
pub fn npm_configuration(f: &Fixture, s: &Sandbox, ws: &Workspace, compound: &[String; 4]) {
    let root_manifest = |scripts: serde_json::Value| {
        fs::write(
            ws.root.join("package.json"),
            serde_json::json!({"name":"root", "workspaces":["packages/*"], "scripts": scripts})
                .to_string(),
        )
        .unwrap();
    };
    root_manifest(serde_json::json!({"ci":"bun build x.ts"}));
    let command = "npm run ci -w a";
    let with_root = || {
        vec![
            (".".to_owned(), "ci".to_owned(), "BUILD".to_owned()),
            member("a", "ci", "TEST"),
        ]
    };
    let included = |why: &str| {
        let value = classify(f, s, &ws.root, command, &[]);
        assert_eq!(members(&value), with_root(), "{why}: {value}");
        assert_eq!(&result(&value), compound, "{why}");
        assert_eq!(
            exec(f, s, &ws.root, command),
            ("UNKNOWN".to_owned(), "UNRESOLVED_COMPOUND".to_owned()),
            "{why}"
        );
    };
    let excluded = |why: &str| {
        let value = classify(f, s, &ws.root, command, &[]);
        assert_eq!(
            members(&value),
            [member("a", "ci", "TEST")],
            "{why}: {value}"
        );
        assert_eq!(result(&value), local("TEST", "BUN_SCRIPT_TEST"), "{why}");
        assert_eq!(
            exec(f, s, &ws.root, command),
            ("TEST".to_owned(), "BUN_SCRIPT_TEST".to_owned()),
            "{why}"
        );
    };
    let observed = |why: &str| {
        let value = classify(f, s, &ws.root, command, &[]);
        assert_eq!(result(&value)[0], "UNKNOWN", "{why}: {value}");
        assert_eq!(result(&value)[3], "OBSERVE", "{why}");
        assert_eq!(exec(f, s, &ws.root, command).0, "UNKNOWN", "{why}");
    };
    let user = s.home.join(".npmrc");
    let global = s.home.join("npm-global/npmrc");
    fs::create_dir_all(global.parent().unwrap()).unwrap();
    excluded("npm's default");
    // The user file names the global file, which includes the root.
    fs::write(&user, format!("globalconfig={}\n", global.display())).unwrap();
    fs::write(&global, "include-workspace-root=true\n").unwrap();
    included("globalconfig from the user file");
    fs::write(&user, "globalconfig=~/npm-global/npmrc\n").unwrap();
    included("a home-relative globalconfig");
    // Values are decoded as npm's ini parser decodes them. Read literally,
    // the commented and escaped locations would name absent files, and the
    // commented setting would not be a boolean.
    fs::write(
        &user,
        format!("globalconfig={} # the shared file\n", global.display()),
    )
    .unwrap();
    included("a commented globalconfig");
    fs::write(&user, format!("globalconfig=\"{}\"\n", global.display())).unwrap();
    included("a quoted globalconfig");
    // npm trims path-field values after decoding, including whitespace
    // inside quotes. The real file includes the BUILD root; a lookup of a
    // distinct trailing-space path would incorrectly produce TEST only.
    fs::write(&user, format!("globalconfig=\"{} \"\n", global.display())).unwrap();
    included("a quoted globalconfig with trailing whitespace");
    fs::remove_file(&user).unwrap();
    let global_with_space = format!("{} ", global.display());
    let env = [("npm_config_globalconfig", global_with_space.as_str())];
    let value = classify(f, s, &ws.root, command, &env);
    assert_eq!(members(&value), with_root(), "environment path: {value}");
    assert_eq!(&result(&value), compound, "environment path: {value}");
    assert_eq!(
        exec_with_env(f, s, &ws.root, command, &env),
        ("UNKNOWN".to_owned(), "UNRESOLVED_COMPOUND".to_owned()),
        "environment globalconfig with trailing whitespace"
    );
    let hashed = s.home.join("npm#global/npmrc");
    fs::create_dir_all(hashed.parent().unwrap()).unwrap();
    fs::write(&hashed, "include-workspace-root=true\n").unwrap();
    let escaped = hashed.display().to_string().replace('#', "\\#");
    fs::write(&user, format!("globalconfig={escaped}\n")).unwrap();
    included("an escaped # in globalconfig");
    fs::remove_dir_all(hashed.parent().unwrap()).unwrap();
    fs::write(&user, "include-workspace-root=true ; the root too\n").unwrap();
    included("a commented setting");
    // A dotted key that ini could nest onto the setting is not decided.
    fs::write(&user, "include-workspace-root.enabled=null\n").unwrap();
    observed("a dotted key ini could nest");
    fs::write(&user, "globalconfig=~/npm-global/npmrc\n").unwrap();
    // The global file is revalidated: a changed setting is the new result.
    fs::write(&global, "include-workspace-root=false\n").unwrap();
    excluded("a changed global file");
    // A location that cannot be established is observed.
    fs::write(&user, "globalconfig=npm-global/npmrc\n").unwrap();
    observed("a relative globalconfig");
    fs::write(
        &user,
        format!("globalconfig={}\n", global.parent().unwrap().display()),
    )
    .unwrap();
    observed("an unreadable globalconfig");
    fs::remove_dir_all(global.parent().unwrap()).unwrap();

    // The user enables root participation; a project key with underscores
    // is an unknown npm key and does not override it, while the hyphenated
    // project setting does.
    fs::write(&user, "include-workspace-root=true\n").unwrap();
    let project = ws.root.join(".npmrc");
    fs::write(&project, "include_workspace_root=false\n").unwrap();
    included("an underscored project key");
    fs::write(&project, "include-workspace-root=false\n").unwrap();
    excluded("the hyphenated project key");
    // Environment keys are normalized and outrank both files.
    fs::remove_file(&project).unwrap();
    let value = classify(
        f,
        s,
        &ws.root,
        command,
        &[("NPM_CONFIG_INCLUDE_WORKSPACE_ROOT", "false")],
    );
    assert_eq!(members(&value), [member("a", "ci", "TEST")], "{value}");
    fs::remove_file(&user).unwrap();
    root_manifest(serde_json::json!({}));
}
