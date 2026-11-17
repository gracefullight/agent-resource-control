//! Recovery from missing, copied, and ambiguous ownership records.
use super::{claude, Fixture, HookContext, InstallAction, Scope};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn paths(fixture: &Fixture, scope: Scope) -> (PathBuf, Option<&Path>) {
    match scope {
        Scope::User => (fixture.home.join(".claude/settings.json"), None),
        Scope::Workspace => (
            fixture.workspace.join(".claude/settings.json"),
            Some(&fixture.workspace),
        ),
    }
}

fn sidecar(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".arc-claude-id");
    name.into()
}

fn expect_recovery_error(fixture: &Fixture, scope: Scope, path: &Path, workspace: Option<&Path>) {
    let settings = fs::read(path).unwrap();
    let identity = fs::read(sidecar(path)).ok();
    for action in [InstallAction::Install, InstallAction::Uninstall] {
        let error = claude::configure(action, scope, workspace, &fixture.context()).unwrap_err();
        assert!(
            error.contains("restore") && error.contains("manually remove"),
            "{error}"
        );
        assert_eq!(fs::read(path).unwrap(), settings);
        assert_eq!(fs::read(sidecar(path)).ok(), identity);
    }
}

pub(super) fn recovery(fixture: &Fixture, scope: Scope) {
    let (path, workspace) = paths(fixture, scope);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut baseline: Value = serde_json::from_str(include_str!("settings.json")).unwrap();
    baseline["hooks"]["PreToolUse"].as_array_mut().unwrap().push(json!({
        "matcher": "Bash", "hooks": [{"type": "command", "command": "'/bin/echo' hooks run claude # ARC Claude Code hook"}]
    }));
    fs::write(&path, serde_json::to_vec(&baseline).unwrap()).unwrap();
    let context = fixture.context();
    claude::configure(InstallAction::Install, scope, workspace, &context).unwrap();
    let installed = fs::read(&path).unwrap();
    let correct = fs::read(sidecar(&path)).unwrap();
    fs::remove_file(sidecar(&path)).unwrap();
    expect_recovery_error(fixture, scope, &path, workspace);

    // A real sidecar from another installation cannot claim this hook.
    let other_workspace = fixture.root.join("other workspace");
    fs::create_dir_all(&other_workspace).unwrap();
    claude::configure(
        InstallAction::Install,
        Scope::Workspace,
        Some(&other_workspace),
        &context,
    )
    .unwrap();
    let other_path = other_workspace.join(".claude/settings.json");
    fs::copy(sidecar(&other_path), sidecar(&path)).unwrap();
    assert_ne!(fs::read(sidecar(&path)).unwrap(), correct);
    expect_recovery_error(fixture, scope, &path, workspace);

    fs::write(sidecar(&path), &correct).unwrap();
    claude::configure(InstallAction::Install, scope, workspace, &context).unwrap();
    assert_eq!(fs::read(&path).unwrap(), installed);
    claude::configure(InstallAction::Uninstall, scope, workspace, &context).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
        baseline
    );
    let once = fs::read(&path).unwrap();
    claude::configure(InstallAction::Uninstall, scope, workspace, &context).unwrap();
    assert_eq!(fs::read(&path).unwrap(), once);
}

pub(super) fn ambiguous_lookalike(fixture: &Fixture, scope: Scope) {
    let (path, workspace) = paths(fixture, scope);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let baseline = json!({"hooks": {"PreToolUse": [
        {"matcher":"Bash", "hooks":[{"type":"command", "command": "'/bin/echo' hooks run claude # ARC Claude Code hook id=0123456789abcdef0123456789abcdef", "userMetadata": true}]}
    ]}});
    fs::write(&path, serde_json::to_vec(&baseline).unwrap()).unwrap();
    expect_recovery_error(fixture, scope, &path, workspace);
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
        baseline
    );
}

pub(super) fn extended_command(fixture: &Fixture, scope: Scope) {
    let (path, workspace) = paths(fixture, scope);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, include_bytes!("settings.json")).unwrap();
    let context = fixture.context();
    claude::configure(InstallAction::Install, scope, workspace, &context).unwrap();
    let installed = fs::read(&path).unwrap();
    let sidecar_bytes = fs::read(sidecar(&path)).unwrap();

    for suffix in ["-user", "_user", "9", "a"] {
        for placement in ["replace", "copy", "other-event", "key"] {
            for executable in ["recorded", "unrecorded"] {
                let mut settings: Value = serde_json::from_slice(&installed).unwrap();
                let group = settings["hooks"]["PreToolUse"]
                    .as_array()
                    .unwrap()
                    .last()
                    .unwrap()
                    .clone();
                let command = group["hooks"][0]["command"].as_str().unwrap();
                let command = if executable == "recorded" {
                    command.to_owned()
                } else {
                    let (_, marker) = command.split_once("# ARC Claude Code hook id=").unwrap();
                    format!(
                        "'/unrecorded/arcctl' hooks run claude # ARC Claude Code hook id={marker}"
                    )
                };
                let extended = format!("{command}{suffix}");
                match placement {
                    "replace" => {
                        settings["hooks"]["PreToolUse"]
                            .as_array_mut()
                            .unwrap()
                            .last_mut()
                            .unwrap()["hooks"][0]["command"] = json!(extended);
                    }
                    "copy" => settings["hooks"]["PreToolUse"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"matcher": "Bash|Read", "hooks": [
                            {"type": "command", "command": extended, "userField": true}
                        ]})),
                    "other-event" => {
                        settings["hooks"]["PostToolUse"] = json!([{"hooks": [
                            {"type": "command", "command": extended}
                        ]}]);
                    }
                    "key" => {
                        let mut object = serde_json::Map::new();
                        object.insert(extended, json!("user value"));
                        settings["future"] = Value::Object(object);
                    }
                    _ => unreachable!(),
                }
                fs::write(&path, serde_json::to_vec(&settings).unwrap()).unwrap();
                for present in [false, true] {
                    if present {
                        fs::write(sidecar(&path), &sidecar_bytes).unwrap();
                    } else {
                        let _ = fs::remove_file(sidecar(&path));
                    }
                    expect_recovery_error(fixture, scope, &path, workspace);
                }
            }
        }
    }
    fs::write(&path, &installed).unwrap();
    assert_eq!(fs::read(sidecar(&path)).unwrap(), sidecar_bytes);
    // A relocated executable leaves both previous and current commands in
    // the identity; an extended copy of the old command is still ambiguous.
    let relocated = fixture.bin.join("relocated arcctl");
    fs::copy(&fixture.executable, &relocated).unwrap();
    let moved_context = HookContext {
        executable: Some(relocated),
    };
    claude::configure(InstallAction::Install, scope, workspace, &moved_context).unwrap();
    let moved_install = fs::read(&path).unwrap();
    let original: Value = serde_json::from_slice(&installed).unwrap();
    let original_command = original["hooks"]["PreToolUse"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["hooks"][0]["command"]
        .as_str()
        .unwrap();
    let mut settings: Value = serde_json::from_slice(&moved_install).unwrap();
    settings["hooks"]["PostToolUse"] = json!([{"hooks": [
        {"type": "command", "command": format!("{original_command}-user")}
    ]}]);
    fs::write(&path, serde_json::to_vec(&settings).unwrap()).unwrap();
    expect_recovery_error(fixture, scope, &path, workspace);
    fs::write(&path, moved_install).unwrap();
    claude::configure(InstallAction::Uninstall, scope, workspace, &context).unwrap();
}

pub(super) fn moved_markers(fixture: &Fixture, scope: Scope) {
    let (path, workspace) = paths(fixture, scope);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, include_bytes!("settings.json")).unwrap();
    let context = fixture.context();
    claude::configure(InstallAction::Install, scope, workspace, &context).unwrap();
    let installed = fs::read(&path).unwrap();
    let identity = fs::read(sidecar(&path)).unwrap();
    for case in [
        "omitted",
        "wildcard",
        "alternative",
        "read",
        "other-event",
        "elsewhere",
        "handler-metadata",
        "group-metadata",
        "altered-command",
        "trailing-text",
        "trailing-punctuation",
        "multiple-markers",
        "marked-key",
    ] {
        let mut settings: Value = serde_json::from_slice(&installed).unwrap();
        let group = settings["hooks"]["PreToolUse"]
            .as_array_mut()
            .unwrap()
            .pop()
            .unwrap();
        let mut moved = group.clone();
        match case {
            "omitted" => {
                moved.as_object_mut().unwrap().remove("matcher");
            }
            "wildcard" => moved["matcher"] = json!("*"),
            "alternative" => moved["matcher"] = json!("Bash|Read"),
            "read" => moved["matcher"] = json!("Read"),
            "handler-metadata" => moved["hooks"][0]["if"] = json!("user condition"),
            "group-metadata" => moved["userMetadata"] = json!("keep"),
            "altered-command" => {
                let original = moved["hooks"][0]["command"].as_str().unwrap();
                let marker = original.split_once("# ARC Claude Code hook id=").unwrap().1;
                moved["hooks"][0]["command"] =
                    json!(format!("echo # ARC Claude Code hook id={marker}"));
            }
            "trailing-text" => {
                let original = moved["hooks"][0]["command"].as_str().unwrap();
                moved["hooks"][0]["command"] = json!(format!("{original} user text"));
            }
            "trailing-punctuation" => {
                let original = moved["hooks"][0]["command"].as_str().unwrap();
                moved["hooks"][0]["command"] = json!(format!("{original}; user text"));
            }
            "multiple-markers" => {
                let original = moved["hooks"][0]["command"].as_str().unwrap();
                moved["hooks"][0]["command"] =
                    json!(format!("{original} # ARC Claude Code hook id=not-an-id"));
            }
            _ => {}
        }
        match case {
            "other-event" => settings["hooks"]["PostToolUse"] = json!([moved]),
            "elsewhere" => settings["future"]["entry"] = moved["hooks"][0].clone(),
            "marked-key" => {
                let key = moved["hooks"][0]["command"].as_str().unwrap();
                let mut future = serde_json::Map::new();
                future.insert(key.to_owned(), json!("keep"));
                settings["future"] = Value::Object(future);
            }
            _ => settings["hooks"]["PreToolUse"]
                .as_array_mut()
                .unwrap()
                .push(moved),
        }
        fs::write(&path, serde_json::to_vec(&settings).unwrap()).unwrap();
        for present in [false, true] {
            if present {
                fs::write(sidecar(&path), &identity).unwrap();
            } else {
                let _ = fs::remove_file(sidecar(&path));
            }
            expect_recovery_error(fixture, scope, &path, workspace);
        }
    }
    fs::write(&path, &installed).unwrap();
    fs::write(sidecar(&path), &identity).unwrap();
    claude::configure(InstallAction::Uninstall, scope, workspace, &context).unwrap();
}
