use super::{
    cli::{accept, child_command, wait},
    fail_open::{arc_stderr, receipt},
    runtime::Fixture,
    support,
};
use std::{
    io::Write,
    os::unix::{net::UnixListener, process::CommandExt},
    process::Stdio,
};
#[test]
fn fallback_preserves_inherited_ignored_and_blocked_sigchld() {
    let _serial = support::serial();
    let f = Fixture::new();
    for ignored in [false, true] {
        for blocked in [false, true] {
            let mut outputs = Vec::new();
            for wrapped in [false, true] {
                let control = f
                    .root
                    .join(format!("chld-{ignored}-{blocked}-{wrapped}.sock"));
                let listener = UnixListener::bind(&control).unwrap();
                let mut c = child_command(&f, wrapped);
                c.env("ARC_FIXTURE_CONTROL", &control)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                unsafe {
                    c.pre_exec(move || {
                        let mut action: libc::sigaction = std::mem::zeroed();
                        action.sa_sigaction = if ignored {
                            libc::SIG_IGN
                        } else {
                            libc::SIG_DFL
                        };
                        action.sa_flags = libc::SA_NOCLDWAIT;
                        libc::sigemptyset(&mut action.sa_mask);
                        if libc::sigaction(libc::SIGCHLD, &action, std::ptr::null_mut()) < 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                        let mut mask = std::mem::zeroed();
                        libc::sigemptyset(&mut mask);
                        if blocked {
                            libc::sigaddset(&mut mask, libc::SIGCHLD);
                        }
                        if libc::sigprocmask(libc::SIG_SETMASK, &mask, std::ptr::null_mut()) < 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                        Ok(())
                    });
                }
                let mut child = c.spawn().unwrap();
                let (mut barrier, info) = accept(&listener);
                barrier.write_all(b"x").unwrap();
                assert_eq!(info["sigchld_ignored"], ignored);
                assert_eq!(info["sigchld_blocked"], blocked);
                assert_eq!(wait(&mut child).code(), Some(17));
                let out = child.wait_with_output().unwrap();
                outputs.push((
                    out.stdout,
                    if wrapped {
                        arc_stderr(&out.stderr)
                    } else {
                        out.stderr
                    },
                ));
            }
            assert_eq!(outputs[0], outputs[1]);
        }
    }
    receipt(
        "sigstate",
        serde_json::json!({"scenario":"sigstate","passed":true,"invocations":1,"ignored":[false,true],"blocked":[false,true]}),
    );
}
