//! Observable checkpoints for stale settings and overlapping ARC updates.
use super::{claude, Fixture, InstallAction, Scope};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

const GUARD: Duration = Duration::from_secs(90);

fn paths(fixture: &Fixture, scope: Scope) -> (PathBuf, Option<&Path>) {
    match scope {
        Scope::User => (fixture.home.join(".claude/settings.json"), None),
        Scope::Workspace => (
            fixture.workspace.join(".claude/settings.json"),
            Some(&fixture.workspace),
        ),
    }
}

fn identity_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".arc-claude-id");
    name.into()
}

fn receive<T>(receiver: &mpsc::Receiver<T>, what: &str) -> T {
    receiver
        .recv_timeout(GUARD)
        .unwrap_or_else(|error| panic!("{what} did not arrive: {error}"))
}

pub(super) fn unrelated_edits(fixture: &Fixture, scope: Scope) {
    let (path, workspace) = paths(fixture, scope);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, include_bytes!("settings.json")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let context = fixture.context();
    for action in [InstallAction::Install, InstallAction::Uninstall] {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (continue_tx, continue_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        thread::scope(|threads| {
            let context_ref = &context;
            threads.spawn(move || {
                let result = claude::configure_with_checkpoint(
                    action,
                    scope,
                    workspace,
                    context_ref,
                    || {},
                    || {},
                    || {
                        ready_tx.send(()).unwrap();
                        receive(
                            &continue_rx,
                            "permission to continue the settings replacement",
                        );
                    },
                );
                done_tx.send(result).unwrap();
            });
            receive(&ready_rx, "prepared settings replacement");
            let identity = fs::read(identity_path(&path)).unwrap();
            let mut edited: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            edited["concurrentUserEdit"] = json!(format!("{action:?}"));
            let edited = serde_json::to_vec(&edited).unwrap();
            if matches!(action, InstallAction::Install) {
                // Simulate an editor that publishes by atomic replacement.
                let replacement = path.with_extension("user-new");
                fs::write(&replacement, &edited).unwrap();
                fs::set_permissions(&replacement, fs::Permissions::from_mode(0o640)).unwrap();
                fs::rename(replacement, &path).unwrap();
            } else {
                // Simulate an editor that updates the existing inode.
                fs::write(&path, &edited).unwrap();
            }
            continue_tx.send(()).unwrap();
            let error = receive(&done_rx, "stale update result").unwrap_err();
            assert!(error.contains("changed during"), "{error}");
            assert_eq!(fs::read(&path).unwrap(), edited);
            assert_eq!(fs::read(identity_path(&path)).unwrap(), identity);
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o640
            );
        });
        // The recorded identity makes the interrupted operation recoverable.
        claude::configure(action, scope, workspace, &context).unwrap();
        let result: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(result["concurrentUserEdit"], format!("{action:?}"));
        let groups = result["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(groups[0]["hooks"][0]["command"], "first-hook");
        assert_eq!(groups[1]["hooks"][0]["command"], "before-one");
        assert_eq!(groups[1]["hooks"][1]["command"], "before-two");
    }
    assert!(!identity_path(&path).exists());

    // An outside writer may change access metadata without touching contents.
    claude::configure(InstallAction::Install, scope, workspace, &context).unwrap();
    let installed = fs::read(&path).unwrap();
    let identity = fs::read(identity_path(&path)).unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    let (continue_tx, continue_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    thread::scope(|threads| {
        let context_ref = &context;
        threads.spawn(move || {
            let result = claude::configure_with_checkpoint(
                InstallAction::Uninstall,
                scope,
                workspace,
                context_ref,
                || {},
                || {},
                || {
                    ready_tx.send(()).unwrap();
                    receive(
                        &continue_rx,
                        "permission to continue after the metadata edit",
                    );
                },
            );
            done_tx.send(result).unwrap();
        });
        receive(&ready_rx, "uninstall prepared before metadata edit");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        continue_tx.send(()).unwrap();
        assert!(receive(&done_rx, "stale metadata update result")
            .unwrap_err()
            .contains("changed during"));
        assert_eq!(fs::read(&path).unwrap(), installed);
        assert_eq!(fs::read(identity_path(&path)).unwrap(), identity);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    });
    claude::configure(InstallAction::Uninstall, scope, workspace, &context).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    #[cfg(target_os = "macos")]
    attribute_edit(&path, workspace, scope, &context);
}

#[cfg(target_os = "macos")]
fn attribute_edit(
    path: &Path,
    workspace: Option<&Path>,
    scope: Scope,
    context: &super::HookContext,
) {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    claude::configure(InstallAction::Install, scope, workspace, context).unwrap();
    let installed = fs::read(path).unwrap();
    let identity = fs::read(identity_path(path)).unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    let (continue_tx, continue_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    thread::scope(|threads| {
        threads.spawn(move || {
            let result = claude::configure_with_checkpoint(
                InstallAction::Uninstall,
                scope,
                workspace,
                context,
                || {},
                || {},
                || {
                    ready_tx.send(()).unwrap();
                    receive(
                        &continue_rx,
                        "permission to continue after the attribute edit",
                    );
                },
            );
            done_tx.send(result).unwrap();
        });
        receive(&ready_rx, "uninstall prepared before attribute edit");
        let path_c = CString::new(path.as_os_str().as_bytes()).unwrap();
        let name = CString::new("com.arc.fixture.concurrent").unwrap();
        let value = b"outside edit";
        // SAFETY: the path, attribute name, and value are live for this call.
        assert_eq!(
            unsafe {
                libc::setxattr(
                    path_c.as_ptr(),
                    name.as_ptr(),
                    value.as_ptr().cast(),
                    value.len(),
                    0,
                    0,
                )
            },
            0
        );
        continue_tx.send(()).unwrap();
        assert!(receive(&done_rx, "stale attribute update result")
            .unwrap_err()
            .contains("changed during"));
        assert_eq!(fs::read(path).unwrap(), installed);
        assert_eq!(fs::read(identity_path(path)).unwrap(), identity);
    });
    claude::configure(InstallAction::Uninstall, scope, workspace, context).unwrap();
    let path_c = CString::new(path.as_os_str().as_bytes()).unwrap();
    let name = CString::new("com.arc.fixture.concurrent").unwrap();
    let mut value = [0u8; 32];
    // SAFETY: the output buffer and both C strings are valid for this call.
    let length = unsafe {
        libc::getxattr(
            path_c.as_ptr(),
            name.as_ptr(),
            value.as_mut_ptr().cast(),
            value.len(),
            0,
            0,
        )
    };
    assert!(
        length >= 0,
        "cannot read the concurrent attribute: {}",
        std::io::Error::last_os_error()
    );
    assert_eq!(&value[..length as usize], b"outside edit");
}

pub(super) fn competing_updates(fixture: &Fixture, scope: Scope) {
    let (path, workspace) = paths(fixture, scope);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, include_bytes!("settings.json")).unwrap();
    let baseline: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let context = fixture.context();
    for (first_action, second_action) in [
        (InstallAction::Install, InstallAction::Uninstall),
        (InstallAction::Uninstall, InstallAction::Install),
    ] {
        if matches!(first_action, InstallAction::Uninstall) {
            claude::configure(InstallAction::Install, scope, workspace, &context).unwrap();
        }
        let (ready_tx, ready_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (attempt_tx, attempt_rx) = mpsc::channel();
        let (observed_tx, observed_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        thread::scope(|threads| {
            let first_done = done_tx.clone();
            let context_ref = &context;
            threads.spawn(move || {
                let result = claude::configure_with_checkpoint(
                    first_action,
                    scope,
                    workspace,
                    context_ref,
                    || {},
                    || {},
                    || {
                        ready_tx.send(()).unwrap();
                        receive(&release_rx, "release of the first ARC writer");
                    },
                );
                first_done.send(result).unwrap();
            });
            receive(&ready_rx, "first ARC writer before replacement");
            let context_ref = &context;
            let path_ref = &path;
            threads.spawn(move || {
                let result = claude::configure_with_checkpoint(
                    second_action,
                    scope,
                    workspace,
                    context_ref,
                    || {
                        attempt_tx.send(()).unwrap();
                    },
                    || {
                        observed_tx
                            .send((
                                fs::read(path_ref).unwrap(),
                                identity_path(path_ref).exists(),
                            ))
                            .unwrap();
                    },
                    || {},
                );
                done_tx.send(result).unwrap();
            });
            receive(&attempt_rx, "competing ARC writer observing the held lock");
            release_tx.send(()).unwrap();
            let (observed, identity_present) =
                receive(&observed_rx, "competing ARC writer snapshot");
            let observed: Value = serde_json::from_slice(&observed).unwrap();
            let count = observed["hooks"]["PreToolUse"].as_array().unwrap().len();
            let expected = baseline["hooks"]["PreToolUse"].as_array().unwrap().len();
            assert_eq!(
                count,
                expected + usize::from(matches!(first_action, InstallAction::Install))
            );
            assert_eq!(
                identity_present,
                matches!(first_action, InstallAction::Install)
            );
            receive(&done_rx, "first completed ARC update").unwrap();
            receive(&done_rx, "second completed ARC update").unwrap();
        });
        let final_settings: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let final_count = final_settings["hooks"]["PreToolUse"]
            .as_array()
            .unwrap()
            .len();
        let baseline_count = baseline["hooks"]["PreToolUse"].as_array().unwrap().len();
        assert_eq!(
            final_count,
            baseline_count + usize::from(matches!(second_action, InstallAction::Install))
        );
        assert_eq!(
            identity_path(&path).exists(),
            matches!(second_action, InstallAction::Install)
        );
        if matches!(second_action, InstallAction::Install) {
            claude::configure(InstallAction::Uninstall, scope, workspace, &context).unwrap();
        }
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
            baseline
        );
    }
}
