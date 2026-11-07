use std::{env, process::Command};

#[test]
fn configuration_probe() {
    if env::var_os("ARC_CONFIGURATION_PROBE").is_some() {
        println!("socket={}", arc_core::socket_path().unwrap().display());
        println!("database={}", arc_core::database_path().unwrap().display());
        println!("anchor={}", arc_core::instance_anchor().unwrap().display());
    }
}

fn probe(tmpdir: Option<&str>, socket: Option<&str>) -> String {
    let mut child = Command::new(env::current_exe().unwrap());
    child
        .args(["--exact", "configuration_probe", "--nocapture"])
        .env("ARC_CONFIGURATION_PROBE", "1")
        .env_remove(arc_core::ARC_SOCKET_PATH)
        .env_remove(arc_core::ARC_DB_PATH)
        .env_remove("TMPDIR");
    if let Some(tmpdir) = tmpdir {
        child.env("TMPDIR", tmpdir);
    }
    if let Some(socket) = socket {
        child.env(arc_core::ARC_SOCKET_PATH, socket);
    }
    let result = child.output().unwrap();
    assert!(result.status.success(), "{result:?}");
    String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .filter(|line| {
            line.starts_with("socket=")
                || line.starts_with("database=")
                || line.starts_with("anchor=")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(target_os = "macos")]
#[test]
fn native_default_is_independent_of_child_tmpdir() {
    let unset = probe(None, None);
    assert_eq!(unset, probe(Some("/changed/runtime/root"), None));
    assert_eq!(unset, probe(Some("/System/arc-unwritable"), None));
    assert!(unset.contains("/Library/Application Support/arc/arc.sqlite3"));
}

#[test]
fn nonempty_socket_override_and_empty_override_policy_are_preserved() {
    assert_eq!(probe(None, None), probe(None, Some("")));
    assert!(probe(None, Some("/explicit/arc.sock")).contains("socket=/explicit/arc.sock"));
    let anchor = |output: String| {
        output
            .lines()
            .find(|line| line.starts_with("anchor="))
            .unwrap()
            .to_owned()
    };
    assert_eq!(
        anchor(probe(None, None)),
        anchor(probe(Some("/other"), Some("/explicit/arc.sock")))
    );
}
