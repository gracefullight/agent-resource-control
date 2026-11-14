use super::*;
use arc_detector::{analyze_script_argv, scripts::*, JobClass};
use std::fs::{self, FileTimes};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "arc-metadata-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir.canonicalize().unwrap())
    }
    fn manifest(&self, relative: &str, value: serde_json::Value) -> PathBuf {
        let dir = self.0.join(relative);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("package.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
        dir
    }
    fn analyze(&self, source: &mut FreshMetadata, args: &[&str]) -> ScriptResolution {
        analyze_script_argv(
            &args.iter().map(OsString::from).collect::<Vec<_>>(),
            &self.0,
            source,
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn replacement_same_size_preserved_mtime_and_fresh_invocations_use_content_identity() {
    for replace in [false, true] {
        let fixture = Fixture::new();
        fixture.manifest(
            "",
            serde_json::json!({"scripts":{"misleading":"vite build "}}),
        );
        let path = fixture.0.join("package.json");
        let prior = fs::metadata(&path).unwrap();
        let mut source = FreshMetadata::default();
        let mut result = fixture.analyze(&mut source, &["bun", "run", "misleading"]);
        assert_eq!(result.classification.classification, JobClass::Build);
        let changed =
            serde_json::to_vec(&serde_json::json!({"scripts":{"misleading":"biome check"}}))
                .unwrap();
        assert_eq!(changed.len() as u64, prior.len());
        if replace {
            let replacement = fixture.0.join("replacement");
            fs::write(&replacement, &changed).unwrap();
            fs::rename(replacement, &path).unwrap();
        } else {
            fs::write(&path, &changed).unwrap();
        }
        OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(FileTimes::new().set_modified(prior.modified().unwrap()))
            .unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().modified().unwrap(),
            prior.modified().unwrap()
        );
        assert_eq!(
            source.revalidate(&result.packages, &mut result.metadata),
            Err(E::Changed)
        );
        let fresh = fixture.analyze(&mut FreshMetadata::default(), &["bun", "run", "misleading"]);
        assert_eq!(fresh.classification.classification, JobClass::Lint);
        assert_ne!(
            fresh.packages[0].fingerprint,
            result.packages[0].fingerprint
        );
    }
}

#[test]
fn nested_and_lifecycle_metadata_changes_deletions_and_new_nearer_manifests_invalidate() {
    let fixture = Fixture::new();
    fixture.manifest(
        "",
        serde_json::json!({"scripts":{"outer":"bun run --cwd nested target"}}),
    );
    let nested = fixture.manifest(
        "nested",
        serde_json::json!({"scripts":{"pretarget":"bun test", "target":"bun test"}}),
    );
    let mut source = FreshMetadata::default();
    let mut result = fixture.analyze(&mut source, &["bun", "run", "outer"]);
    assert_eq!(result.packages.len(), 2);
    source
        .revalidate(&result.packages, &mut result.metadata)
        .unwrap();
    fixture.manifest(
        "nested",
        serde_json::json!({"scripts":{"pretarget":"biome check", "target":"bun test"}}),
    );
    assert_eq!(
        source.revalidate(&result.packages, &mut result.metadata),
        Err(E::Changed)
    );
    fs::remove_file(nested.join("package.json")).unwrap();
    assert_eq!(
        source.revalidate(&result.packages, &mut MetadataStats::default()),
        Err(E::Changed)
    );

    let fixture = Fixture::new();
    fixture.manifest("", serde_json::json!({"scripts":{"target":"bun test"}}));
    let subdir = fixture.0.join("subdir");
    fs::create_dir(&subdir).unwrap();
    let mut source = FreshMetadata::default();
    let mut b = MetadataStats::default();
    let first = source.package(&subdir, &mut b).unwrap();
    assert_eq!(first.package_dir, fixture.0);
    fixture.manifest(
        "subdir",
        serde_json::json!({"scripts":{"target":"vite build"}}),
    );
    assert_eq!(source.revalidate(&[first], &mut b), Err(E::Changed));
}

#[test]
fn every_io_and_retention_limit_is_enforced_before_decoding_or_retaining() {
    let fixture = Fixture::new();
    let path = fixture.0.join("package.json");
    let json = br#"{"scripts":{"x":"bun test"}}"#;
    let mut bytes = json.to_vec();
    bytes.resize(MAX_PACKAGE_BYTES, b' ');
    fs::write(&path, &bytes).unwrap();
    let mut source = FreshMetadata::default();
    let mut b = MetadataStats::default();
    let snapshot = source.package(&fixture.0, &mut b).unwrap();
    assert_eq!(b.bytes, MAX_PACKAGE_BYTES);
    source.revalidate(&[snapshot], &mut b).unwrap();
    assert_eq!(b.bytes, 2 * MAX_PACKAGE_BYTES);
    bytes.push(b' ');
    fs::write(&path, &bytes).unwrap();
    assert_eq!(
        FreshMetadata::default().package(&fixture.0, &mut MetadataStats::default()),
        Err(E::PackageBytes)
    );
    fs::write(&path, json).unwrap();
    let mut b = MetadataStats {
        bytes: MAX_METADATA_BYTES - json.len(),
        ..MetadataStats::default()
    };
    let mut source = FreshMetadata::default();
    let snapshot = source.package(&fixture.0, &mut b).unwrap();
    assert_eq!(
        source.revalidate(&[snapshot], &mut b),
        Err(E::AggregateBytes)
    );
    let mut b = MetadataStats {
        bytes: MAX_METADATA_BYTES - json.len() + 1,
        ..MetadataStats::default()
    };
    assert_eq!(
        FreshMetadata::default().package(&fixture.0, &mut b),
        Err(E::AggregateBytes)
    );

    let mut source = FreshMetadata::default();
    let mut b = MetadataStats::default();
    for n in 0..=MAX_PACKAGE_FILES {
        let dir = fixture.manifest(
            &format!("p{n}"),
            serde_json::json!({"scripts":{"x":"bun test"}}),
        );
        let result = source.package(&dir, &mut b);
        if n < MAX_PACKAGE_FILES {
            result.unwrap();
        } else {
            assert_eq!(result, Err(E::FileLimit));
        }
    }
    assert_eq!(b.files, MAX_PACKAGE_FILES);
    let mut b = MetadataStats {
        lookups: MAX_PACKAGE_LOOKUPS,
        ..MetadataStats::default()
    };
    assert_eq!(
        FreshMetadata::default().package(&fixture.0, &mut b),
        Err(E::LookupLimit)
    );
    let many: BTreeMap<_, _> = (0..=MAX_PACKAGE_SCRIPTS)
        .map(|n| (format!("x{n}"), "bun test"))
        .collect();
    fixture.manifest("", serde_json::json!({"scripts":many}));
    assert_eq!(
        FreshMetadata::default().package(&fixture.0, &mut MetadataStats::default()),
        Err(E::ScriptLimit)
    );
    let exactly: BTreeMap<_, _> = (0..MAX_PACKAGE_SCRIPTS)
        .map(|n| (format!("x{n}"), "bun test"))
        .collect();
    fixture.manifest("", serde_json::json!({"scripts":exactly}));
    let mut b = MetadataStats {
        lookups: MAX_PACKAGE_LOOKUPS - 2,
        ..MetadataStats::default()
    };
    let snapshot = FreshMetadata::default()
        .package(&fixture.0, &mut b)
        .unwrap();
    assert_eq!(snapshot.scripts.len(), MAX_PACKAGE_SCRIPTS);
    assert_eq!(b.lookups, MAX_PACKAGE_LOOKUPS);
    fixture.manifest(
        "",
        serde_json::json!({"scripts":{"x":"a".repeat(MAX_RETAINED_SCRIPT_BYTES)}}),
    );
    assert_eq!(
        FreshMetadata::default().package(&fixture.0, &mut MetadataStats::default()),
        Err(E::RetainedScriptBytes)
    );
}

#[test]
fn bounded_discovery_and_nonregular_or_malformed_metadata_never_block() {
    let fixture = Fixture::new();
    let mut deep = fixture.0.clone();
    for _ in 0..MAX_PACKAGE_LOOKUPS {
        deep.push("d");
    }
    fs::create_dir_all(&deep).unwrap();
    let mut b = MetadataStats::default();
    assert_eq!(
        FreshMetadata::default().package(&deep, &mut b),
        Err(E::LookupLimit)
    );
    assert_eq!(b.lookups, MAX_PACKAGE_LOOKUPS);
    for invalid in [
        "[]",
        "{",
        r#"{"scripts":[]} "#,
        r#"{"scripts":{"x":17}}"#,
        r#"{"scripts":{"":"bun test"}}"#,
    ] {
        fs::write(fixture.0.join("package.json"), invalid).unwrap();
        assert_eq!(
            FreshMetadata::default().package(&fixture.0, &mut MetadataStats::default()),
            Err(E::Invalid)
        );
    }
    let path = fixture.0.join("package.json");
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(fixture.0.join("missing"), &path).unwrap();
    assert_eq!(
        FreshMetadata::default().package(&fixture.0, &mut MetadataStats::default()),
        Err(E::Unreadable)
    );
    fs::remove_file(&path).unwrap();
    use std::os::unix::ffi::OsStrExt;
    let path_c = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(path_c.as_ptr(), 0o600) }, 0);
    assert_eq!(
        FreshMetadata::default().package(&fixture.0, &mut MetadataStats::default()),
        Err(E::Unreadable)
    );
}

#[test]
fn real_repository_scripts_classify_without_executing_any_script_or_probes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (name, class) in [
        ("desktop:build", JobClass::Build),
        ("desktop:typecheck", JobClass::Typecheck),
    ] {
        let mut source = FreshMetadata::default();
        let mut result = analyze_script_argv(
            &["bun".into(), "run".into(), name.into()],
            root,
            &mut source,
        );
        assert_eq!(result.classification.classification, class, "{result:?}");
        assert_eq!(result.packages.len(), 2);
        source
            .revalidate(&result.packages, &mut result.metadata)
            .unwrap();
        if class == JobClass::Typecheck {
            assert_eq!(result.analysis.invocations, 4);
        }
    }
    let fixture = Fixture::new();
    let poison = fixture.0.join("must-not-exist");
    fixture.manifest("", serde_json::json!({"scripts":{"prebuild":format!("touch '{}'", poison.display()),"build":"vite build"}}));
    let result = fixture.analyze(&mut FreshMetadata::default(), &["bun", "run", "build"]);
    assert_eq!(result.classification.classification, JobClass::Unknown);
    assert!(
        !poison.exists(),
        "Classification must never execute lifecycle or tool discovery"
    );
}

#[test]
fn distinct_manifests_share_byte_and_retained_script_budgets_including_revalidation() {
    let fixture = Fixture::new();
    let mut source = FreshMetadata::default();
    let mut b = MetadataStats::default();
    let mut snapshots = Vec::new();
    for n in 0..4 {
        let dir = fixture.manifest(
            &format!("p{n}"),
            serde_json::json!({"scripts":{"x":"bun test"}}),
        );
        let mut bytes = fs::read(dir.join("package.json")).unwrap();
        bytes.resize(MAX_PACKAGE_BYTES, b' ');
        fs::write(dir.join("package.json"), bytes).unwrap();
        snapshots.push(source.package(&dir, &mut b).unwrap());
    }
    assert_eq!(b.bytes, MAX_METADATA_BYTES);
    assert_eq!(
        source.revalidate(&snapshots, &mut b),
        Err(E::AggregateBytes)
    );
    let mut source = FreshMetadata::default();
    let mut b = MetadataStats::default();
    let body = "x".repeat(MAX_RETAINED_SCRIPT_BYTES / 2 - 1);
    for n in 0..2 {
        let dir = fixture.manifest(
            &format!("retained{n}"),
            serde_json::json!({"scripts":{"x":body}}),
        );
        source.package(&dir, &mut b).unwrap();
    }
    assert_eq!(b.script_bytes, MAX_RETAINED_SCRIPT_BYTES);
    let dir = fixture.manifest("overflow", serde_json::json!({"scripts":{"x":"bun test"}}));
    assert_eq!(source.package(&dir, &mut b), Err(E::RetainedScriptBytes));
    assert_eq!(b.script_bytes, MAX_RETAINED_SCRIPT_BYTES);
}
