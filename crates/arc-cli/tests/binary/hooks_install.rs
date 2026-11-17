//! Exercise the real shared CLI dispatcher against isolated vendor settings.
use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

fn invoke(root: &Path, workspace: &Path, vendor: &str, scope: &str, action: &str) {
    let stderr = root.join("dispatcher-stderr");
    let mut command = Command::new(env!("CARGO_BIN_EXE_arcctl"));
    command
        .args(["hooks", action, vendor, "--scope", scope])
        .env_clear()
        .env("HOME", root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(fs::File::create(&stderr).unwrap()));
    if scope == "workspace" {
        command.arg("--workspace").arg(workspace);
    }
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("{vendor} {scope} {action} dispatcher did not exit");
        }
        std::thread::yield_now();
    };
    assert!(
        status.success(),
        "{vendor} {scope} {action}: {}",
        fs::read_to_string(stderr).unwrap()
    );
}

#[test]
fn real_cli_dispatches_both_installers_for_user_and_workspace() {
    let fixture = Fixture(std::path::PathBuf::from("/tmp").join(format!(
        "arc-dispatch-install-{}",
        arc_core::process_identity::random_job_id().unwrap()
    )));
    let home = fixture.0.join("home");
    let workspace = fixture.0.join("workspace");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    for (vendor, directory, filename, source, key) in [
        (
            "claude",
            ".claude",
            "settings.json",
            include_str!("../fixtures/agent_hooks/claude/settings.json"),
            "PreToolUse",
        ),
        (
            "cursor",
            ".cursor",
            "hooks.json",
            include_str!("../fixtures/agent_hooks/cursor/user.json"),
            "preToolUse",
        ),
    ] {
        for scope in ["user", "workspace"] {
            let root = if scope == "user" { &home } else { &workspace };
            let directory = root.join(directory);
            fs::create_dir_all(&directory).unwrap();
            let path = directory.join(filename);
            fs::write(&path, source).unwrap();
            let original: Value = serde_json::from_str(source).unwrap();
            invoke(&home, &workspace, vendor, scope, "install");
            let installed: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            let entries = installed["hooks"][key].as_array().unwrap();
            let previous = original["hooks"][key].as_array().unwrap();
            assert_eq!(&entries[..previous.len()], previous);
            assert_eq!(entries.len(), previous.len() + 1);
            invoke(&home, &workspace, vendor, scope, "install");
            assert_eq!(
                serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
                installed
            );
            invoke(&home, &workspace, vendor, scope, "uninstall");
            invoke(&home, &workspace, vendor, scope, "uninstall");
            assert_eq!(
                serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
                original
            );
        }
    }
}
