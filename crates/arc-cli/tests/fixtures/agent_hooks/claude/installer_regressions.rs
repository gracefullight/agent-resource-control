//! Installer regressions for lossless settings and access metadata.
use super::{claude, Fixture, HookContext, InstallAction, Scope};
use std::{fs, os::unix::fs::MetadataExt, path::Path};

fn settings(fixture: &Fixture, scope: Scope) -> (std::path::PathBuf, Option<&Path>) {
    let directory = match scope {
        Scope::User => &fixture.home,
        Scope::Workspace => &fixture.workspace,
    };
    let path = directory.join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let workspace = match scope {
        Scope::User => None,
        Scope::Workspace => Some(fixture.workspace.as_path()),
    };
    (path, workspace)
}

pub(super) fn lossless(fixture: &Fixture, scope: Scope) {
    let (path, workspace) = settings(fixture, scope);
    let original = r#"{
      "large": 18446744073709551617,
      "decimal": -0.123456789012345678901234567890123456789,
      "hugeExponent": 1.234567890123456789e+400,
      "lookalike": "18446744073709551617",
      "hooks": {"PreToolUse": [
        {"matcher": "Read", "metadata": 987654321098765432109876543210,
         "hooks": [{"type": "command", "command": "first",
                    "metadata": 0.0000000000000000000000000000123456789}]},
        {"matcher": "Bash", "hooks": [{"type": "command", "command": "second"}]}
      ]}}
    "#;
    fs::write(&path, original).unwrap();
    let check = || {
        let text = fs::read_to_string(&path).unwrap();
        for field in [
            "\"large\": 18446744073709551617",
            "\"decimal\": -0.123456789012345678901234567890123456789",
            "\"hugeExponent\": 1.234567890123456789e+400",
            "\"metadata\": 987654321098765432109876543210",
            "\"metadata\": 0.0000000000000000000000000000123456789",
        ] {
            assert!(text.contains(field), "field {field} changed: {text}");
        }
        assert!(text.contains("\"lookalike\": \"18446744073709551617\""));
        assert!(!text.contains("__arc_json_number_"));
        assert!(text.find("\"first\"").unwrap() < text.find("\"second\"").unwrap());
    };
    let context = fixture.context();
    claude::configure(InstallAction::Install, scope, workspace, &context).unwrap();
    check();
    let installed = fs::read(&path).unwrap();
    claude::configure(InstallAction::Install, scope, workspace, &context).unwrap();
    assert_eq!(fs::read(&path).unwrap(), installed);
    check();
    let relocated = fixture.bin.join("new arcctl");
    fs::copy(&fixture.executable, &relocated).unwrap();
    let new_context = HookContext {
        executable: Some(relocated),
    };
    claude::configure(InstallAction::Install, scope, workspace, &new_context).unwrap();
    check();
    claude::configure(InstallAction::Uninstall, scope, workspace, &context).unwrap();
    check();
    assert!(!fs::read_to_string(&path)
        .unwrap()
        .contains("# ARC Claude Code hook id="));
}

#[cfg(target_os = "macos")]
pub(super) fn acl(fixture: &Fixture, scope: Scope) {
    use std::process::Command;
    let (path, workspace) = settings(fixture, scope);
    fs::write(&path, include_bytes!("settings.json")).unwrap();
    let mut command = Command::new("/usr/bin/id");
    command.args(["-un"]).env_clear();
    let user = super::process::output(
        &mut command,
        &fixture.root,
        "ACL user lookup",
        std::time::Duration::from_secs(90),
    );
    assert!(user.status.success());
    let user = std::str::from_utf8(&user.stdout).unwrap().trim();
    let mut command = Command::new("/bin/chmod");
    command
        .arg("+a")
        .arg(format!("user:{user} allow read"))
        .arg(&path)
        .env_clear();
    let output = super::process::output(
        &mut command,
        &fixture.root,
        "ACL setup",
        std::time::Duration::from_secs(90),
    );
    assert!(output.status.success());
    let access = || {
        let metadata = fs::metadata(&path).unwrap();
        let mut command = Command::new("/bin/ls");
        command.args(["-le"]).arg(&path).env_clear();
        let output = super::process::output(
            &mut command,
            &fixture.root,
            "ACL inspection",
            std::time::Duration::from_secs(90),
        );
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        let entries = text.lines().skip(1).collect::<Vec<_>>().join("\n");
        assert!(entries.contains("allow read"), "missing ACL: {text}");
        (metadata.uid(), metadata.gid(), metadata.mode(), entries)
    };
    let before = access();
    let context = fixture.context();
    claude::configure(InstallAction::Install, scope, workspace, &context).unwrap();
    assert_eq!(access(), before);
    let relocated = fixture.bin.join("moved executable");
    fs::copy(&fixture.executable, &relocated).unwrap();
    let moved = HookContext {
        executable: Some(relocated),
    };
    claude::configure(InstallAction::Install, scope, workspace, &moved).unwrap();
    assert_eq!(access(), before);
    claude::configure(InstallAction::Uninstall, scope, workspace, &context).unwrap();
    assert_eq!(access(), before);
}

#[cfg(not(target_os = "macos"))]
pub(super) fn acl(_fixture: &Fixture, _scope: Scope) {}

pub(super) fn invalid_identity(fixture: &Fixture, scope: Scope) {
    let (path, workspace) = settings(fixture, scope);
    let original = include_bytes!("settings.json");
    fs::write(&path, original).unwrap();
    let mut id_path = path.as_os_str().to_os_string();
    id_path.push(".arc-claude-id");
    fs::write(&id_path, b"unrelated file").unwrap();
    for action in [InstallAction::Install, InstallAction::Uninstall] {
        let error = claude::configure(action, scope, workspace, &fixture.context()).unwrap_err();
        assert!(
            error.contains("restore") && error.contains("manually remove"),
            "{error}"
        );
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::read(&id_path).unwrap(), b"unrelated file");
    }
    fs::remove_file(&id_path).unwrap();
    claude::configure(InstallAction::Install, scope, workspace, &fixture.context()).unwrap();
    let installed = fs::read(&path).unwrap();
    let valid_identity = fs::read(&id_path).unwrap();
    fs::write(&id_path, b"invalid identity").unwrap();
    for action in [InstallAction::Install, InstallAction::Uninstall] {
        let error = claude::configure(action, scope, workspace, &fixture.context()).unwrap_err();
        assert!(
            error.contains("restore") && error.contains("manually remove"),
            "{error}"
        );
        assert_eq!(fs::read(&path).unwrap(), installed);
        assert_eq!(fs::read(&id_path).unwrap(), b"invalid identity");
    }
    fs::write(&id_path, valid_identity).unwrap();
    claude::configure(
        InstallAction::Uninstall,
        scope,
        workspace,
        &fixture.context(),
    )
    .unwrap();
}
