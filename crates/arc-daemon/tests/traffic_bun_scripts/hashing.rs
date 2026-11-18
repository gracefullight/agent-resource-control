use super::projects;
use std::{fs, path::Path, process::Command};

pub fn sha(path: &Path) -> String {
    use std::{
        collections::HashMap,
        os::unix::fs::MetadataExt,
        sync::{Mutex, OnceLock},
    };

    // Fixture shims have identical bytes in each private directory, while
    // executable witnesses repeatedly refer to the same immutable inode.
    // Cache only an exact byte match or an unchanged native file identity.
    type FileIdentity = (u64, u64, u64, i64, i64, i64, i64);
    static SMALL: OnceLock<Mutex<HashMap<Vec<u8>, String>>> = OnceLock::new();
    static LARGE: OnceLock<Mutex<HashMap<FileIdentity, String>>> = OnceLock::new();
    let identity = |m: &fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    let before = fs::metadata(path).unwrap();
    let small = (before.len() <= 4096).then(|| fs::read(path).unwrap());
    if let Some(bytes) = &small {
        if let Some(hash) = SMALL
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .get(bytes)
        {
            return hash.clone();
        }
    } else if let Some(hash) = LARGE
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .get(&identity(&before))
    {
        return hash.clone();
    }
    let mut command = Command::new("/usr/bin/shasum");
    command.args(["-a", "256"]).arg(path);
    let out = projects::fixture_output(&mut command).unwrap();
    assert!(out.status.success());
    let hash = String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    assert_eq!(
        identity(&before),
        identity(&fs::metadata(path).unwrap()),
        "hashed fixture changed"
    );
    if let Some(bytes) = small {
        assert_eq!(bytes, fs::read(path).unwrap(), "hashed fixture changed");
        SMALL
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .insert(bytes, hash.clone());
    } else {
        LARGE
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .insert(identity(&before), hash.clone());
    }
    hash
}
