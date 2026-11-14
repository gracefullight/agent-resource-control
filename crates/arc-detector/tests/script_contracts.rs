use arc_detector::{scripts::*, Classification, JobClass, Locality, Reason, Uncertainty};
use std::collections::BTreeMap;

#[test]
fn metadata_limits_charge_before_retaining_and_never_commit_overruns() {
    let mut b = MetadataStats::default();
    for _ in 0..MAX_PACKAGE_LOOKUPS {
        b.lookup().unwrap();
    }
    assert_eq!(b.lookup(), Err(MetadataError::LookupLimit));
    assert_eq!(b.lookups, MAX_PACKAGE_LOOKUPS);
    for _ in 0..MAX_PACKAGE_FILES {
        b.file().unwrap();
    }
    assert_eq!(b.file(), Err(MetadataError::FileLimit));
    assert_eq!(b.files, MAX_PACKAGE_FILES);
    assert_eq!(
        b.read(MAX_PACKAGE_BYTES + 1),
        Err(MetadataError::PackageBytes)
    );
    assert_eq!(b.bytes, 0);
    for _ in 0..(MAX_METADATA_BYTES / MAX_PACKAGE_BYTES) {
        b.read(MAX_PACKAGE_BYTES).unwrap();
    }
    assert_eq!(b.bytes, MAX_METADATA_BYTES);
    assert_eq!(b.read(1), Err(MetadataError::AggregateBytes));
    assert_eq!(b.bytes, MAX_METADATA_BYTES);
    b.bytes = usize::MAX;
    assert_eq!(b.read(1), Err(MetadataError::AggregateBytes));
}

#[test]
fn script_names_and_bodies_share_one_aggregate_retention_budget() {
    let mut b = MetadataStats::default();
    let scripts = BTreeMap::from([("x".to_owned(), "a".repeat(MAX_RETAINED_SCRIPT_BYTES - 1))]);
    b.scripts(&scripts).unwrap();
    assert_eq!(b.script_bytes, MAX_RETAINED_SCRIPT_BYTES);
    assert_eq!(
        b.scripts(&BTreeMap::from([("y".into(), String::new())])),
        Err(MetadataError::RetainedScriptBytes)
    );
    assert_eq!(b.script_bytes, MAX_RETAINED_SCRIPT_BYTES);
    let too_many = (0..=MAX_PACKAGE_SCRIPTS)
        .map(|n| (n.to_string(), String::new()))
        .collect();
    assert_eq!(
        MetadataStats::default().scripts(&too_many),
        Err(MetadataError::ScriptLimit)
    );
    let exactly = (0..MAX_PACKAGE_SCRIPTS)
        .map(|n| (n.to_string(), String::new()))
        .collect();
    assert!(MetadataStats::default().scripts(&exactly).is_ok());
    b.script_bytes = usize::MAX;
    assert_eq!(b.scripts(&scripts), Err(MetadataError::RetainedScriptBytes));
}

#[test]
fn script_classifications_validate_pairs_and_legacy_projection_is_closed() {
    for (class, reason) in [
        (JobClass::Build, Reason::BunScriptBuild),
        (JobClass::Test, Reason::BunScriptTest),
        (JobClass::Lint, Reason::BunScriptLint),
        (JobClass::Typecheck, Reason::BunScriptTypecheck),
    ] {
        let c = Classification {
            classification: class,
            reason,
            locality: Locality::HostLocal,
            uncertainty: Uncertainty::None,
        };
        assert!(c.is_consistent());
        assert!(c.is_bun_script());
        let roundtrip: Classification =
            serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(roundtrip, c);
        let old = c.legacy();
        assert!(old.is_consistent());
        assert_eq!(old.classification, JobClass::Unknown);
        assert_eq!(old.reason, Reason::UnresolvedCommand);
        let invalid = Classification {
            locality: Locality::Remote,
            ..c
        };
        assert!(!invalid.is_consistent());
        let invalid = Classification {
            classification: JobClass::RemoteExec,
            ..c
        };
        assert!(!invalid.is_consistent());
    }
    for args in [
        ["cargo", "build"],
        ["cargo", "test"],
        ["cargo", "clippy"],
        ["ssh", "host"],
    ] {
        let c = arc_detector::classify_argv(&args.map(std::ffi::OsString::from));
        assert_eq!(c, c.legacy());
    }
    // Adding TYPECHECK never makes the old syntax-only classifier guess a script.
    assert_eq!(
        arc_detector::classify_argv(&["bun", "run", "typecheck"].map(std::ffi::OsString::from))
            .reason,
        Reason::UnresolvedScript
    );
}

#[test]
fn transient_metadata_debug_never_contains_paths_or_script_bodies() {
    let snapshot = PackageSnapshot {
        package_dir: "/private/workspace".into(),
        scripts: BTreeMap::from([("private_name".into(), "secret script body".into())]),
        fingerprint: MetadataFingerprint {
            bytes: 20,
            sha256: [1; 32],
        },
    };
    assert_eq!(format!("{snapshot:?}"), "PackageSnapshot([REDACTED])");
    let mut changed = snapshot.clone();
    changed.fingerprint.sha256 = [2; 32];
    assert_ne!(snapshot, changed); // equal size cannot erase content identity
}
