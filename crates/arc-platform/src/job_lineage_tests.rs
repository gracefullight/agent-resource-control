use super::*;
use std::{
    os::unix::fs::{symlink, DirBuilderExt},
    process::Command,
};

struct Private(PathBuf);
impl Private {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "arc-meter-lineage-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
    fn socket(&self) -> PathBuf {
        self.0.join("ipc")
    }
}
impl Drop for Private {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn nested_native_probe() {
    let Some(socket) = std::env::var_os("ARC_METER_LINEAGE_PROBE") else {
        return;
    };
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let id = arc_core::process_identity::random_job_id().unwrap();
    let actual = relation(Path::new(&socket), &observer, &id);
    match std::env::var("ARC_METER_LINEAGE_EXPECT").unwrap().as_str() {
        "nested" => assert_eq!(
            actual,
            AccountingRelation::IncludedInRoot {
                root_job_id: "a".repeat(32)
            }
        ),
        "unknown" => assert_eq!(actual, AccountingRelation::Unverified {}),
        _ => unreachable!(),
    }
    if std::env::var_os("ARC_METER_LINEAGE_AGAIN").is_some() {
        let _record = Record::create(Path::new(&socket), &observer, &id, &actual).unwrap();
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "job_lineage::tests::nested_native_probe"])
            .env_remove("ARC_METER_LINEAGE_AGAIN")
            .status()
            .unwrap();
        assert!(status.success());
    }
}
fn nested(socket: &Path, expected: &str, again: bool) {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "job_lineage::tests::nested_native_probe"])
        .env("ARC_METER_LINEAGE_PROBE", socket)
        .env("ARC_METER_LINEAGE_EXPECT", expected);
    if again {
        command.env("ARC_METER_LINEAGE_AGAIN", "1");
    }
    let output = command.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}
#[test]
fn private_records_preserve_canonical_root_across_multiple_native_generations() {
    let root = Private::new();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let chain = ancestors(&observer).unwrap();
    assert!(!chain.is_empty());
    assert!(chain.iter().all(|p| p.pid > 1));
    assert!(chain.iter().all(|p| p.boot_id == observer.boot_id));
    let id = "a".repeat(32);
    assert_eq!(
        relation(&root.socket(), &observer, &id),
        AccountingRelation::CanonicalRoot {
            root_job_id: id.clone()
        }
    );
    let record = Record::create(
        &root.socket(),
        &observer,
        &id,
        &AccountingRelation::CanonicalRoot {
            root_job_id: id.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        read(&root.0, &observer).unwrap(),
        Some((id.clone(), Some(id)))
    );
    assert_eq!(record.file.metadata().unwrap().len(), SIZE as u64);
    nested(&root.socket(), "nested", true);
    drop(record);
    assert!(!path(&root.0, &observer).exists());
}
#[test]
fn unknown_or_malformed_records_are_not_promoted_to_canonical_roots() {
    let root = Private::new();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let id = "a".repeat(32);
    let record = Record::create(
        &root.socket(),
        &observer,
        &id,
        &AccountingRelation::Unverified {},
    )
    .unwrap();
    nested(&root.socket(), "unknown", true);
    fs::set_permissions(&record.path, fs::Permissions::from_mode(0o644)).unwrap();
    nested(&root.socket(), "unknown", false);
    fs::set_permissions(&record.path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&record.path, b"invalid").unwrap();
    nested(&root.socket(), "unknown", false);
    assert!(read(&root.0, &observer).is_err());
}
#[test]
fn symlinks_foreign_boot_hardlinks_and_replaced_inode_cannot_establish_lineage() {
    let root = Private::new();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let id = "a".repeat(32);
    let record = Record::create(
        &root.socket(),
        &observer,
        &id,
        &AccountingRelation::CanonicalRoot {
            root_job_id: id.clone(),
        },
    )
    .unwrap();
    let original = root.0.join("original");
    fs::rename(&record.path, &original).unwrap();
    symlink(&original, &record.path).unwrap();
    nested(&root.socket(), "unknown", false);
    fs::remove_file(&record.path).unwrap();
    fs::hard_link(&original, &record.path).unwrap();
    nested(&root.socket(), "unknown", false);
    fs::remove_file(&record.path).unwrap();
    let mut bytes = fs::read(&original).unwrap();
    bytes[8] = if bytes[8] == b'a' { b'b' } else { b'a' };
    fs::write(&record.path, bytes).unwrap();
    fs::set_permissions(&record.path, fs::Permissions::from_mode(0o600)).unwrap();
    nested(&root.socket(), "unknown", false);
    drop(record);
    assert!(
        path(&root.0, &observer).exists(),
        "Drop must preserve a replacement inode"
    );
}
#[test]
fn invalid_identity_directory_and_record_relation_stay_unknown() {
    let root = Private::new();
    let mut observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let id = "a".repeat(32);
    assert!(Record::create(
        &root.socket(),
        &observer,
        &id,
        &AccountingRelation::CanonicalRoot {
            root_job_id: "b".repeat(32)
        }
    )
    .is_err());
    assert!(Record::create(
        &root.socket(),
        &observer,
        "invalid",
        &AccountingRelation::Unverified {}
    )
    .is_err());
    fs::set_permissions(&root.0, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        relation(&root.socket(), &observer, &id),
        AccountingRelation::Unverified {}
    );
    observer.start_token = "0".into();
    assert!(ancestors(&observer).is_err());
    assert_eq!(
        relation(&root.socket(), &observer, "invalid"),
        AccountingRelation::Unverified {}
    );
}
