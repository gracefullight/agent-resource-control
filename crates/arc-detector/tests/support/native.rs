//! Shared assertions only; family grammar data stays exclusively in its lane.
#![allow(dead_code)]
use arc_detector::{
    analyze_argv, classify_argv, Classification, JobClass, Locality, Reason, Uncertainty,
    MAX_INPUT_BYTES, MAX_TOKENS, MAX_WORK,
};
use std::ffi::OsString;

pub fn argv(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}
pub fn known(values: &[&str], reason: Reason) {
    let original = argv(values);
    let expected = arc_detector::native::CLASSIFICATIONS
        .into_iter()
        .find(|c| c.reason == reason)
        .unwrap();
    let (actual, work) = analyze_argv(&original);
    assert_eq!(actual, expected, "{values:?}");
    assert_eq!(
        analyze_argv(&original),
        (actual, work),
        "deterministic result and budget"
    );
    assert_eq!(
        original,
        argv(values),
        "native arguments must be unmodified"
    );
    assert!(work.work <= MAX_WORK && work.tokens <= MAX_TOKENS);
}
pub fn observe(values: &[&str]) {
    let original = argv(values);
    let (actual, work) = analyze_argv(&original);
    assert_eq!(actual.classification, JobClass::Unknown, "{values:?}");
    assert_eq!(actual.uncertainty, Uncertainty::Observe, "{values:?}");
    assert_eq!(analyze_argv(&original), (actual, work));
    assert_eq!(original, argv(values));
    assert!(work.work <= MAX_WORK);
}
pub fn limits(prefix: &[&str]) {
    let mut too_many = argv(prefix);
    too_many.resize(MAX_TOKENS + 1, OsString::from("x"));
    assert_eq!(classify_argv(&too_many).reason, Reason::TokenLimit);
    let mut too_long = argv(prefix);
    too_long.push(OsString::from("x".repeat(MAX_INPUT_BYTES)));
    assert_eq!(classify_argv(&too_long).reason, Reason::InputLimit);
    let mut nul = argv(prefix);
    nul.push(OsString::from("a\0b"));
    assert_eq!(classify_argv(&nul).reason, Reason::UnsupportedSyntax);
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let mut bytes = argv(prefix);
        bytes.push(OsString::from_vec(vec![255]));
        let saved = bytes.clone();
        assert_eq!(classify_argv(&bytes).reason, Reason::UnsupportedSyntax);
        assert_eq!(bytes, saved);
    }
}

pub fn vocabulary() {
    for good in arc_detector::native::CLASSIFICATIONS {
        assert!(good.is_consistent() && good.is_native_tool());
        assert_eq!(
            serde_json::from_value::<Classification>(serde_json::to_value(good).unwrap()).unwrap(),
            good
        );
        for bad in [
            Classification {
                classification: JobClass::Unknown,
                ..good
            },
            Classification {
                locality: Locality::Remote,
                ..good
            },
            Classification {
                uncertainty: Uncertainty::Observe,
                ..good
            },
        ] {
            assert!(!bad.is_consistent());
        }
        assert_eq!(
            good.legacy(),
            Classification {
                classification: JobClass::Unknown,
                locality: Locality::Unknown,
                reason: Reason::UnresolvedCommand,
                uncertainty: Uncertainty::Observe
            }
        );
    }
}

/// Fresh child process makes PATH poisoning isolated and safe for parallel unit tests.
/// Injected metadata panics if a future recognizer attempts project inspection.
pub fn poison(test_name: &str, commands: &[(&[&str], Reason)]) {
    use arc_detector::scripts::{MetadataError, MetadataStats, PackageMetadata, PackageSnapshot};
    use std::{fs, path::Path, process::Command};
    const CHILD: &str = "ARC_NATIVE_CLASSIFICATION_POISON";
    struct PoisonMetadata;
    impl PackageMetadata for PoisonMetadata {
        fn package(
            &mut self,
            _: &Path,
            _: &mut MetadataStats,
        ) -> Result<PackageSnapshot, MetadataError> {
            panic!("native argv classification read project metadata")
        }
        fn revalidate(
            &mut self,
            _: &[PackageSnapshot],
            _: &mut MetadataStats,
        ) -> Result<(), MetadataError> {
            panic!("native argv classification revalidated project metadata")
        }
    }
    if let Some(root) = std::env::var_os(CHILD) {
        let root = std::path::PathBuf::from(root);
        for &(command, reason) in commands {
            known(command, reason);
            let mut args = argv(command);
            args[0] = root.join(command[0]).into_os_string();
            assert_eq!(classify_argv(&args).reason, reason);
            let result = arc_detector::analyze_script_argv(&args, &root, &mut PoisonMetadata);
            assert_eq!(result.classification.reason, reason);
            assert!(result.packages.is_empty());
            assert_eq!(result.metadata, MetadataStats::default());
        }
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "arc-native-poison-{test_name}-{}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    for name in ["go", "swift", "cmake", "ctest"] {
        let path = root.join(name);
        fs::write(
            &path,
            format!(
                "#!/bin/sh\nprintf executed > '{}/LAUNCHED'\nexit 99\n",
                root.display()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
    for name in [
        "go.mod",
        "go.work",
        "Package.swift",
        "CMakeLists.txt",
        "CTestTestfile.cmake",
    ] {
        fs::write(
            root.join(name),
            "POISON: must never evaluate project files during classification\n",
        )
        .unwrap();
    }
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test_name, "--nocapture"])
        .current_dir(&root)
        .env(CHILD, &root)
        .env("PATH", &root)
        .output()
        .unwrap();
    let launched = root.join("LAUNCHED").exists();
    fs::remove_dir_all(root).unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!launched, "classification executed a poisoned tool");
}
