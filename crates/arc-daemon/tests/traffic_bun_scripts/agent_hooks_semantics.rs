//! Compare a shell's direct and production-hook commands with a real daemon.
use super::*;

pub(super) fn compare(case: &Case, vendor: &str, kind: Kind, literal: &str) {
    let project = case.artifacts.join(format!("semantics-{vendor}-{kind:?}"));
    fs::create_dir(&project).unwrap();
    if kind == Kind::Vite {
        write(
            &project.join("package.json"),
            json!({"scripts":{"build":"vite build"}}).to_string(),
        );
    }
    let bin = project.join("bin");
    fs::create_dir(&bin).unwrap();
    let program = if kind == Kind::Cargo { "cargo" } else { "bun" };
    let tool = bin.join(program);
    write(&tool, b"#!/bin/sh\nprintf 'start\\n' >> \"$WITNESS\"\nprintf 'program=%s\\ncwd=%s\\nvalue=%s\\nlineage=%s\\npath=%s\\nargs=' \"$0\" \"$PWD\" \"$PRIVATE_VALUE\" \"$ARC_TRAFFIC_LINEAGE\" \"$PATH\"\nprintf '<%s>' \"$@\"\nprintf '\\nstdin='\n/bin/cat\nprintf 'child-stderr\\n' >&2\nexit 17\n");
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
    let path =
        std::env::join_paths([bin.as_path(), Path::new("/usr/bin"), Path::new("/bin")]).unwrap();
    let rewritten = transform(&project, vendor, literal, &path);
    let input = project.join("stdin");
    write(&input, b"stdin bytes\n");
    let starts = project.join("starts");
    let mut outputs = Vec::new();
    for (index, command) in [literal, rewritten.as_str()].into_iter().enumerate() {
        let stdout = project.join(format!("semantics-{index}.stdout"));
        let stderr = project.join(format!("semantics-{index}.stderr"));
        let mut shell = Command::new("/bin/zsh");
        shell
            .args(["-c", &format!("exec {command}")])
            .env_clear()
            .env("PATH", &path)
            .env("HOME", &case.fixture.root)
            .env(arc_core::ARC_SOCKET_PATH, &case.fixture.socket)
            .env(arc_core::ARC_DB_PATH, &case.fixture.database)
            .env("PRIVATE_VALUE", "literal $HOME ' value")
            .env("ARC_TRAFFIC_LINEAGE", "supplied-lineage")
            .env("WITNESS", &starts)
            .current_dir(&project)
            .stdin(Stdio::from(File::open(&input).unwrap()))
            .stdout(Stdio::from(File::create(&stdout).unwrap()))
            .stderr(Stdio::from(File::create(&stderr).unwrap()));
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            shell.env("LLVM_PROFILE_FILE", profile);
        }
        let mut child = hooks::fixture::OwnedChild::spawn(&mut shell, "hook semantics");
        let pid = child.child_mut().id();
        let status = child.wait();
        let output = std::process::Output {
            status,
            stdout: fs::read(stdout).unwrap(),
            stderr: fs::read(stderr).unwrap(),
        };
        assert_eq!(output.status.code(), Some(17), "{output:?}");
        assert_eq!(log(&starts).lines().count(), index + 1);
        if index == 1 {
            let id = case.job(pid);
            eventually(|| case.row(&id)["state"] == "COMPLETED");
            let result = case.reconcile(&id, "BUILD", kind.reason());
            assert_eq!(result["sqlite"]["exit_code"], 17);
            assert_eq!(result["diagnostic"]["admission"]["permit_active"], false);
            assert_eq!(case.active()["permits"], 0);
        }
        outputs.push(output);
    }
    assert_eq!(outputs[0].stdout, outputs[1].stdout, "{vendor} {literal}");
    assert_eq!(outputs[0].stderr, outputs[1].stderr, "{vendor} {literal}");
    let stdout = String::from_utf8_lossy(&outputs[1].stdout);
    assert!(stdout.contains(&format!("program={}\n", tool.display())));
    assert!(stdout.contains(&format!("cwd={}\n", project.display())));
    assert!(stdout.contains("value=literal $HOME ' value\n"));
    assert!(stdout.contains("lineage=supplied-lineage\n"));
    assert!(stdout.ends_with("stdin=stdin bytes\n"));
}
