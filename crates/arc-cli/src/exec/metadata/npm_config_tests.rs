//! npm configuration resolution over real files.
use super::*;

fn words(s: &str) -> Vec<String> {
    s.split(' ').map(str::to_owned).collect()
}
fn env(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    pairs.iter().map(|(k, v)| (k.into(), v.into())).collect()
}

#[test]
fn layers_follow_npm_precedence_and_selection_changes_are_ambiguous() {
    let dir = std::env::current_exe().unwrap().with_file_name(format!(
        "arc-npm-config-{}-{}",
        std::process::id(),
        module_path!().replace("::", "-")
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (root, home) = (dir.join("root"), dir.join("home"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let home_env = [("HOME", home.to_str().unwrap())];
    let include = |command: &str, pairs: &[(&str, &str)]| match resolve_with(
        &words(command),
        &root,
        env(pairs),
    ) {
        Npm::Resolved { include_root, .. } => Some(include_root),
        Npm::Ambiguous => None,
        Npm::NotApplicable => panic!("{command}"),
    };
    let fan = "npm run ci -w a";
    // Default: excluded. The rewrite makes that explicit after `run`.
    assert_eq!(include(fan, &home_env), Some(false));
    let resolved = resolve_with(&words(fan), &root, env(&home_env));
    assert_eq!(
        resolved.rewrite(&words(fan)),
        words("npm run --include-workspace-root=false ci -w a")
    );
    // A command-line flag is kept and wins over every layer.
    let explicit = words("npm run ci -w a --include-workspace-root");
    let with_env = [
        ("HOME", home.to_str().unwrap()),
        ("NPM_CONFIG_INCLUDE_WORKSPACE_ROOT", "false"),
    ];
    let resolved = resolve_with(&explicit, &root, env(&with_env));
    assert!(matches!(
        resolved,
        Npm::Resolved {
            include_root: true,
            ..
        }
    ));
    assert_eq!(resolved.rewrite(&explicit), explicit);
    // Environment, then project, user and global files.
    assert_eq!(include(fan, &with_env), Some(false));
    std::fs::write(home.join(".npmrc"), "include-workspace-root=true\n").unwrap();
    assert_eq!(include(fan, &home_env), Some(true));
    std::fs::write(
        root.join(".npmrc"),
        "; project\ninclude-workspace-root = false\n",
    )
    .unwrap();
    assert_eq!(include(fan, &home_env), Some(false));
    // npm keeps file keys as written: an underscore or another case is
    // an unknown key and cannot override the user's hyphenated setting.
    for text in [
        "include_workspace_root = false\n",
        "Include-Workspace-Root=false\n",
    ] {
        std::fs::write(root.join(".npmrc"), text).unwrap();
        assert_eq!(include(fan, &home_env), Some(true), "{text}");
    }
    // Environment keys are normalized, and outrank both files.
    let underscored = [
        ("HOME", home.to_str().unwrap()),
        ("NPM_CONFIG_INCLUDE_WORKSPACE_ROOT", "false"),
    ];
    assert_eq!(include(fan, &underscored), Some(false));
    std::fs::remove_file(root.join(".npmrc")).unwrap();
    std::fs::remove_file(home.join(".npmrc")).unwrap();
    std::fs::write(dir.join("global"), "include-workspace-root\n").unwrap();
    let global = dir.join("global");
    let global_env = [
        ("HOME", home.to_str().unwrap()),
        ("npm_config_globalconfig", global.to_str().unwrap()),
    ];
    assert_eq!(include(fan, &global_env), Some(true));
    // A changed file changes the digest.
    let before = resolve_with(&words(fan), &root, env(&global_env));
    std::fs::write(&global, "include-workspace-root=true\n").unwrap();
    assert_ne!(before, resolve_with(&words(fan), &root, env(&global_env)));
    std::fs::remove_file(&global).unwrap();
    // Ambiguous evidence.
    for text in [
        "workspace=b\n",
        "workspaces[]=a\n",
        "if-present=true\n",
        "include-workspace-root=${X}\n",
        "include-workspace-root=maybe\n",
        "[section]\ninclude-workspace-root=true\n",
        "include-workspace-root[]=true\n",
        "${KEY}=true\n",
    ] {
        std::fs::write(root.join(".npmrc"), text).unwrap();
        assert_eq!(include(fan, &home_env), None, "{text}");
    }
    std::fs::remove_file(root.join(".npmrc")).unwrap();
    for pairs in [
        &[("npm_config_workspace", "b")][..],
        &[("npm_config_include_workspace_root", "yes")][..],
        &[],
    ] {
        let mut pairs = pairs.to_vec();
        if !pairs.is_empty() {
            pairs.push(("HOME", home.to_str().unwrap()));
        }
        assert_eq!(include(fan, &pairs), None, "{pairs:?}");
    }
    // Unrelated keys and other commands are not decided here.
    std::fs::write(root.join(".npmrc"), "registry=https://example.invalid/\n").unwrap();
    assert_eq!(include(fan, &home_env), Some(false));
    for other in ["npm run ci", "bun run --filter a ci", "npm test"] {
        assert_eq!(
            resolve_with(&words(other), &root, env(&home_env)),
            Npm::NotApplicable
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn configuration_files_are_located_as_npm_locates_them() {
    let dir = std::env::current_exe().unwrap().with_file_name(format!(
        "arc-npm-locations-{}-{}",
        std::process::id(),
        module_path!().replace("::", "-")
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (root, home) = (dir.join("root"), dir.join("home"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(home.join("npm/etc")).unwrap();
    let home_env = [("HOME", home.to_str().unwrap())];
    let fan = "npm run ci -w a";
    let include = |command: &str| match resolve_with(&words(command), &root, env(&home_env)) {
        Npm::Resolved { include_root, .. } => Some(include_root),
        Npm::Ambiguous => None,
        Npm::NotApplicable => panic!("{command}"),
    };
    let global = home.join("global-npmrc");
    let write = |path: &Path, text: &str| std::fs::write(path, text).unwrap();
    // The user file names the global file, which includes the root.
    write(
        &home.join(".npmrc"),
        &format!("globalconfig={}\n", global.display()),
    );
    assert_eq!(include(fan), Some(false), "an absent global file");
    write(&global, "include-workspace-root=true\n");
    assert_eq!(include(fan), Some(true));
    // npm trims JavaScript whitespace from path fields after ini decoding,
    // including quoted values and environment values.
    write(
        &home.join(".npmrc"),
        &format!("globalconfig=\"{} \u{feff}\"\n", global.display()),
    );
    assert_eq!(include(fan), Some(true), "quoted path field");
    write(&home.join(".npmrc"), "\n");
    let padded_global = format!("{} \u{feff}", global.display());
    let global_env = [
        ("HOME", home.to_str().unwrap()),
        ("npm_config_globalconfig", padded_global.as_str()),
    ];
    assert!(matches!(
        resolve_with(&words(fan), &root, env(&global_env)),
        Npm::Resolved {
            include_root: true,
            ..
        }
    ));
    write(
        &home.join(".npmrc"),
        &format!("globalconfig={}\n", global.display()),
    );
    // The global file is revalidated through the digest.
    let before = resolve_with(&words(fan), &root, env(&home_env));
    write(&global, "include-workspace-root=false\n");
    let after = resolve_with(&words(fan), &root, env(&home_env));
    assert_ne!(before, after);
    assert!(matches!(
        after,
        Npm::Resolved {
            include_root: false,
            ..
        }
    ));
    // `~/` expands with HOME; a user prefix locates etc/npmrc.
    write(&global, "include-workspace-root=true\n");
    write(&home.join(".npmrc"), "globalconfig=~/global-npmrc\n");
    assert_eq!(include(fan), Some(true));
    write(&home.join("npm/etc/npmrc"), "include-workspace-root=true\n");
    write(&home.join(".npmrc"), "prefix=~/npm\n");
    assert_eq!(include(fan), Some(true));
    // A user setting outranks the global file it names.
    write(
        &home.join(".npmrc"),
        "prefix=~/npm\ninclude-workspace-root=false\n",
    );
    assert_eq!(include(fan), Some(false));
    // The project file can move the user file.
    let moved = home.join("moved-npmrc");
    write(&moved, "include-workspace-root=true\n");
    write(&home.join(".npmrc"), "include-workspace-root=false\n");
    write(
        &root.join(".npmrc"),
        &format!("userconfig={}\n", moved.display()),
    );
    assert_eq!(include(fan), Some(true));
    std::fs::remove_file(root.join(".npmrc")).unwrap();
    // Locations that cannot be established are ambiguous.
    for text in [
        "globalconfig=relative/npmrc\n",
        "globalconfig=${HOME}/npmrc\n",
        "globalconfig[]=/x\n",
        "globalconfig=\n",
    ] {
        write(&home.join(".npmrc"), text);
        assert_eq!(include(fan), None, "{text}");
    }
    write(
        &home.join(".npmrc"),
        &format!("globalconfig={}\n", home.join("npm").display()),
    );
    assert_eq!(include(fan), None, "a directory is unreadable");
    write(&home.join(".npmrc"), "\n");
    // The fan-out grammar refuses a location on the command line, so such
    // a command is never resolved here; the layer resolution refuses it too.
    for command in [
        "npm run ci -w a --globalconfig=/x",
        "npm run ci -w a --userconfig /x",
        "npm run ci -w a --prefix=/x",
    ] {
        assert_eq!(
            resolve_with(&words(command), &root, env(&home_env)),
            Npm::NotApplicable,
            "{command}"
        );
        assert_eq!(
            resolve_layers(None, &words(command), &root, env(&home_env)),
            Err(()),
            "{command}"
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn ini_tokens_decode_as_npm_decodes_them() {
    use ini::{decode, lines, Decoded, Line};
    let text = |s: &str| Decoded::Text(s.to_owned());
    for (raw, expected) in [
        ("/x/npmrc # comment", text("/x/npmrc")),
        ("/x/npmrc;comment", text("/x/npmrc")),
        (r"/x/npm\#rc", text("/x/npm#rc")),
        (r"a\;b\\c\d", text(r"a;b\c\d")),
        ("trailing\\", text("trailing\\")),
        ("\"/x/a b\"", text("/x/a b")),
        ("\"/x/\\u0061\"", text("/x/a")),
        ("'/x/npmrc'", text("/x/npmrc")),
        ("'\"/x\"'", text("/x")),
        ("\"/x\" ; c", text("\"/x\"")),
        ("'", text("")),
        ("\u{feff} spaced \u{3000}", text("spaced")),
        ("'true'", Decoded::Json(serde_json::Value::Bool(true))),
        ("'{}'", Decoded::Json(serde_json::json!({}))),
        ("\"\\ud800\"", Decoded::Uncertain),
        ("'1e400'", Decoded::Uncertain),
    ] {
        assert_eq!(decode(raw), expected, "{raw:?}");
    }
    let parsed: Vec<Line> =
        lines("\u{feff}k=1\r=skipped\n  # note\nflag\nv=\"false\"\nx[]=1\n[]=1\nk=a\u{2028}b")
            .collect();
    let assign = |key: &str, array: bool, value: Decoded| Line::Assign {
        key: text(key),
        array,
        value,
    };
    assert_eq!(
        parsed,
        [
            assign("k", false, text("1")),
            Line::Skip,
            Line::Skip,
            assign("flag", false, Decoded::Json(true.into())),
            assign("v", false, Decoded::Json(false.into())),
            assign("x", true, text("1")),
            // `ini` reads `[]=1` as a key; any line opening with `[` could
            // be a section, which is never decided.
            Line::Section,
            Line::Skip,
        ]
    );
    assert!(ini::nests(
        &text("a.b"),
        &Decoded::Json(serde_json::Value::Null)
    ));
    assert!(!ini::nests(&text("a.b"), &text("{}")));
    assert!(!ini::nests(&text("ab"), &Decoded::Uncertain));
}

#[test]
fn commented_quoted_and_escaped_settings_resolve_as_npm_reads_them() {
    let dir = std::env::current_exe().unwrap().with_file_name(format!(
        "arc-npm-ini-{}-{}",
        std::process::id(),
        module_path!().replace("::", "-")
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (root, home) = (dir.join("root"), dir.join("home"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(home.join("np#m")).unwrap();
    let home_env = [("HOME", home.to_str().unwrap())];
    let include = || match resolve_with(&words("npm run ci -w a"), &root, env(&home_env)) {
        Npm::Resolved { include_root, .. } => Some(include_root),
        Npm::Ambiguous => None,
        Npm::NotApplicable => panic!("not a fan-out"),
    };
    let user = home.join(".npmrc");
    let global = home.join("np#m/npmrc");
    std::fs::write(&global, "include-workspace-root=true\n").unwrap();
    let shown = global.display().to_string();
    let escaped = shown.replace('#', "\\#");
    for text in [
        // Read literally, this path would be absent: the mistaken default.
        format!("globalconfig={escaped} # the shared file\n"),
        format!("globalconfig=\"{shown}\"\n"),
        format!("globalconfig='{shown}' \n"),
        format!("globalconfig={escaped}\n"),
        format!("\"globalconfig\" = {escaped}\n"),
        "globalconfig=~/sub/../np\\#m/npmrc\n".to_owned(),
        "include-workspace-root=true ; enabled\n".to_owned(),
        "include-workspace-root ; no value is true\n".to_owned(),
        "include-workspace-root='true'\n".to_owned(),
        "include-workspace-root=true;false\n".to_owned(),
        "registry=x\rinclude-workspace-root=true\n".to_owned(),
    ] {
        std::fs::write(&user, &text).unwrap();
        assert_eq!(include(), Some(true), "{text:?}");
    }
    for text in [
        "include-workspace-root=\"false\" \n",
        "include-workspace-root;x=false\n",
        "include-workspace-root=true\ninclude-workspace-root=false\u{2028}\n",
    ] {
        std::fs::write(&user, text).unwrap();
        // The last line `ini` matches wins; a value holding U+2028 is skipped.
        let expected = text.ends_with("\u{2028}\n");
        assert_eq!(include(), Some(expected), "{text:?}");
    }
    // A literal `#` without its escape begins a comment: npm names the
    // absent `np`, not the global file that includes the root.
    std::fs::write(&user, format!("globalconfig={shown}\n")).unwrap();
    assert_eq!(include(), Some(false), "an absent global file");
    std::fs::write(home.join("np"), "include-workspace-root=true\n").unwrap();
    assert_eq!(include(), Some(true), "npm reads `np`");
    std::fs::remove_file(home.join("np")).unwrap();
    // Tokens whose decoding is not reproduced, and locations npm would
    // resolve against its working directory, are ambiguous.
    for text in [
        "globalconfig=\"\\ud800\"\n",
        "globalconfig='{}'\n",
        "globalconfig=\"/x\" ; c\n",
        "globalconfig=~\n",
        "globalconfig\n",
        "include-workspace-root.x=null\n",
        "'a.b'='{\"c\":1}'\n",
        "include-workspace-root=null\n",
        "include-workspace-root=\\true\n",
        "  [section]\n",
    ] {
        std::fs::write(&user, text).unwrap();
        assert_eq!(include(), None, "{text:?}");
    }
    // Environment values are taken exactly, never ini-decoded.
    std::fs::write(&user, "\n").unwrap();
    for (value, expected) in [("true", Some(true)), ("true # x", None), ("'true'", None)] {
        let pairs = [
            ("HOME", home.to_str().unwrap()),
            ("npm_config_include_workspace_root", value),
        ];
        let got = match resolve_with(&words("npm run ci -w a"), &root, env(&pairs)) {
            Npm::Resolved { include_root, .. } => Some(include_root),
            _ => None,
        };
        assert_eq!(got, expected, "{value:?}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
