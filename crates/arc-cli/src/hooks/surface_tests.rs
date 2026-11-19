//! Experiments on command-shell resolution; these do not provide production proof.
use super::*;

// Capture to files so a full pipe cannot block the process while the owned
// supervisor enforces its 120-second watchdog and cleans its descendants.
fn supervised_output(
    mut command: std::process::Command,
    fixture: &Fixture,
) -> std::process::Output {
    use std::process::{Output, Stdio};

    let id = arc_core::process_identity::random_job_id().unwrap();
    let stdout = fixture.0.join(format!("{id}.stdout"));
    let stderr = fixture.0.join(format!("{id}.stderr"));
    command
        .stdout(Stdio::from(fs::File::create(&stdout).unwrap()))
        .stderr(Stdio::from(fs::File::create(&stderr).unwrap()));
    let label = format!(
        "shell investigation (stdout: {}, stderr: {})",
        stdout.display(),
        stderr.display()
    );
    let mut child = super::super::fixture::OwnedChild::spawn(&mut command, &label);
    Output {
        status: child.wait(),
        stdout: fs::read(stdout).unwrap(),
        stderr: fs::read(stderr).unwrap(),
    }
}

#[test]
fn hook_path_does_not_prove_command_shell_function_resolution() {
    let fixture = Fixture::new();
    let arcctl = fixture.executable("arcctl");
    fs::write(&arcctl, "#!/bin/sh\nshift 2\nexec \"$@\"\n").unwrap();
    fixture.executable("cargo");
    let context = HookContext {
        executable: Some(arcctl),
    };
    let event: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/agent_hooks/claude-pre-tool-use.json"
    ))
    .unwrap();
    assert!(dispatch(Vendor::Claude, &event, &context).is_some());

    // The operator accepted that a shell function is bypassed by this route.
    let false_surface = VerifiedSurface;
    let rewritten = rewrite_literal("cargo build", &context, Some(&false_surface)).unwrap();
    let run = |command: &str| {
        let mut process = std::process::Command::new("/bin/bash");
        process
            .args([
                "-c",
                &format!("cargo() {{ printf 'function:%s' \"$1\"; }}; {command}"),
            ])
            .env_clear()
            .env("PATH", &fixture.0);
        supervised_output(process, &fixture)
    };
    let direct = run("cargo build");
    assert!(direct.status.success());
    assert_eq!(direct.stdout, b"function:build");
    assert!(!fixture.0.join("executed").exists());
    let wrapped = run(&rewritten);
    assert!(wrapped.status.success());
    assert!(wrapped.stdout.is_empty());
    assert_eq!(fs::read(fixture.0.join("executed")).unwrap(), b"x");
}
