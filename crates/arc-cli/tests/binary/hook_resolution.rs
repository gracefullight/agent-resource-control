//! Direct zsh and built production-hook comparisons for PATH and spawn errors.
use super::hook_scheduled::FixtureChild;
use serde_json::Value;
use std::{
    ffi::OsStr,
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from("/tmp").join(format!(
            "arc-hook-resolution-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("bin")).unwrap();
        fs::create_dir(root.join("missing")).unwrap();
        fs::create_dir(root.join("nonexecutable")).unwrap();
        symlink(&root, root.join("logical")).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"build":"vite build"}}"#,
        )
        .unwrap();
        fs::write(root.join("stdin"), b"stdin bytes\n").unwrap();
        for program in ["cargo", "bun"] {
            let tool = root.join("bin").join(program);
            fs::write(&tool, b"#!/bin/sh\nsaved_underscore=$_\nprintf 'start\\n' >> \"$WITNESS\"\nprintf 'program=%s\\ncwd=%s\\nunderscore=%s\\npath=%s\\nvalue=%s\\nargs=' \"$0\" \"$PWD\" \"$saved_underscore\" \"$PATH\" \"$PRIVATE_VALUE\"\nprintf '<%s>' \"$@\"\nprintf '\\nstdin='\n/bin/cat\nprintf child-stderr >&2\nexit 17\n").unwrap();
            fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
            symlink(&tool, root.join(program)).unwrap();
            let blocked = root.join("nonexecutable").join(program);
            fs::write(&blocked, b"#!/bin/sh\nexit 99\n").unwrap();
            fs::set_permissions(blocked, fs::Permissions::from_mode(0o644)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn hook(&self, vendor: &str, source: &str) -> String {
        let mut command = Command::new(env!("CARGO_BIN_EXE_arcctl"));
        command
            .args(["hooks", "run", vendor])
            .env_clear()
            .env("PATH", "/nonexistent")
            .env("HOME", &self.0)
            .current_dir(&self.0)
            .stdin(Stdio::piped());
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        let mut child = FixtureChild::spawn(command, &self.0);
        child.write_input(source.as_bytes());
        let output = child.output();
        assert!(output.status.success(), "{vendor}: {output:?}");
        assert!(output.stderr.is_empty());
        let response: Value = serde_json::from_slice(&output.stdout).unwrap();
        let input = if vendor == "claude" {
            assert!(response["hookSpecificOutput"]
                .get("permissionDecision")
                .is_none());
            &response["hookSpecificOutput"]["updatedInput"]
        } else {
            assert!(response.get("permission").is_none());
            &response["updated_input"]
        };
        input["command"].as_str().unwrap().to_owned()
    }

    fn shell(&self, command: &str, path: &OsStr) -> Output {
        self.shell_with_pwd(command, path, None)
    }

    fn shell_with_pwd(&self, command: &str, path: &OsStr, pwd: Option<&Path>) -> Output {
        let mut shell = Command::new("/bin/zsh");
        shell
            .args(["-c", &format!("exec {command}")])
            .env_clear()
            .env("PATH", path)
            .env("HOME", &self.0)
            .env("ARC_SOCKET_PATH", self.0.join("missing.sock"))
            .env("WITNESS", self.0.join("starts"))
            .env("PRIVATE_VALUE", "literal $HOME ' value")
            .current_dir(&self.0)
            .stdin(Stdio::from(fs::File::open(self.0.join("stdin")).unwrap()));
        if let Some(pwd) = pwd {
            shell.env("PWD", pwd);
        }
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            shell.env("LLVM_PROFILE_FILE", profile);
        }
        FixtureChild::spawn(shell, &self.0).output()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

#[test]
fn built_hooks_preserve_zsh_resolution_and_spawn_exit_codes() {
    let fixture = Fixture::new();
    for (vendor, literal, source) in [
        (
            "claude",
            "cargo build",
            include_str!("../fixtures/agent_hooks/claude-recorded-2.1.327-cargo-build.json"),
        ),
        (
            "claude",
            "bun run build",
            include_str!("../fixtures/agent_hooks/claude-recorded-2.1.327-bun-run-build.json"),
        ),
        (
            "cursor",
            "cargo build",
            include_str!(
                "../fixtures/agent_hooks/cursor/recorded/preToolUse-cargo-build-rewrite.json"
            ),
        ),
        (
            "cursor",
            "bun run build",
            include_str!(
                "../fixtures/agent_hooks/cursor/recorded/preToolUse-bun-run-build-rewrite.json"
            ),
        ),
    ] {
        let rewritten = fixture.hook(vendor, source);
        let route = super::event_key::route(vendor, &serde_json::from_str(source).unwrap());
        assert!(route.starts_with(" hook-exec --arc-event "), "{route}");
        assert!(rewritten.ends_with(&format!("{route}{literal}")));
        for (label, path) in [
            ("relative", "bin:/usr/bin:/bin"),
            ("empty", ":/usr/bin:/bin"),
            ("dot", ".:/usr/bin:/bin"),
            ("trailing", "bin/:/usr/bin:/bin"),
            ("dot-slash", "./:/usr/bin:/bin"),
        ] {
            let direct = fixture.shell(literal, OsStr::new(path));
            let wrapped = fixture.shell(&rewritten, OsStr::new(path));
            assert_eq!(direct.status.code(), Some(17), "{vendor} {literal} {label}");
            assert_eq!(wrapped.status.code(), direct.status.code());
            assert_eq!(wrapped.stdout, direct.stdout, "{vendor} {literal} {label}");
            assert!(
                super::fixture_stderr_matches(&direct.stderr, &wrapped.stderr),
                "{wrapped:?}"
            );
            let text = String::from_utf8_lossy(&wrapped.stdout);
            let program = literal.split(' ').next().unwrap();
            let expected = match label {
                "relative" => format!("{}/bin/{program}", fixture.0.display()),
                "trailing" => format!("{}/bin//{program}", fixture.0.display()),
                "dot-slash" => format!("{}/.//{program}", fixture.0.display()),
                _ => format!("{}/{program}", fixture.0.display()),
            };
            assert!(text.contains(&format!("underscore={expected}\n")), "{text}");
            assert!(text.contains("value=literal $HOME ' value\n"));
            assert!(text.ends_with("stdin=stdin bytes\n"));
        }
        let logical = fixture.0.join("logical");
        let direct =
            fixture.shell_with_pwd(literal, OsStr::new("bin:/usr/bin:/bin"), Some(&logical));
        let wrapped =
            fixture.shell_with_pwd(&rewritten, OsStr::new("bin:/usr/bin:/bin"), Some(&logical));
        assert_eq!(wrapped.status.code(), Some(17));
        assert_eq!(
            wrapped.stdout, direct.stdout,
            "{vendor} {literal} logical PWD"
        );
        assert!(super::fixture_stderr_matches(
            &direct.stderr,
            &wrapped.stderr
        ));
        assert!(String::from_utf8_lossy(&wrapped.stdout).contains(&format!(
            "underscore={}/bin/{}\n",
            logical.display(),
            literal.split(' ').next().unwrap()
        )));
        for (label, code) in [("missing", 127), ("nonexecutable", 126)] {
            let path = fixture.0.join(label);
            let direct = fixture.shell(literal, path.as_os_str());
            let wrapped = fixture.shell(&rewritten, path.as_os_str());
            assert_eq!(
                direct.status.code(),
                Some(code),
                "{vendor} {literal} {label}"
            );
            assert_eq!(
                wrapped.status.code(),
                direct.status.code(),
                "{vendor} {literal} {label}: {wrapped:?}"
            );
            assert!(wrapped.stdout.is_empty());
            assert!(String::from_utf8_lossy(&wrapped.stderr).contains("SPAWN_FAILED"));
        }
    }
    assert_eq!(
        fs::read_to_string(Path::new(&fixture.0).join("starts"))
            .unwrap()
            .lines()
            .count(),
        48
    );
}
