//! Barriers exercise the interval with admission granted but no child created.
use super::{isolated, pipe, read, write, Signals, TERMINAL};
use std::{
    process::Command,
    time::{Duration, Instant},
};

#[test]
fn shared_job_terminal_signals_after_admission_before_fork_cancel_execution() {
    isolated(
        "exec::signals::tests::handoff::shared_job_terminal_signals_after_admission_before_fork_cancel_execution",
        || {
            for signal in TERMINAL {
                let signals = Signals::install(true).unwrap();
                let admitted = pipe();
                let delivered = pipe();
                // The sender is a separate native process. It cannot deliver
                // until the wrapper acknowledges admission, and spawn cannot
                // begin until the group delivery syscall has completed.
                let sender = unsafe { libc::fork() };
                assert!(sender >= 0);
                if sender == 0 {
                    assert_eq!(read(admitted[0]), 1);
                    assert_eq!(unsafe { libc::kill(-libc::getpgrp(), signal) }, 0);
                    write(delivered[1], 1);
                    unsafe { libc::_exit(0) };
                }
                assert!(signals.admit_spawn());
                write(admitted[1], 1);
                assert_eq!(read(delivered[0]), 1);
                let mut sender_status = 0;
                assert_eq!(unsafe { libc::waitpid(sender, &mut sender_status, 0) }, sender);
                assert!(libc::WIFEXITED(sender_status));
                assert_eq!(libc::WEXITSTATUS(sender_status), 0);
                // libtest has multiple threads; wait for the wrapper's handler
                // to acknowledge delivery before taking the fork snapshot.
                let end = Instant::now() + Duration::from_secs(5);
                while !signals.interrupted() && Instant::now() < end {
                    std::thread::sleep(Duration::from_millis(1));
                }
                assert!(signals.interrupted(), "wrapper did not retain cancellation");

                let invoked = pipe();
                let mut command = Command::new("/bin/sh");
                command.args(["-c", "exit 17"]);
                signals.prepare_child(&mut command);
                use std::os::unix::process::CommandExt;
                unsafe {
                    command.pre_exec(move || {
                        // Reached only if the production pre-exec admission
                        // permits execution. A write here would be a failure.
                        libc::write(invoked[1], b"x".as_ptr().cast(), 1);
                        Ok(())
                    });
                }
                let result = command.spawn();
                if let Ok(mut child) = result {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("pre-child terminal cancellation admitted execution");
                }
                assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Interrupted);
                assert!(signals.interrupted());
                unsafe { libc::close(invoked[1]) };
                let mut byte = 0u8;
                assert_eq!(unsafe { libc::read(invoked[0], (&mut byte as *mut u8).cast(), 1) }, 0);
                for fd in admitted.into_iter().chain(delivered).chain([invoked[0]]) {
                    unsafe { libc::close(fd) };
                }
            }
        },
    );
}
