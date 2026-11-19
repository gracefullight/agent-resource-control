use super::*;

#[test]
fn creation_after_last_comparison_is_never_overwritten() {
    let root = std::env::temp_dir().join(format!(
        "arc-cursor-absent-race-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&root).unwrap();
    let path = root.join("hooks.json");
    let temp = root.join("prepared");
    fs::write(&temp, b"arc update").unwrap();
    snapshot::check(&path, None).unwrap();
    fs::write(&path, b"other writer").unwrap();
    let error = publish(&temp, &path, false, remove_temporary).unwrap_err();
    assert!(error.contains("changed during"), "{error}");
    assert_eq!(fs::read(&path).unwrap(), b"other writer");
    assert_eq!(fs::read(&temp).unwrap(), b"arc update");
    fs::remove_dir_all(root).unwrap();
}

fn entry() -> Value {
    json!({
        "command": "'/tmp/arcctl' hooks run cursor # ARC_CURSOR_HOOK_V1",
        "matcher": "^Shell$"
    })
}

fn root(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "arc-cursor-{label}-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&path).unwrap();
    path
}

#[test]
fn failed_temporary_unlink_records_owner_for_retry_and_uninstall() {
    let dir = root("unlink-failure");
    let path = dir.join("hooks.json");
    let hook = entry();
    let error = update_config_with(
        InstallAction::Install,
        &dir,
        &path,
        Some(&hook),
        || {},
        || {},
        UpdateInterceptors {
            after_owner_snapshot: || {},
            remove_temporary: |_: &Path| Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            write_owner: lock::write_owner,
            after_publish: || Ok(()),
        },
    )
    .unwrap_err();
    assert!(
        error.contains("temporary file could not be removed"),
        "{error}"
    );
    let published = fs::read(&path).unwrap();
    let installed: Value = serde_json::from_slice(&published).unwrap();
    assert_eq!(installed["hooks"]["preToolUse"], json!([hook]));
    let record: Value =
        serde_json::from_slice(&fs::read(dir.join(".hooks.json.arc.lock")).unwrap()).unwrap();
    assert_eq!(record, hook);
    assert_eq!(
        fs::read_dir(&dir)
            .unwrap()
            .filter(|item| item
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp"))
            .count(),
        1
    );
    update_config(
        InstallAction::Install,
        &dir,
        &path,
        Some(&hook),
        || {},
        || {},
    )
    .unwrap();
    assert_eq!(fs::read(&path).unwrap(), published);
    update_config(InstallAction::Uninstall, &dir, &path, None, || {}, || {}).unwrap();
    let uninstalled: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(uninstalled["hooks"]["preToolUse"], json!([]));
    assert!(fs::read(dir.join(".hooks.json.arc.lock"))
        .unwrap()
        .is_empty());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn owner_edit_after_snapshot_is_preserved_before_settings_or_record_writes() {
    for scenario in ["owned-entry", "stale-owner", "missing-settings"] {
        let dir = root(scenario);
        let path = dir.join("hooks.json");
        let hook = entry();
        update_config(
            InstallAction::Install,
            &dir,
            &path,
            Some(&hook),
            || {},
            || {},
        )
        .unwrap();
        match scenario {
            "stale-owner" => fs::write(&path, b"{\"unrelated\":true}").unwrap(),
            "missing-settings" => fs::remove_file(&path).unwrap(),
            _ => {}
        }
        let before = fs::read(&path).ok();
        let lock_path = dir.join(".hooks.json.arc.lock");
        let error = update_config_with(
            InstallAction::Uninstall,
            &dir,
            &path,
            None,
            || {},
            || {},
            UpdateInterceptors {
                after_owner_snapshot: || fs::write(&lock_path, b"intervening owner edit").unwrap(),
                remove_temporary,
                write_owner: lock::write_owner,
                after_publish: || Ok(()),
            },
        )
        .unwrap_err();
        assert_eq!(error, AMBIGUOUS, "{scenario}");
        assert_eq!(fs::read(&lock_path).unwrap(), b"intervening owner edit");
        assert_eq!(fs::read(&path).ok(), before, "{scenario}");
        fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn valid_owner_edit_after_decision_is_not_accepted_as_the_baseline() {
    let dir = root("owner-after-decision");
    let path = dir.join("hooks.json");
    let original = entry();
    update_config(
        InstallAction::Install,
        &dir,
        &path,
        Some(&original),
        || {},
        || {},
    )
    .unwrap();
    let settings = fs::read(&path).unwrap();
    let lock_path = dir.join(".hooks.json.arc.lock");
    let intervening = json!({"command":"'/tmp/arcctl-intervening' hooks run cursor # ARC_CURSOR_HOOK_V1", "matcher":"^Shell$"});
    let intervening = serde_json::to_vec(&intervening).unwrap();
    let replacement = json!({"command":"'/tmp/arcctl-new' hooks run cursor # ARC_CURSOR_HOOK_V1", "matcher":"^Shell$"});
    let error = update_config_with(
        InstallAction::Install,
        &dir,
        &path,
        Some(&replacement),
        || {},
        || fs::write(&lock_path, &intervening).unwrap(),
        UpdateInterceptors {
            after_owner_snapshot: || {},
            remove_temporary,
            write_owner: lock::write_owner,
            after_publish: || Ok(()),
        },
    )
    .unwrap_err();
    assert_eq!(error, AMBIGUOUS);
    assert_eq!(fs::read(&path).unwrap(), settings);
    assert_eq!(fs::read(&lock_path).unwrap(), intervening);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_ownership_write_requires_manual_resolution_before_retry() {
    for partial in [false, true] {
        let dir = root("interrupted-owner");
        let path = dir.join("hooks.json");
        let lock_path = dir.join(".hooks.json.arc.lock");
        let original = json!({"version":1,"unrelated":{"keep":true},
            "hooks":{"preToolUse":[{"command":"echo keep","matcher":"Shell"}]}});
        fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
        let hook = entry();
        let error = update_config_with(
            InstallAction::Install,
            &dir,
            &path,
            Some(&hook),
            || {},
            || {},
            UpdateInterceptors {
                after_owner_snapshot: || {},
                remove_temporary,
                write_owner: |file: &mut fs::File, bytes: &[u8]| {
                    if partial {
                        file.write_all(&bytes[..bytes.len() / 2]).unwrap();
                    }
                    Err("injected owner write failure")
                },
                after_publish: || Ok(()),
            },
        )
        .unwrap_err();
        for name in [".cursor/hooks.json", ".cursor/.hooks.json.arc.lock", MARKER] {
            assert!(error.contains(name), "{error}");
        }
        let settings = fs::read(&path).unwrap();
        let owner = fs::read(&lock_path).unwrap();
        for action in [InstallAction::Install, InstallAction::Uninstall] {
            assert_eq!(
                update_config(action, &dir, &path, Some(&hook), || {}, || {}),
                Err(AMBIGUOUS)
            );
            assert_eq!(fs::read(&path).unwrap(), settings);
            assert_eq!(fs::read(&lock_path).unwrap(), owner);
        }

        // Simulate a human resolving the two files after identifying the
        // installed entry; the installer itself does not repair either file.
        let mut resolved: Value = serde_json::from_slice(&settings).unwrap();
        resolved["hooks"]["preToolUse"]
            .as_array_mut()
            .unwrap()
            .retain(|candidate| candidate != &hook);
        assert_eq!(resolved, original);
        fs::write(&path, serde_json::to_vec(&resolved).unwrap()).unwrap();
        fs::write(&lock_path, b"").unwrap();
        update_config(
            InstallAction::Install,
            &dir,
            &path,
            Some(&hook),
            || {},
            || {},
        )
        .unwrap();
        update_config(InstallAction::Uninstall, &dir, &path, None, || {}, || {}).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
            original
        );
        fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn owner_edit_after_publication_is_not_overwritten() {
    let dir = root("owner-after-publication");
    let path = dir.join("hooks.json");
    let lock_path = dir.join(".hooks.json.arc.lock");
    fs::write(&path, b"{\"unrelated\":true}").unwrap();
    let hook = entry();
    let result = update_config_with(
        InstallAction::Install,
        &dir,
        &path,
        Some(&hook),
        || {},
        || {},
        UpdateInterceptors {
            after_owner_snapshot: || {},
            remove_temporary,
            write_owner: lock::write_owner,
            after_publish: || {
                fs::write(&lock_path, b"intervening owner edit").unwrap();
                Ok(())
            },
        },
    );
    assert_eq!(result, Err(AMBIGUOUS));
    assert_eq!(fs::read(&lock_path).unwrap(), b"intervening owner edit");
    let settings: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(settings["unrelated"], true);
    assert_eq!(settings["hooks"]["preToolUse"], json!([hook]));
    fs::remove_dir_all(dir).unwrap();
}
