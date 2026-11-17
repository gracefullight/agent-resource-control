//! Isolated Claude settings scenarios.
use super::{claude, Fixture, HookContext, InstallAction, Scope};
use serde_json::{json, Value};
use std::{fs, os::unix::fs::PermissionsExt, path::Path};

pub(super) fn installer_case(fixture: &Fixture, scope: Scope, custom: bool, symlink: bool) {
    let (directory, workspace) = match scope {
        Scope::User if custom => (&fixture.custom, None),
        Scope::User => (&fixture.home, None),
        Scope::Workspace => (&fixture.workspace, Some(fixture.workspace.as_path())),
    };
    let path = if custom {
        directory.join("settings.json")
    } else {
        directory.join(".claude/settings.json")
    };
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut baseline: Value = serde_json::from_str(include_str!("settings.json")).unwrap();
    let unmarked = format!(
        "'{}' hooks run claude",
        fixture.executable.to_str().unwrap().replace('\'', "'\\''")
    );
    // An identical unmarked command belongs to the user and is never removed.
    baseline["hooks"]["PreToolUse"]
        .as_array_mut()
        .unwrap()
        .push(json!({"matcher": "Bash", "hooks": [{"type": "command", "command": unmarked}]}));
    let target = if symlink {
        let target = fixture.root.join("linked-settings.json");
        fs::write(&target, serde_json::to_vec(&baseline).unwrap()).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        target
    } else {
        fs::write(&path, serde_json::to_vec(&baseline).unwrap()).unwrap();
        path.clone()
    };
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let context = fixture.context();
    claude::configure(InstallAction::Install, scope, workspace, &context).unwrap();
    let id = identity(&target);
    let bytes = fs::read(&path).unwrap();
    let installed: Value = serde_json::from_slice(&bytes).unwrap();
    let mut expected = baseline.clone();
    let command = format!(
        "'{}' hooks run claude # ARC Claude Code hook id={id}",
        fixture.executable.to_str().unwrap().replace('\'', "'\\''")
    );
    expected["hooks"]["PreToolUse"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "matcher": "Bash", "hooks": [{"type": "command", "command": command}]
        }));
    assert_eq!(installed, expected);
    if symlink {
        assert!(fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read(&target).unwrap(), bytes);
    }
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    claude::configure(InstallAction::Install, scope, workspace, &context).unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    // A move of arcctl replaces the managed entry in place rather than
    // appending a second hook or disturbing any surrounding user hooks.
    let updated_executable = fixture.bin.join("updated arcctl");
    fs::copy(&fixture.executable, &updated_executable).unwrap();
    let updated_context = HookContext {
        executable: Some(updated_executable.clone()),
    };
    claude::configure(InstallAction::Install, scope, workspace, &updated_context).unwrap();
    let updated_command = format!(
        "'{}' hooks run claude # ARC Claude Code hook id={id}",
        updated_executable.to_str().unwrap().replace('\'', "'\\''")
    );
    expected["hooks"]["PreToolUse"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap()["hooks"][0]["command"] = json!(updated_command);
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
        expected
    );
    // Removal uses the ARC marker and works even if the prior executable is
    // no longer available.
    claude::configure(
        InstallAction::Uninstall,
        scope,
        workspace,
        &HookContext { executable: None },
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
        baseline
    );
    if symlink {
        assert!(fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink());
    }
    let removed = fs::read(&path).unwrap();
    claude::configure(InstallAction::Uninstall, scope, workspace, &context).unwrap();
    assert_eq!(fs::read(&path).unwrap(), removed);
    if custom {
        assert!(!fixture.home.join(".claude/settings.json").exists());
    }
}

// A copied marker with user metadata cannot be distinguished by command alone.
// Both operations must leave every entry intact and ask for manual recovery.
pub(super) fn mixed_case(fixture: &Fixture, scope: Scope, reinstall: bool) {
    let (directory, workspace) = match scope {
        Scope::User => (&fixture.home, None),
        Scope::Workspace => (&fixture.workspace, Some(fixture.workspace.as_path())),
    };
    let path = directory.join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let baseline: Value = serde_json::from_str(include_str!("settings.json")).unwrap();
    fs::write(&path, serde_json::to_vec(&baseline).unwrap()).unwrap();
    let context = fixture.context();
    claude::configure(InstallAction::Install, scope, workspace, &context).unwrap();

    let mut mixed: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let groups = mixed["hooks"]["PreToolUse"].as_array_mut().unwrap();
    let mut marked = groups.last().unwrap()["hooks"][0].clone();
    marked["if"] = json!("user condition");
    marked["futureHandlerField"] = json!({"keep": true});
    groups.push(json!({
        "matcher": "Bash", "metadata": "preserve",
        "hooks": [{"type": "command", "command": "before"}, marked,
                  {"type": "command", "command": "after"}]
    }));
    fs::write(&path, serde_json::to_vec(&mixed).unwrap()).unwrap();
    let before = fs::read(&path).unwrap();
    let id = identity(&path);
    let actions = if reinstall {
        [InstallAction::Install, InstallAction::Uninstall]
    } else {
        [InstallAction::Uninstall, InstallAction::Install]
    };
    for action in actions {
        let error = claude::configure(action, scope, workspace, &context).unwrap_err();
        assert!(error.contains("manually remove"), "{error}");
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(identity(&path), id);
    }
    mixed["hooks"]["PreToolUse"].as_array_mut().unwrap().pop();
    fs::write(&path, serde_json::to_vec(&mixed).unwrap()).unwrap();
    claude::configure(InstallAction::Uninstall, scope, workspace, &context).unwrap();
    assert_eq!(read_settings(&path), baseline);
}

fn read_settings(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

pub(super) fn marker_in_executable_path(fixture: &Fixture, scope: Scope) {
    let (directory, workspace) = match scope {
        Scope::User => (&fixture.home, None),
        Scope::Workspace => (&fixture.workspace, Some(fixture.workspace.as_path())),
    };
    let path = directory.join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let baseline: Value = serde_json::from_str(include_str!("settings.json")).unwrap();
    fs::write(&path, serde_json::to_vec(&baseline).unwrap()).unwrap();
    let bin = fixture
        .root
        .join("bin # ARC Claude Code hook id=0123456789abcdef0123456789abcdef");
    fs::create_dir_all(&bin).unwrap();
    let executable = bin.join("arcctl");
    fs::copy(&fixture.executable, &executable).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let context = HookContext {
        executable: Some(executable),
    };
    claude::configure(InstallAction::Install, scope, workspace, &context).unwrap();
    let installed = fs::read(&path).unwrap();
    claude::configure(InstallAction::Install, scope, workspace, &context).unwrap();
    assert_eq!(fs::read(&path).unwrap(), installed);
    claude::configure(InstallAction::Uninstall, scope, workspace, &context).unwrap();
    assert_eq!(read_settings(&path), baseline);
}

pub(super) fn authored_commands(fixture: &Fixture, scope: Scope) {
    let (directory, workspace) = match scope {
        Scope::User => (&fixture.home, None),
        Scope::Workspace => (&fixture.workspace, Some(fixture.workspace.as_path())),
    };
    let path = directory.join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let suffix = " hooks run claude # ARC Claude Code hook";
    let authored = [
        format!("'/bin/echo'{suffix}"),
        format!("'/bin/echo'{suffix} id=0123456789abcdef0123456789abcdeZ"),
        format!("'/usr/bin/logger' && '/usr/local/bin/arcctl'{suffix}"),
        format!("'/usr/bin/logger' ; '/usr/local/bin/arcctl'{suffix}"),
        format!("'/tools/it'\\''s logger' && '/usr/local/bin/arcctl'{suffix}"),
        format!("'/usr/local/bin/arcctl' ''{suffix}"),
        format!("'relative/arcctl'{suffix}"),
    ];
    let handlers: Vec<Value> = authored
        .iter()
        .enumerate()
        .map(|(index, command)| json!({"type": "command", "command": command, "userField": index}))
        .collect();
    let mut baseline: Value = serde_json::from_str(include_str!("settings.json")).unwrap();
    baseline["hooks"]["PreToolUse"]
        .as_array_mut()
        .unwrap()
        .push(json!({"matcher": "Bash", "hooks": handlers}));
    baseline["hooks"]["PreToolUse"]
        .as_array_mut()
        .unwrap()
        .push(json!({"matcher": "Read", "hooks": [
            {"type": "command", "command": authored[0]}
        ]}));
    fs::write(&path, serde_json::to_vec(&baseline).unwrap()).unwrap();

    let context = fixture.context();
    claude::configure(InstallAction::Install, scope, workspace, &context).unwrap();
    let mut installed = read_settings(&path);
    let mut without_managed = installed.clone();
    without_managed["hooks"]["PreToolUse"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert_eq!(
        without_managed, baseline,
        "initial install changed user hooks"
    );

    let groups = installed["hooks"]["PreToolUse"].as_array_mut().unwrap();
    let group_index = groups.len() - 3;
    let mixed = groups[group_index]["hooks"].as_array_mut().unwrap();
    let impostor = format!("'/bin/echo'{suffix} id={}-user", identity(&path));
    mixed.push(json!({"type": "command", "command": impostor}));
    groups.push(json!({"matcher": "Read", "hooks": [
        {"type": "command", "command": authored[0], "userField": "other matcher"},
        {"type": "command", "command": impostor}
    ]}));
    fs::write(&path, serde_json::to_vec(&installed).unwrap()).unwrap();

    // A copied identity with a different executable or suffix is ambiguous,
    // even when the original owned entry is also present. Neither action may
    // silently discard it or change the settings while ownership is unclear.
    let ambiguous = fs::read(&path).unwrap();
    let original_id = identity(&path);
    for action in [InstallAction::Install, InstallAction::Uninstall] {
        let error = claude::configure(action, scope, workspace, &context).unwrap_err();
        assert!(error.contains("manually remove"), "{error}");
        assert_eq!(fs::read(&path).unwrap(), ambiguous);
        assert_eq!(identity(&path), original_id);
    }
    let groups = installed["hooks"]["PreToolUse"].as_array_mut().unwrap();
    groups.pop();
    groups[group_index]["hooks"].as_array_mut().unwrap().pop();
    fs::write(&path, serde_json::to_vec(&installed).unwrap()).unwrap();

    let relocated = fixture.bin.join("arcctl''relocated");
    fs::copy(&fixture.executable, &relocated).unwrap();
    let updated_context = HookContext {
        executable: Some(relocated.clone()),
    };
    let mut expected = installed.clone();
    let managed_index = expected["hooks"]["PreToolUse"].as_array().unwrap().len() - 1;
    expected["hooks"]["PreToolUse"][managed_index]["hooks"][0]["command"] = json!(format!(
        "'{}'{suffix} id={}",
        relocated.to_str().unwrap().replace('\'', "'\\''"),
        identity(&path)
    ));
    claude::configure(InstallAction::Install, scope, workspace, &updated_context).unwrap();
    assert_eq!(
        read_settings(&path),
        expected,
        "reinstall changed user hooks"
    );
    let once = fs::read(&path).unwrap();
    claude::configure(InstallAction::Install, scope, workspace, &updated_context).unwrap();
    assert_eq!(fs::read(&path).unwrap(), once);

    expected["hooks"]["PreToolUse"]
        .as_array_mut()
        .unwrap()
        .remove(managed_index);
    claude::configure(
        InstallAction::Uninstall,
        scope,
        workspace,
        &HookContext { executable: None },
    )
    .unwrap();
    assert_eq!(
        read_settings(&path),
        expected,
        "uninstall changed user hooks"
    );
    let once = fs::read(&path).unwrap();
    claude::configure(InstallAction::Uninstall, scope, workspace, &context).unwrap();
    assert_eq!(fs::read(&path).unwrap(), once);
}

pub(super) fn identity(path: &Path) -> String {
    let path = fs::canonicalize(path).unwrap();
    let mut name = path.as_os_str().to_os_string();
    name.push(".arc-claude-id");
    let record: Value = serde_json::from_slice(&fs::read(name).unwrap()).unwrap();
    record["id"].as_str().unwrap().to_owned()
}

pub(super) fn bad_settings(fixture: &Fixture) {
    let path = fixture.home.join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let context = fixture.context();
    for bytes in [
        b"{broken".as_slice(),
        b"[]",
        b"{\"hooks\":[]}",
        b"{\"hooks\":{\"PreToolUse\":{}}}",
        b"{\"number\":01}",
        b"{\"number\":1e+}",
        b"{1:2}",
        b"{\"duplicate\":1,\"duplicate\":2}",
    ] {
        fs::write(&path, bytes).unwrap();
        assert!(claude::configure(InstallAction::Install, Scope::User, None, &context).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    assert!(claude::configure(
        InstallAction::Install,
        Scope::User,
        Some(Path::new("/tmp")),
        &context
    )
    .is_err());
    assert!(claude::configure(
        InstallAction::Install,
        Scope::Workspace,
        Some(Path::new("relative")),
        &context
    )
    .is_err());
    assert!(claude::configure(
        InstallAction::Install,
        Scope::User,
        None,
        &HookContext { executable: None }
    )
    .is_err());
}

pub(super) fn missing_settings(fixture: &Fixture) {
    let path = fixture.home.join(".claude/settings.json");
    let context = fixture.context();
    claude::configure(InstallAction::Uninstall, Scope::User, None, &context).unwrap();
    assert!(!path.exists());
    claude::configure(InstallAction::Install, Scope::User, None, &context).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    claude::configure(InstallAction::Uninstall, Scope::User, None, &context).unwrap();
    assert!(
        serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap()["hooks"]["PreToolUse"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
