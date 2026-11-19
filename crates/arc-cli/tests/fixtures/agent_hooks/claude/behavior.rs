//! Execute only after the fixture hook has returned its proposed command.
use super::{claude, forward_profile, local_event, process, Fixture, HookContext, VerifiedSurface};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::{Command, Output, Stdio},
    time::Duration,
};

pub(super) fn execution(fixture: &Fixture) {
    let script = b"#!/bin/sh\nprintf 'started\\n' >> \"$ARC_CLAUDE_STARTS\"\nprintf 'cwd=%s env=%s underscore=%s argv=%s,%s\\n' \"$PWD\" \"$FIXTURE_VALUE\" \"$_\" \"${1-}\" \"${2-}\"\nprintf 'profile=%s\\n' \"$LLVM_PROFILE_FILE\"\n/bin/cat\nprintf child-stderr >&2\nexit 17\n";
    for program in ["cargo", "bun"] {
        let path = fixture.bin.join(program);
        fs::write(&path, script).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let input = fixture.root.join("stdin");
    let starts = fixture.root.join("starts");
    fs::write(&input, b"stdin bytes\n").unwrap();
    let context = HookContext {
        executable: Some(env!("CARGO_BIN_EXE_arcctl").into()),
    };
    let surface = VerifiedSurface;
    let run = |command: &str| -> Output {
        let mut child = Command::new("/bin/sh");
        child
            .args(["-c", command])
            .env_clear()
            .env("PATH", &fixture.bin)
            .env("HOME", &fixture.home)
            .env("ARC_SOCKET_PATH", fixture.root.join("missing.sock"))
            .env("ARC_CLAUDE_STARTS", &starts)
            .env("FIXTURE_VALUE", "controlled value")
            .env("ARC_TRAFFIC_LINEAGE", "controlled lineage")
            .current_dir(&fixture.workspace)
            .stdin(Stdio::from(fs::File::open(&input).unwrap()));
        forward_profile(&mut child);
        process::output(
            &mut child,
            &fixture.root,
            "command execution",
            Duration::from_secs(90),
        )
    };
    for (index, literal) in ["cargo build", "bun run build"].iter().enumerate() {
        let mut event = local_event(fixture);
        event["tool_input"]["command"] = json!(literal);
        let response = claude::handle_verified(&event, &context, Some(&surface)).unwrap();
        let rewritten = response["hookSpecificOutput"]["updatedInput"]["command"]
            .as_str()
            .unwrap();
        // The hook itself only formed JSON. The two following child invocations
        // are made by this controlled fixture, after inspecting its response.
        assert_eq!(
            fs::read_to_string(&starts)
                .unwrap_or_default()
                .lines()
                .count(),
            index * 2
        );
        let direct = run(literal);
        assert_eq!(
            fs::read_to_string(&starts).unwrap().lines().count(),
            index * 2 + 1
        );
        let wrapped = run(rewritten);
        assert_eq!(
            fs::read_to_string(&starts).unwrap().lines().count(),
            index * 2 + 2
        );
        assert_eq!(direct.status.code(), Some(17));
        assert_eq!(wrapped.status.code(), direct.status.code());
        assert_eq!(wrapped.stdout, direct.stdout);
        let profile = std::env::var_os("LLVM_PROFILE_FILE").unwrap();
        let mut expected = b"profile=".to_vec();
        expected.extend_from_slice(std::os::unix::ffi::OsStrExt::as_bytes(profile.as_os_str()));
        expected.push(b'\n');
        assert!(wrapped
            .stdout
            .windows(expected.len())
            .any(|line| line == expected));
        assert!(wrapped
            .stderr
            .windows(direct.stderr.len())
            .any(|bytes| bytes == direct.stderr));
    }
}
