//! Lane C cases of the named target: root participation, proved remote
//! members of every runner and a non-ASCII `?` glob member.
use crate::journey::Sandbox;
use crate::runtime::Fixture;
use crate::workspace::{classify, exec, member, members, result, Workspace, CI_A, CI_B};
use std::fs;

/// The fan-out every Bun case uses.
const FAN_OUT: &str = "bun run --filter '*' ci";

/// Root participation follows the selection and npm's configuration.
pub fn root_participation(f: &Fixture, s: &Sandbox, ws: &Workspace, compound: &[String; 4]) {
    let fan_out = FAN_OUT;
    // Root and member may both define the script. A selector that excludes
    // the root, or npm's default configuration, does not run the root's
    // script; npm configuration that includes it adds the root as `.`.
    let root_manifest = |scripts: serde_json::Value| {
        fs::write(
            ws.root.join("package.json"),
            serde_json::json!({
                "name":"root", "workspaces":["packages/*"], "scripts": scripts
            })
            .to_string(),
        )
        .unwrap();
    };
    root_manifest(serde_json::json!({"ci":"bun build x.ts"}));
    for command in [
        "bun run --filter a ci",
        "bun run --filter ./packages/a ci",
        "npm run ci -w a",
        "npm run ci -w a --no-include-workspace-root",
    ] {
        let value = classify(f, s, &ws.root, command, &[]);
        assert_eq!(
            members(&value),
            [member("a", "ci", "TEST")],
            "{command}: {value}"
        );
        assert_eq!(
            result(&value)[..3],
            ["TEST", "HOST_LOCAL", "BUN_SCRIPT_TEST"]
        );
    }
    assert_eq!(
        exec(f, s, &ws.root, "npm run ci -w a"),
        ("TEST".to_owned(), "BUN_SCRIPT_TEST".to_owned())
    );
    let root_member = || (".".to_owned(), "ci".to_owned(), "BUILD".to_owned());
    for (command, env) in [
        (
            "npm run ci -w a",
            vec![("npm_config_include_workspace_root", "true")],
        ),
        ("npm run ci -w a --include-workspace-root", vec![]),
    ] {
        let value = classify(f, s, &ws.root, command, &env);
        assert_eq!(
            members(&value),
            [root_member(), member("a", "ci", "TEST")],
            "{command}: {value}"
        );
        assert_eq!(&result(&value), compound, "{command}");
    }
    // The project's .npmrc is configuration too, and is rechecked.
    fs::write(ws.root.join(".npmrc"), "include-workspace-root=true\n").unwrap();
    let configured = classify(f, s, &ws.root, "npm run ci --workspaces", &[]);
    assert_eq!(
        members(&configured),
        [
            root_member(),
            member("a", "ci", "TEST"),
            member("b", "ci", "BUILD")
        ],
        "{configured}"
    );
    assert_eq!(exec(f, s, &ws.root, "npm run ci -w a").0, "UNKNOWN");
    // Configuration that changes the selection is ambiguous.
    fs::write(ws.root.join(".npmrc"), "workspace=b\n").unwrap();
    let ambiguous = classify(f, s, &ws.root, "npm run ci -w a", &[]);
    assert_eq!(result(&ambiguous)[0], "UNKNOWN", "{ambiguous}");
    assert_eq!(result(&ambiguous)[3], "OBSERVE");
    fs::remove_file(ws.root.join(".npmrc")).unwrap();
    // A root whose script agrees keeps one class.
    root_manifest(serde_json::json!({"ci":"bun test"}));
    let agreeing = classify(
        f,
        s,
        &ws.root,
        "npm run ci -w a --include-workspace-root",
        &[],
    );
    assert_eq!(result(&agreeing)[..2], ["TEST", "HOST_LOCAL"], "{agreeing}");
    assert_eq!(members(&agreeing).len(), 2);
    // Bun's `*` matches the root's name: whether the root runs is not
    // established, so the fan-out is observed.
    let star = classify(f, s, &ws.root, fan_out, &[]);
    assert_eq!(result(&star)[0], "UNKNOWN", "{star}");
    assert_eq!(result(&star)[3], "OBSERVE");
    root_manifest(serde_json::json!({}));
}

/// Every runner form and repository spelling that proves a remote member.
pub fn remote_members(f: &Fixture, s: &Sandbox, ws: &Workspace) {
    // Every runner form and repository spelling that proves the member's
    // endpoint keeps the local-plus-remote result and each member's locality.
    for body in [
        "gh workflow run ci.yml --repo=github.com/example/project",
        "gh workflow run --ref main -R github.com/example/project ci.yml",
    ] {
        ws.member("b", &[("ci", body)]);
        for command in [
            "npm run ci --workspaces",
            "npm run ci -w a -w b",
            "pnpm -r run ci",
            "yarn workspaces foreach -A run ci",
            "bun run --filter '*' ci",
        ] {
            let value = classify(f, s, &ws.root, command, &[]);
            assert_eq!(
                result(&value),
                ["MIXED", "MIXED", "COMPOUND_MIXED", "NONE"],
                "{body}: {command}: {value}"
            );
            assert_eq!(value["analysis"]["members"][0]["locality"], "HOST_LOCAL");
            assert_eq!(value["analysis"]["members"][1]["locality"], "REMOTE");
        }
        assert_eq!(
            exec(f, s, &ws.root, "npm run ci --workspaces").0,
            "MIXED",
            "{body}"
        );
    }
    // Without an explicit repository the endpoint follows the member's Git
    // remotes, which the caller's facts do not describe: never admitted.
    ws.member("b", &[("ci", "gh workflow run ci.yml")]);
    let implicit = classify(f, s, &ws.root, "npm run ci --workspaces", &[]);
    assert_eq!(result(&implicit)[0], "UNKNOWN", "{implicit}");
    assert_eq!(result(&implicit)[3], "OBSERVE");
    assert_eq!(exec(f, s, &ws.root, "npm run ci --workspaces").0, "UNKNOWN");
    ws.member(
        "b",
        &[(
            "ci",
            "gh workflow run ci.yml --repo github.com/example/project",
        )],
    );
}

/// `?` matches one character of any width.
pub fn unicode_glob(f: &Fixture, s: &Sandbox, compound: &[String; 4]) {
    let fan_out = FAN_OUT;
    // `?` names one character, whatever its UTF-8 width: an ASCII TEST
    // member and a Hangul BUILD member are both classified, and their two
    // local classes stay observed; `ab` is not matched.
    let unicode = Workspace::new(s, "unicode", &[]);
    fs::write(
        unicode.root.join("package.json"),
        serde_json::json!({"name":"root", "workspaces":["packages/?"]}).to_string(),
    )
    .unwrap();
    unicode.member("a", &[CI_A]);
    unicode.member("한", &[CI_B]);
    unicode.member("ab", &[CI_A]);
    let narrow = classify(f, s, &unicode.root, fan_out, &[]);
    assert_eq!(
        members(&narrow),
        [member("a", "ci", "TEST"), member("한", "ci", "BUILD")],
        "{narrow}"
    );
    assert_eq!(&result(&narrow), compound);
    assert_eq!(
        exec(f, s, &unicode.root, fan_out),
        ("UNKNOWN".to_owned(), "UNRESOLVED_COMPOUND".to_owned())
    );
}
