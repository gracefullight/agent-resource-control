//! Deterministic synthetic stream bytes exercise the production Swift admission path.
//! This complements, and never substitutes for, the real SwiftPM comparisons.
use super::super::*;
use std::{
    fs::{self, OpenOptions},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    process::Stdio,
};

const STDOUT: &str = concat!(
    "Building for debugging...\n[1/2] Fixture\n[2/2] FixtureTests-product\n",
    "stdout before\n<OUT>\nstdout marker\n</OUT>\nstdout after\n"
);
const STDERR: &str = concat!(
    "Building for debugging...\n[1/2] Fixture\n[2/2] FixtureTests-product\n",
    "stderr before\n<ERR>\nstderr marker\n</ERR>\nstderr after\n"
);

struct Tools(PathBuf);
impl RunnerTools for Tools {
    fn shims(&self) -> &Path {
        &self.0
    }
}
impl Drop for Tools {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn exact(expected: [&[u8]; 2], actual: [&[u8]; 2]) {
    assert_eq!(expected[0], actual[0], "stdout bytes and order");
    assert_eq!(expected[1], actual[1], "stderr bytes and order");
}

pub fn check(read: Read) {
    let directory = root()
        .join("target/traffic-native-tools/stream-forwarding")
        .join(arc_core::process_identity::random_job_id().unwrap());
    fs::create_dir_all(&directory).unwrap();
    let script = directory.join("swift");
    write(
        &script,
        format!(
            "#!/bin/sh\nprintf 'once\\n' >> \"$TBS_PROJECT/launches\" || exit 1\nprintf '%s' '{STDOUT}'\nprintf '%s' '{STDERR}' >&2\nif [ \"${{STREAM_HOLD:-}}\" = 1 ]; then\n  /usr/bin/mkfifo \"$TBS_PROJECT/gate\" || exit 1\n  IFS= read -r line < \"$TBS_PROJECT/gate\" || exit 1\nfi\n"
        ),
    );
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    let case = Case::with_tools(
        "synthetic-swift-stream-forwarding",
        read,
        false,
        Tools(directory),
        json!({"kind":"synthetic stream forwarding; not SwiftPM"}),
        json!([]),
    );
    for form in [Form::SwiftBuild, Form::SwiftTest] {
        let verb = if form == Form::SwiftBuild {
            "build"
        } else {
            "test"
        };
        let direct = case.artifacts.join(format!("direct-{}", form.id()));
        let managed = case.artifacts.join(format!("managed-{}", form.id()));
        fs::create_dir_all(&direct).unwrap();
        fs::create_dir_all(&managed).unwrap();
        let output = Command::new(&script)
            .arg(verb)
            .current_dir(&direct)
            .env("TBS_PROJECT", &direct)
            .env_remove("STREAM_HOLD")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(output.status.success());
        exact(
            [STDOUT.as_bytes(), STDERR.as_bytes()],
            [&output.stdout, &output.stderr],
        );
        assert_eq!(log(&direct.join("launches")), "once\n");
        write(&direct.join("stdout"), &output.stdout);
        write(&direct.join("stderr"), &output.stderr);

        let mut run = Run::start_with(&case, &managed, &["swift", verb], |cmd| {
            cmd.env("STREAM_HOLD", "1");
        });
        eventually(|| {
            assert!(run.wrapper.child.try_wait().unwrap().is_none());
            match OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(managed.join("gate"))
            {
                Ok(file) => {
                    run.fifo = Some(file);
                    true
                }
                Err(e) if matches!(e.raw_os_error(), Some(libc::ENOENT | libc::ENXIO)) => false,
                Err(e) => panic!("synthetic forwarding gate: {e}"),
            }
        });
        let id = case.job(run.wrapper.child.id());
        eventually(|| case.row(&id)["state"] == "RUNNING");
        let child = case.row(&id)["child_pid"].as_u64().unwrap() as u32;
        run.wrapper
            .own_process(arc_core::process_identity::ProcessIdentity::capture(child).unwrap());
        run.finish(0);
        let stdout = fs::read(managed.join("stdout")).unwrap();
        let stderr = fs::read(managed.join("stderr")).unwrap();
        exact([&output.stdout, &output.stderr], [&stdout, &stderr]);
        assert_eq!(log(&managed.join("launches")), "once\n");
        let facts = case.terminal(&run, form.class(), form.reason(), 0);
        case.save(
            &format!("{}-synthetic-streams", form.id()),
            json!({"kind":"synthetic", "stdoutSha256":sha(&managed.join("stdout")),
                "stderrSha256":sha(&managed.join("stderr")), "exactPerStream":true,
                "executions":1,"outcome":facts}),
        );
    }
    case.done();
}

#[test]
fn synthetic_swift_stream_forwarding_detects_progress_loss_reordering_and_corruption() {
    let expected = [STDOUT.as_bytes(), STDERR.as_bytes()];
    exact(expected, expected);
    for stream in 0..2 {
        let original = if stream == 0 { STDOUT } else { STDERR };
        for changed in [
            original.replace("[1/2] Fixture\n", ""),
            original.replace("[2/2] FixtureTests-product\n", ""),
            original.replace(
                "[1/2] Fixture\n[2/2] FixtureTests-product\n",
                "[2/2] FixtureTests-product\n[1/2] Fixture\n",
            ),
            original.replace("[1/2] Fixture\n", "[1/2] FiXture\n"),
            original.replace(" before\n", " beFore\n"),
        ] {
            let actual = if stream == 0 {
                [changed.as_bytes(), STDERR.as_bytes()]
            } else {
                [STDOUT.as_bytes(), changed.as_bytes()]
            };
            assert!(std::panic::catch_unwind(|| exact(expected, actual)).is_err());
        }
    }
}
