use super::*;

#[test]
fn creation_after_last_comparison_is_never_overwritten() {
    let root = std::env::temp_dir().join(format!(
        "arc-claude-absent-race-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&root).unwrap();
    let path = root.join("settings.json");
    let snapshot = Snapshot::read(&path).unwrap();
    let error = snapshot
        .write_with_intercept(
            &path,
            b"arc update",
            || Ok(()),
            || {
                fs::write(&path, b"other writer").unwrap();
            },
        )
        .unwrap_err();
    assert!(error.contains("changed during"), "{error}");
    assert_eq!(fs::read(&path).unwrap(), b"other writer");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn replacement_after_last_comparison_is_restored() {
    let root = std::env::temp_dir().join(format!(
        "arc-claude-swap-race-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&root).unwrap();
    let path = root.join("settings.json");
    fs::write(&path, b"original").unwrap();
    let snapshot = Snapshot::read(&path).unwrap();
    let error = snapshot
        .write_with_intercept(
            &path,
            b"arc update",
            || Ok(()),
            || {
                let other = root.join("other");
                fs::write(&other, b"other writer").unwrap();
                fs::rename(other, &path).unwrap();
            },
        )
        .unwrap_err();
    assert!(error.contains("changed during"), "{error}");
    assert_eq!(fs::read(&path).unwrap(), b"other writer");
    fs::remove_dir_all(root).unwrap();
}
