use super::*;
#[cfg(target_os = "macos")]
use std::os::unix::fs::MetadataExt;

const AMBIGUOUS: &str = "Cursor ARC marker has ambiguous ownership; review hooks.json and remove or resolve the marked entry manually.";

fn config_path(fixture: &Fixture, scope: &str) -> PathBuf {
    let root = if scope == "user" {
        &fixture.home
    } else {
        &fixture.workspace
    };
    let dir = root.join(".cursor");
    fs::create_dir(&dir).unwrap();
    dir.join("hooks.json")
}

fn installed_fixture(scope: &str, source: &str) -> (Fixture, PathBuf, Value) {
    let fixture = Fixture::new();
    let path = config_path(&fixture, scope);
    fs::write(&path, source).unwrap();
    fixture.worker(&format!("install-{scope}"), &[]);
    let installed = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    (fixture, path, installed)
}

fn reject_ambiguous(fixture: &Fixture, path: &Path, scope: &str, settings: &Value) {
    let bytes = serde_json::to_vec_pretty(settings).unwrap();
    fs::write(path, &bytes).unwrap();
    for action in ["install", "uninstall"] {
        fixture.worker(
            &format!("{action}-{scope}"),
            &[("CURSOR_EXPECT_ERROR_MESSAGE", AMBIGUOUS)],
        );
        assert_eq!(
            fs::read(path).unwrap(),
            bytes,
            "{action} modified ambiguous settings"
        );
    }
}

#[test]
fn executable_replacement_updates_the_proven_entry_and_keeps_hook_order() {
    for (scope, source) in [
        ("user", include_str!("user.json")),
        ("workspace", include_str!("workspace.json")),
    ] {
        let (fixture, path, mut installed) = installed_fixture(scope, source);
        let original: Value = serde_json::from_str(source).unwrap();
        let unrelated = json!({"command": "/usr/bin/printf unrelated", "matcher": "Shell"});
        installed["hooks"]["preToolUse"]
            .as_array_mut()
            .unwrap()
            .push(unrelated.clone());
        fs::write(&path, serde_json::to_vec(&installed).unwrap()).unwrap();
        let new_arcctl = fixture.bin.join("replacement arc'ctl");
        write_executable(&new_arcctl, "#!/bin/sh\nexit 0\n");
        let extra = [("CURSOR_TEST_ARC", new_arcctl.to_str().unwrap())];
        fixture.worker(&format!("install-{scope}"), &extra);
        let updated: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let entries = updated["hooks"]["preToolUse"].as_array().unwrap();
        assert_eq!(
            &entries[..2],
            original["hooks"]["preToolUse"].as_array().unwrap()
        );
        assert_eq!(entries.len(), 4);
        assert_eq!(
            entries[2],
            json!({
                "command": format!("'{}' hooks run cursor # ARC_CURSOR_HOOK_V1", new_arcctl.to_str().unwrap().replace('\'', "'\\''")),
                "matcher": "^Shell$"
            })
        );
        assert_eq!(entries[3], unrelated);
        fixture.worker(&format!("install-{scope}"), &extra);
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
            updated
        );
        let absent = fixture.bin.join("already removed");
        fixture.worker(
            &format!("uninstall-{scope}"),
            &[("CURSOR_TEST_ARC", absent.to_str().unwrap())],
        );
        let mut expected = original;
        expected["hooks"]["preToolUse"]
            .as_array_mut()
            .unwrap()
            .push(unrelated);
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
            expected
        );
        fixture.worker(&format!("uninstall-{scope}"), &[]);
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
            expected
        );
    }
}

#[test]
fn unproven_matching_handlers_and_duplicate_copies_are_ambiguous() {
    for (scope, source) in [
        ("user", include_str!("user.json")),
        ("workspace", include_str!("workspace.json")),
    ] {
        let fixture = Fixture::new();
        let path = config_path(&fixture, scope);
        let mut unproven: Value = serde_json::from_str(source).unwrap();
        unproven["hooks"]["preToolUse"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "command": "'/usr/bin/printf' hooks run cursor # ARC_CURSOR_HOOK_V1",
                "matcher": "^Shell$"
            }));
        reject_ambiguous(&fixture, &path, scope, &unproven);

        fs::write(&path, source).unwrap();
        fixture.worker(&format!("install-{scope}"), &[]);
        let installed: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        for changed in [
            json!({
                "command": "'/usr/bin/printf' hooks run cursor # ARC_CURSOR_HOOK_V1",
                "matcher": "^Shell$"
            }),
            json!({
                "command": installed["hooks"]["preToolUse"][2]["command"],
                "matcher": "^Shell$",
                "timeout": 23
            }),
        ] {
            let mut replaced = installed.clone();
            replaced["hooks"]["preToolUse"][2] = changed;
            reject_ambiguous(&fixture, &path, scope, &replaced);
        }
        let mut duplicate = installed.clone();
        duplicate["hooks"]["preToolUse"]
            .as_array_mut()
            .unwrap()
            .push(installed["hooks"]["preToolUse"][2].clone());
        reject_ambiguous(&fixture, &path, scope, &duplicate);

        // A missing ownership record cannot turn an otherwise plausible entry
        // into evidence that the installer wrote it.
        fs::write(&path, serde_json::to_vec(&installed).unwrap()).unwrap();
        fs::write(path.parent().unwrap().join(".hooks.json.arc.lock"), b"").unwrap();
        reject_ambiguous(&fixture, &path, scope, &installed);
    }
}

#[test]
fn external_entry_removal_clears_stale_ownership() {
    for (scope, source) in [
        ("user", include_str!("user.json")),
        ("workspace", include_str!("workspace.json")),
    ] {
        for remove_file in [false, true] {
            let (fixture, path, installed) = installed_fixture(scope, source);
            if remove_file {
                fs::remove_file(&path).unwrap();
            } else {
                fs::write(&path, source).unwrap();
            }
            fixture.worker(&format!("uninstall-{scope}"), &[]);
            reject_ambiguous(&fixture, &path, scope, &installed);
        }
    }
}

#[test]
fn copied_commands_with_distinct_metadata_are_ambiguous_and_untouched() {
    for (scope, source) in [
        ("user", include_str!("user.json")),
        ("workspace", include_str!("workspace.json")),
    ] {
        let (fixture, path, installed) = installed_fixture(scope, source);
        let entry = installed["hooks"]["preToolUse"][2].clone();
        for (field, value) in [
            ("timeout", json!(23)),
            ("matcher", json!("Shell|Read")),
            ("failClosed", json!(true)),
            ("custom_option", json!({"keep": true})),
            ("type", json!("command")),
        ] {
            let mut settings = installed.clone();
            let mut copied = entry.clone();
            copied[field] = value;
            settings["hooks"]["preToolUse"]
                .as_array_mut()
                .unwrap()
                .push(copied);
            reject_ambiguous(&fixture, &path, scope, &settings);
        }
        for suffix in [
            "  annotation",
            "\tannotation",
            "\n/usr/bin/printf another-command",
        ] {
            let mut settings = installed.clone();
            let mut copied = entry.clone();
            copied["command"] = json!(format!("{}{suffix}", entry["command"].as_str().unwrap()));
            settings["hooks"]["preToolUse"]
                .as_array_mut()
                .unwrap()
                .push(copied);
            reject_ambiguous(&fixture, &path, scope, &settings);
        }
    }
}

#[test]
fn exact_markers_anywhere_outside_owned_entries_stop_without_changes() {
    for (scope, source) in [
        ("user", include_str!("user.json")),
        ("workspace", include_str!("workspace.json")),
    ] {
        let (fixture, path, installed) = installed_fixture(scope, source);
        let entry = installed["hooks"]["preToolUse"][2].clone();
        let cases = [
            json!({"version": 1, "hooks": {"postToolUse": [entry.clone()]}}),
            json!({"version": 1, "hooks": {"sessionStart": [{"nested": [entry.clone()]}]}}),
            json!({"version": 1, "metadata": {"saved": entry.clone()}}),
            json!({"version": 1, "hooks": {"preToolUse": [{"command": "ARC_CURSOR_HOOK_V1"}]}}),
            json!({"version": 1, "hooks": {"preToolUse": "ARC_CURSOR_HOOK_V1"}}),
            json!({"version": 1, "ARC_CURSOR_HOOK_V1": {"keep": true}}),
        ];
        for settings in &cases {
            reject_ambiguous(&fixture, &path, scope, settings);
        }
    }
}

#[test]
fn prefix_lookalikes_survive_install_and_uninstall() {
    for (scope, source) in [
        ("user", include_str!("user.json")),
        ("workspace", include_str!("workspace.json")),
    ] {
        let fixture = Fixture::new();
        let path = config_path(&fixture, scope);
        let mut original: Value = serde_json::from_str(source).unwrap();
        for marker in [
            "ARC_CURSOR_HOOK_V12",
            "ARC_CURSOR_HOOK_V1_backup",
            "XARC_CURSOR_HOOK_V1",
        ] {
            original["hooks"]["preToolUse"]
                .as_array_mut()
                .unwrap()
                .push(json!({
                    "command": format!(
                        "'{}' hooks run cursor # {marker}",
                        fixture.arcctl.to_str().unwrap().replace('\'', "'\\''")
                    ),
                    "matcher": "^Shell$"
                }));
        }
        original["metadata"] = json!({"ARC_CURSOR_HOOK_V1_backup": "ARC_CURSOR_HOOK_V12"});
        fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
        fixture.worker(&format!("install-{scope}"), &[]);
        fixture.worker(&format!("install-{scope}"), &[]);
        fixture.worker(&format!("uninstall-{scope}"), &[]);
        fixture.worker(&format!("uninstall-{scope}"), &[]);
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
            original
        );
    }
}

#[test]
fn missing_version_is_added_for_install_and_not_required_to_uninstall() {
    for (scope, source) in [
        ("user", include_str!("user.json")),
        ("workspace", include_str!("workspace.json")),
    ] {
        let (fixture, path, installed) = installed_fixture(scope, source);
        let mut missing = installed.clone();
        missing.as_object_mut().unwrap().remove("version");
        fs::write(&path, serde_json::to_vec(&missing).unwrap()).unwrap();
        fixture.worker(&format!("install-{scope}"), &[]);
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
            installed
        );
        fs::write(&path, serde_json::to_vec(&missing).unwrap()).unwrap();
        fixture.worker(&format!("uninstall-{scope}"), &[]);
        let mut expected: Value = serde_json::from_str(source).unwrap();
        expected.as_object_mut().unwrap().remove("version");
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
            expected
        );
    }
}

#[test]
fn restrictive_umask_keeps_installer_reopenable_in_both_scopes() {
    for (scope, source) in [
        ("user", include_str!("user.json")),
        ("workspace", include_str!("workspace.json")),
    ] {
        let fixture = Fixture::new();
        let path = config_path(&fixture, scope);
        fs::write(&path, source).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        for action in ["install", "install", "uninstall", "uninstall"] {
            fixture.worker(
                &format!("{action}-{scope}"),
                &[("CURSOR_TEST_RESTRICTIVE_UMASK", "1")],
            );
        }
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
            serde_json::from_str::<Value>(source).unwrap()
        );
        assert_eq!(
            fs::metadata(path.parent().unwrap().join(".hooks.json.arc.lock"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
}

#[cfg(target_os = "macos")]
fn acl(path: &Path) -> String {
    let output = process::output(
        Command::new("/bin/ls")
            .env_clear()
            .env("LC_ALL", "C")
            .arg("-lde")
            .arg(path),
        "ACL inspection exit",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
#[cfg(target_os = "macos")]
fn installer_preserves_file_owner_group_mode_and_macos_acl() {
    for (scope, source) in [
        ("user", include_str!("user.json")),
        ("workspace", include_str!("workspace.json")),
    ] {
        let fixture = Fixture::new();
        let path = config_path(&fixture, scope);
        fs::write(&path, source).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let output = process::output(
            Command::new("/bin/chmod")
                .env_clear()
                .env("LC_ALL", "C")
                .args(["+a", "everyone allow read"])
                .arg(&path),
            "ACL setup exit",
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let expected_acl = acl(&path);
        assert!(
            expected_acl.contains("everyone allow read"),
            "{expected_acl}"
        );
        let original = fs::metadata(&path).unwrap();
        for action in [format!("install-{scope}"), format!("uninstall-{scope}")] {
            fixture.worker(&action, &[]);
            let actual = fs::metadata(&path).unwrap();
            assert_eq!(
                (actual.uid(), actual.gid(), actual.mode()),
                (original.uid(), original.gid(), original.mode())
            );
            assert_eq!(acl(&path), expected_acl);
        }
    }
}
