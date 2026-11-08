use std::{env, path::PathBuf, process::Command};

#[test]
fn database_path_probe() {
    let Some(case) = env::var_os("ARC_DATABASE_PATH_PROBE") else {
        return;
    };
    match case.to_str().unwrap() {
        "missing-home" => {
            let error = arc_core::database_path().unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
            assert_eq!(error.to_string(), "HOME is not set");
        }
        "override" => assert_eq!(
            arc_core::database_path().unwrap(),
            PathBuf::from(env::var_os(arc_core::ARC_DB_PATH).unwrap())
        ),
        "default" => {
            #[cfg(target_os = "macos")]
            let expected = "/fixture-home/Library/Application Support/arc/arc.sqlite3";
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let expected = "/fixture-home/.local/share/arc/arc.sqlite3";
            #[cfg(not(target_os = "windows"))]
            assert_eq!(arc_core::database_path().unwrap(), PathBuf::from(expected));
        }
        other => panic!("unknown probe case: {other}"),
    }
}

fn probe(case: &str) -> Command {
    let mut child = Command::new(env::current_exe().unwrap());
    child
        .args(["--exact", "database_path_probe", "--nocapture"])
        .env("ARC_DATABASE_PATH_PROBE", case)
        .env_remove(arc_core::ARC_DB_PATH)
        .env_remove("HOME")
        .env_remove("XDG_DATA_HOME");
    child
}

fn succeeds(child: &mut Command) {
    let output = child.output().unwrap();
    assert!(output.status.success(), "{output:?}");
}

#[cfg(not(target_os = "windows"))]
#[test]
fn absent_and_empty_home_fail_after_empty_database_override() {
    succeeds(&mut probe("missing-home"));
    succeeds(
        probe("missing-home")
            .env("HOME", "")
            .env(arc_core::ARC_DB_PATH, ""),
    );
    succeeds(
        probe("default")
            .env("HOME", "/fixture-home")
            .env(arc_core::ARC_DB_PATH, ""),
    );
}

#[test]
fn database_override_precedes_missing_platform_directory() {
    succeeds(probe("override").env(arc_core::ARC_DB_PATH, "relative/database.sqlite3"));
}

#[cfg(unix)]
#[test]
fn database_override_preserves_non_utf8_path_bytes() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    succeeds(probe("override").env(
        arc_core::ARC_DB_PATH,
        OsStr::from_bytes(b"/fixture/\xff/database.sqlite3"),
    ));
}
