//! Compare inherited SIGPIPE behavior with a native shell invocation.
use super::{cli::command, fail_open::arc_stderr, runtime::Fixture, support};
use std::{fs, os::unix::process::CommandExt, process::Command};

fn compare(ignored: bool) {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut outputs = Vec::new();
    for wrapped in [false, true] {
        let marker = f.root.join(format!("sigpipe-{ignored}-{wrapped}"));
        let mut c = if wrapped {
            let mut c = command(&f);
            c.args(["exec", "--", "/bin/sh"]);
            c
        } else {
            Command::new("/bin/sh")
        };
        c.args([
            "-c",
            "printf 'call\\n' >> \"$ARC_TEST_MARKER\"; kill -s USR1 $$; printf blocked; kill -s PIPE $$; printf survived; exit 17",
        ])
        .env("ARC_TEST_MARKER", &marker);
        unsafe {
            c.pre_exec(move || {
                let disposition = if ignored {
                    libc::SIG_IGN
                } else {
                    libc::SIG_DFL
                };
                if libc::signal(libc::SIGPIPE, disposition) == libc::SIG_ERR {
                    return Err(std::io::Error::last_os_error());
                }
                let mut set = std::mem::zeroed();
                libc::sigemptyset(&mut set);
                libc::sigaddset(&mut set, libc::SIGUSR1);
                if libc::sigprocmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let out = c.output().unwrap();
        assert_eq!(fs::read_to_string(marker).unwrap(), "call\n");
        outputs.push(out);
    }
    use std::os::unix::process::ExitStatusExt;
    if ignored {
        assert_eq!(outputs[0].status.code(), Some(17));
        assert_eq!(outputs[0].stdout, b"blockedsurvived");
    } else {
        assert_eq!(outputs[0].status.signal(), Some(libc::SIGPIPE));
        assert_eq!(outputs[0].stdout, b"blocked");
    }
    assert_eq!(outputs[0].status, outputs[1].status);
    assert_eq!(outputs[0].stdout, outputs[1].stdout);
    assert_eq!(outputs[0].stderr, arc_stderr(&outputs[1].stderr));
}

#[test]
fn fallback_preserves_inherited_ignored_sigpipe() {
    compare(true);
}

#[test]
fn fallback_preserves_inherited_default_sigpipe() {
    compare(false);
}
