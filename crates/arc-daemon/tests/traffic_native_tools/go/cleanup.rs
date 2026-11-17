//! Both watchdog and unwind must reap Go -> test binary -> native grandchild.
use super::super::{eventually, root, write, Form};
use super::{owned::GoChild, projects::*};
use std::{
    fs, io,
    process::{Command, Stdio},
    time::Instant,
};

#[test]
fn interrupted_go_fixture_reaps_native_descendants() {
    let tools = Tools::installed();
    let base = root().join(format!(
        "target/traffic-native-tools/go/interrupted-{}",
        std::process::id()
    ));
    for mode in ["watchdog", "unwind"] {
        let path = project(&base, &tools, mode, Form::GoTest, false, false);
        write(
            &path.join("interrupt_test.go"),
            include_str!("sources/interrupt_test.go.txt"),
        );
        let mut command = Command::new(&tools.go);
        command
            .args([
                "test",
                "-p=2",
                "-count=1",
                "-run=^TestInterruptRoot$",
                "-timeout=30s",
                ".",
            ])
            .current_dir(&path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure(&path, &mut command);
        let mut child = GoChild::spawn(&mut command).unwrap();
        eventually(|| {
            assert!(child.child.try_wait().unwrap().is_none());
            child.observe().unwrap();
            path.join("interrupt-root").exists() && path.join("interrupt-leaf").exists()
        });
        child.observe().unwrap();
        let identities = child.identities();
        assert!(
            identities.len() >= 3,
            "must own actual native grandchildren"
        );
        for marker in ["interrupt-root", "interrupt-leaf"] {
            let pid: u32 = fs::read_to_string(path.join(marker))
                .unwrap()
                .parse()
                .unwrap();
            assert!(identities.iter().any(|identity| identity.pid == pid));
        }
        if mode == "watchdog" {
            let error = child.wait_until(Instant::now()).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::TimedOut, "{error}");
            drop(child);
        } else {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let _owned = child;
                panic!("intentional Go fixture unwind");
            }));
            assert!(result.is_err());
            assert!(crate::cleanup::take_unwind_errors().is_empty());
        }
        for identity in identities {
            assert!(
                crate::cleanup::capture(identity.pid).unwrap().as_ref() != Some(&identity),
                "native descendant still exists: {identity:?}"
            );
        }
    }
    fs::remove_dir_all(base).unwrap();
}
