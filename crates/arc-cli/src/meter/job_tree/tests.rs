use super::*;
use std::os::unix::fs::DirBuilderExt;

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "arc-cli-job-tree-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_tree_is_one_added_key_and_the_v1_fields_keep_their_values() {
    let summary = br#"{"schema_version":1,"job_id":"a","accounting":{"state":"missing"}}"#;
    let merged = merge(summary, serde_json::json!({"terminal":true})).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&merged).unwrap();
    assert_eq!(value[SUMMARY_KEY], serde_json::json!({"terminal":true}));
    value.as_object_mut().unwrap().remove(SUMMARY_KEY);
    assert_eq!(
        value,
        serde_json::from_slice::<serde_json::Value>(summary).unwrap()
    );
    // An absent report is an explicit null, never an invented zero tree.
    let merged = merge(summary, serde_json::Value::Null).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&merged).unwrap();
    assert!(value[SUMMARY_KEY].is_null());
    // A summary that already names the key, or is not an object, is left alone.
    assert!(merge(br#"{"process_tree":1}"#, serde_json::Value::Null).is_none());
    assert!(merge(b"[]", serde_json::Value::Null).is_none());
}

#[test]
fn without_an_advertising_daemon_the_v1_bytes_are_unchanged() {
    let directory = Directory::new();
    let socket = directory.0.join("ipc");
    let summary = br#"{"schema_version":1}"#.to_vec();
    assert_eq!(
        extend(&socket, &"a".repeat(32), summary.clone()).unwrap(),
        summary
    );
    // A file nobody locks advertises nothing, whoever the peer is.
    std::fs::write(directory.0.join(ADVERTISEMENT), "").unwrap();
    assert!(!holds(&socket, std::process::id()));
    assert!(!holds(Path::new("ipc"), std::process::id()));
}
