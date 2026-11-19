//! Installer race, access, and ownership regressions for the shared seam.
use super::hooks::{
    fixture::{claude, cursor, OwnedChild},
    HookContext, InstallAction, Scope,
};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::{
        fs::{symlink, PermissionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "arc-installer-hardening-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn context(&self) -> HookContext {
        HookContext {
            executable: Some(PathBuf::from(env!("CARGO_BIN_EXE_arcctl"))),
        }
    }
    fn settings(&self, vendor: &str) -> PathBuf {
        let (directory, file) = if vendor == "claude" {
            (".claude", "settings.json")
        } else {
            (".cursor", "hooks.json")
        };
        fs::create_dir_all(self.0.join(directory)).unwrap();
        self.0.join(directory).join(file)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

fn configure(fixture: &Fixture, vendor: &str, action: InstallAction) -> Result<(), &'static str> {
    match vendor {
        "claude" => claude::configure(
            action,
            Scope::Workspace,
            Some(&fixture.0),
            &fixture.context(),
        ),
        "cursor" => cursor::configure(
            action,
            Scope::Workspace,
            Some(&fixture.0),
            &fixture.context(),
        ),
        _ => unreachable!(),
    }
}

#[test]
fn intervening_settings_and_ownership_changes_are_preserved() {
    for vendor in ["claude", "cursor"] {
        for target in ["settings", "sidecar", "sidecar-replaced"] {
            let fixture = Fixture::new();
            let path = fixture.settings(vendor);
            fs::write(&path, b"{}").unwrap();
            configure(&fixture, vendor, InstallAction::Install).unwrap();
            let before = fs::read(&path).unwrap();
            let sidecar = if vendor == "claude" {
                path.with_file_name("settings.json.arc-claude-id")
            } else {
                path.with_file_name(".hooks.json.arc.lock")
            };
            let edited = if target == "settings" {
                &path
            } else {
                &sidecar
            };
            let edit = || {
                if target == "sidecar-replaced" {
                    let replacement = fixture.0.join("sidecar-replacement");
                    fs::write(&replacement, b"outside writer").unwrap();
                    fs::rename(replacement, edited).unwrap();
                } else {
                    fs::write(edited, b"outside writer").unwrap();
                }
            };
            let result = if vendor == "claude" {
                claude::configure_with_checkpoint(
                    InstallAction::Uninstall,
                    Scope::Workspace,
                    Some(&fixture.0),
                    &fixture.context(),
                    || {},
                    || {},
                    edit,
                )
            } else {
                cursor::configure_intercepted(
                    InstallAction::Uninstall,
                    Scope::Workspace,
                    Some(&fixture.0),
                    &fixture.context(),
                    || {},
                    edit,
                )
            };
            assert!(result.is_err(), "{vendor} accepted {target} edit");
            assert_eq!(fs::read(edited).unwrap(), b"outside writer");
            if target != "settings" {
                assert_eq!(fs::read(&path).unwrap(), before);
            }
        }
    }
}

#[test]
fn symlink_retarget_during_install_does_not_touch_either_target() {
    let fixture = Fixture::new();
    let link = fixture.settings("claude");
    let first = fixture.0.join("first.json");
    let second = fixture.0.join("second.json");
    fs::write(&first, b"{} ").unwrap();
    fs::write(&second, b"{\"second\":true}").unwrap();
    symlink(&first, &link).unwrap();
    let result = claude::configure_with_checkpoint(
        InstallAction::Install,
        Scope::Workspace,
        Some(&fixture.0),
        &fixture.context(),
        || {},
        || {},
        || {
            fs::remove_file(&link).unwrap();
            symlink(&second, &link).unwrap();
        },
    );
    assert!(result.unwrap_err().contains("symlink changed"));
    assert_eq!(fs::read_link(link).unwrap(), second);
    assert_eq!(fs::read(first).unwrap(), b"{} ");
    assert_eq!(fs::read(second).unwrap(), b"{\"second\":true}");

    let fixture = Fixture::new();
    let link = fixture.settings("cursor");
    let target = fixture.0.join("outside.json");
    fs::write(&target, b"{} ").unwrap();
    symlink(&target, &link).unwrap();
    assert!(configure(&fixture, "cursor", InstallAction::Install).is_err());
    assert_eq!(fs::read(target).unwrap(), b"{} ");
    assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
}

#[test]
fn alternate_matchers_and_extra_fields_preserve_unrelated_and_reject_ambiguous_entries() {
    for vendor in ["claude", "cursor"] {
        let fixture = Fixture::new();
        let path = fixture.settings(vendor);
        let (event, original) = if vendor == "claude" {
            (
                "PreToolUse",
                json!({"matcher":"Read|Bash","timeout":17,
                "hooks":[{"type":"command","command":"echo unrelated","timeout":9}]}),
            )
        } else {
            (
                "preToolUse",
                json!({"matcher":"Read|Shell","command":"echo unrelated","timeout":17}),
            )
        };
        fs::write(
            &path,
            serde_json::to_vec(&json!({"hooks":{(event):[original.clone()]}})).unwrap(),
        )
        .unwrap();
        configure(&fixture, vendor, InstallAction::Install).unwrap();
        let installed: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(installed["hooks"][event][0], original);
        for mutation in ["matcher", "extra"] {
            let mut edited = installed.clone();
            let owned = &mut edited["hooks"][event][1];
            if mutation == "matcher" {
                owned["matcher"] = json!(if vendor == "claude" {
                    "Bash|Read"
                } else {
                    "Shell|Read"
                });
            } else {
                owned["timeout"] = json!(42);
            }
            let bytes = serde_json::to_vec(&edited).unwrap();
            fs::write(&path, &bytes).unwrap();
            for action in [InstallAction::Install, InstallAction::Uninstall] {
                assert!(
                    configure(&fixture, vendor, action).is_err(),
                    "{vendor}: {mutation}"
                );
                assert_eq!(fs::read(&path).unwrap(), bytes);
            }
        }
        fs::write(&path, serde_json::to_vec(&installed).unwrap()).unwrap();
        configure(&fixture, vendor, InstallAction::Uninstall).unwrap();
        let after: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(after["hooks"][event], json!([original]));
    }
}

fn invoke_with_umask(root: &Path, vendor: &str, action: &str) {
    let stderr = root.join("umask-stderr");
    let mut command = Command::new(env!("CARGO_BIN_EXE_arcctl"));
    command
        .args([
            "hooks",
            action,
            vendor,
            "--scope",
            "workspace",
            "--workspace",
        ])
        .arg(root)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(fs::File::create(&stderr).unwrap()));
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    unsafe {
        command.pre_exec(|| {
            libc::umask(0o777);
            Ok(())
        });
    }
    let mut child = OwnedChild::spawn(&mut command, "restrictive umask installer");
    let status = child.wait();
    assert!(
        status.success(),
        "{vendor} {action}: {}",
        fs::read_to_string(stderr).unwrap()
    );
}

#[test]
fn restrictive_umask_keeps_created_files_reopenable_by_the_owner() {
    for vendor in ["claude", "cursor"] {
        let fixture = Fixture::new();
        let path = fixture.settings(vendor);
        invoke_with_umask(&fixture.0, vendor, "install");
        let lock = if vendor == "claude" {
            path.with_file_name("settings.json.arc-claude-lock")
        } else {
            path.with_file_name(".hooks.json.arc.lock")
        };
        for file in [&path, &lock] {
            assert_eq!(
                fs::metadata(file).unwrap().permissions().mode() & 0o600,
                0o600
            );
            fs::read(file).unwrap();
        }
        if vendor == "claude" {
            let id = path.with_file_name("settings.json.arc-claude-id");
            assert_eq!(
                fs::metadata(&id).unwrap().permissions().mode() & 0o777,
                0o600
            );
            fs::read(id).unwrap();
        }
        let installed = fs::read(&path).unwrap();
        invoke_with_umask(&fixture.0, vendor, "install");
        assert_eq!(fs::read(&path).unwrap(), installed);
        invoke_with_umask(&fixture.0, vendor, "uninstall");
        let uninstalled = fs::read(&path).unwrap();
        invoke_with_umask(&fixture.0, vendor, "uninstall");
        assert_eq!(fs::read(&path).unwrap(), uninstalled);
    }
}

#[cfg(target_os = "macos")]
fn captured_command(
    root: &Path,
    mut command: Command,
) -> (std::process::ExitStatus, String, String) {
    let id = arc_core::process_identity::random_job_id().unwrap();
    let stdout = root.join(format!("{id}.out"));
    let stderr = root.join(format!("{id}.err"));
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(fs::File::create(&stdout).unwrap()))
        .stderr(Stdio::from(fs::File::create(&stderr).unwrap()));
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let mut child = OwnedChild::spawn(&mut command, "macOS installer ACL probe");
    let status = child.wait();
    let output = fs::read_to_string(&stdout).unwrap();
    let error = fs::read_to_string(&stderr).unwrap();
    fs::remove_file(stdout).unwrap();
    fs::remove_file(stderr).unwrap();
    (status, output, error)
}

#[cfg(target_os = "macos")]
#[test]
fn inherited_owner_deny_acl_fails_without_changing_existing_settings() {
    for vendor in ["claude", "cursor"] {
        let fixture = Fixture::new();
        let path = fixture.settings(vendor);
        fs::write(&path, b"{\"untouched\":true}").unwrap();
        let mut id = Command::new("/usr/bin/id");
        id.arg("-un");
        let (status, user, error) = captured_command(&fixture.0, id);
        assert!(status.success(), "cannot identify ACL user: {error}");
        let mut chmod = Command::new("/bin/chmod");
        chmod
            .args([
                "+a",
                &format!(
                    "user:{} deny read,write,file_inherit,only_inherit",
                    user.trim()
                ),
            ])
            .arg(path.parent().unwrap());
        let (status, _, error) = captured_command(&fixture.0, chmod);
        assert!(status.success(), "cannot install fixture ACL: {error}");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut command = Command::new(env!("CARGO_BIN_EXE_arcctl"));
            command
                .args([
                    "hooks",
                    "install",
                    vendor,
                    "--scope",
                    "workspace",
                    "--workspace",
                ])
                .arg(&fixture.0)
                .env_clear()
                .env("HOME", &fixture.0);
            let (status, _, error) = captured_command(&fixture.0, command);
            assert!(
                !status.success(),
                "{vendor} unexpectedly accepted owner-denying ACL"
            );
            assert!(!error.is_empty());
            assert_eq!(fs::read(&path).unwrap(), b"{\"untouched\":true}");
            let lock = if vendor == "claude" {
                "settings.json.arc-claude-lock"
            } else {
                ".hooks.json.arc.lock"
            };
            let lock = path.with_file_name(lock);
            assert_eq!(
                fs::read(&lock).unwrap_err().kind(),
                std::io::ErrorKind::PermissionDenied
            );
            let mut list = Command::new("/bin/ls");
            list.arg("-le").arg(lock);
            let (status, output, _) = captured_command(&fixture.0, list);
            assert!(status.success());
            assert!(output.contains("inherited deny read,write"), "{output}");
        }));
        let mut cleanup = Command::new("/bin/chmod");
        cleanup.arg("-N").arg(path.parent().unwrap());
        assert!(captured_command(&fixture.0, cleanup).0.success());
        for entry in fs::read_dir(path.parent().unwrap()).unwrap() {
            let mut cleanup = Command::new("/bin/chmod");
            cleanup.arg("-N").arg(entry.unwrap().path());
            assert!(captured_command(&fixture.0, cleanup).0.success());
        }
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
    }
}
