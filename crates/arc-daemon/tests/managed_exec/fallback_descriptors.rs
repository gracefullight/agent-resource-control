//! A non-Rust child observes the actual inherited descriptor table.
use super::{
    cli::command,
    fail_open::{arc_stderr, receipt},
    fault::{Fault, MODES},
    runtime::Fixture,
    support,
};
use std::{
    fs,
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Command, Stdio},
};
#[test]
fn fallback_preserves_closed_standard_and_extra_inherited_descriptors() {
    let _serial = support::serial();
    let source = r#"import fcntl,json,os,sys
state=[]
for fd in range(3):
    try:
        fcntl.fcntl(fd,fcntl.F_GETFD)
        state.append(True)
    except OSError:
        state.append(False)
extra=os.read(9,4096)
with open(sys.argv[2],'a') as marker:
    marker.write(str(os.getpid())+'\n')
with open(sys.argv[1],'w') as report:
    json.dump({'standard':state,'extra':list(extra)},report)
sys.exit(17)
"#;
    for mode in MODES {
        let f = Fixture::new();
        let mut daemon = f.spawn("test");
        let fault = Fault::new(&f, mode);
        let inherited = f.root.join("input");
        fs::write(&inherited, b"inherited\0descriptor").unwrap();
        for mask in [0u8, 1, 2, 4, 7] {
            let mut observations = Vec::new();
            for wrapped in [false, true] {
                let file = fs::File::open(&inherited).unwrap();
                let fd = file.as_raw_fd();
                let report = f.root.join(format!("fds-{mask}-{wrapped}.json"));
                let mut c = if wrapped {
                    let mut c = command(&f);
                    c.args(["exec", "--", "python3"]);
                    c
                } else {
                    Command::new("python3")
                };
                let marker = f.root.join(format!("calls-{mask}-{wrapped}"));
                c.args(["-c", source])
                    .arg(&report)
                    .arg(&marker)
                    .env(arc_core::ARC_SOCKET_PATH, &fault.path)
                    .stdin(Stdio::null());
                unsafe {
                    c.pre_exec(move || {
                        if libc::dup2(fd, 9) < 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                        libc::fcntl(9, libc::F_SETFD, 0);
                        for i in 0..3 {
                            if mask & (1 << i) != 0 {
                                libc::close(i);
                            }
                        }
                        Ok(())
                    });
                }
                let out = c.output().unwrap();
                assert_eq!(out.status.code(), Some(17));
                assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 1);
                assert!(out.stdout.is_empty());
                assert!(if wrapped {
                    arc_stderr(&out.stderr)
                } else {
                    out.stderr
                }
                .is_empty());
                let observation: serde_json::Value =
                    serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
                assert_eq!(
                    observation["standard"],
                    serde_json::json!((0..3).map(|i| mask & (1 << i) == 0).collect::<Vec<_>>())
                );
                assert_eq!(
                    observation["extra"],
                    serde_json::json!(b"inherited\0descriptor".to_vec())
                );
                observations.push(observation);
            }
            assert_eq!(observations[0], observations[1]);
        }
        let report = fault.finish();
        let expected_event = match mode {
            "absent" | "refused" => None,
            "disconnect-before-read" => Some("accepted-before-read"),
            "disconnect-register" | "disconnect-claim" => Some("disconnected-before-commit"),
            "stall-register" | "stall-claim" | "shared-budget" => Some("stalled-before-commit"),
            "partial-register" => Some("partial-reply-withheld"),
            "committed-register" | "committed-claim" => Some("backend-ack-withheld"),
            "busy-register" | "busy-claim" => Some("injected-db-busy"),
            _ => unreachable!("unknown fault mode: {mode}"),
        };
        if let Some(expected) = expected_event {
            assert!(
                report.events.iter().any(|event| event["event"] == expected),
                "fault {mode} was not injected: {:?}",
                report.events
            );
        }
        daemon.stop(libc::SIGTERM);
        f.report();
    }
    receipt(
        "descriptors",
        serde_json::json!({"scenario":"descriptors","passed":true,"invocations":1,"masks":[0,1,2,4,7],"inheritedFd":9,"outageClasses":MODES,"invocationsPerCommand":1}),
    );
}
