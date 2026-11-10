//! Snapshot native state before Rust initializes standard descriptors and SIGPIPE.
use std::{
    io,
    os::unix::process::CommandExt,
    process::Command,
    sync::atomic::{AtomicI32, AtomicU8, Ordering},
};
static CLOSED: AtomicU8 = AtomicU8::new(0);
// -1 means the loader-time query failed; 0 and 1 represent default and ignore.
static PIPE_IGNORED: AtomicI32 = AtomicI32::new(-1);
extern "C" fn capture() {
    let mut mask = 0;
    for fd in 0..3 {
        // SAFETY: F_GETFD only observes the inherited descriptor table.
        if unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
            mask |= 1 << fd;
        }
    }
    CLOSED.store(mask, Ordering::Relaxed);
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    if unsafe { libc::sigaction(libc::SIGPIPE, std::ptr::null(), &mut action) } == 0 {
        PIPE_IGNORED.store(
            i32::from(action.sa_sigaction == libc::SIG_IGN),
            Ordering::Relaxed,
        );
    }
}
// Native loaders call these before Rust's generated main/runtime initializer.
#[used]
#[cfg_attr(target_os = "macos", unsafe(link_section = "__DATA,__mod_init_func"))]
#[cfg_attr(target_os = "linux", unsafe(link_section = ".init_array"))]
static INITIALIZE: extern "C" fn() = capture;

pub(crate) fn restore_in_child(command: &mut Command) {
    let mask = CLOSED.load(Ordering::Relaxed);
    let pipe_ignored = PIPE_IGNORED.load(Ordering::Relaxed);
    // SAFETY: only native close calls run after fork. This hook is registered
    // after terminal handoff, and never touches ARC's close-on-exec descriptors.
    unsafe {
        command.pre_exec(move || {
            for fd in 0..3 {
                if mask & (1 << fd) != 0 {
                    libc::close(fd);
                }
            }
            if pipe_ignored < 0 {
                return Err(io::Error::from_raw_os_error(libc::EIO));
            }
            restore_sigpipe(pipe_ignored != 0)?;
            Ok(())
        });
    }
}

fn restore_sigpipe(ignored: bool) -> io::Result<()> {
    // Command resets SIGPIPE to default before its pre-exec hooks. An ignored
    // disposition survives exec; all other inherited dispositions become default.
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    action.sa_sigaction = if ignored {
        libc::SIG_IGN
    } else {
        libc::SIG_DFL
    };
    unsafe {
        libc::sigemptyset(&mut action.sa_mask);
        if libc::sigaction(libc::SIGPIPE, &action, std::ptr::null_mut()) != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::restore_sigpipe;
    use std::{
        io,
        os::unix::process::{CommandExt, ExitStatusExt},
        process::Command,
    };

    #[test]
    fn child_hook_restores_sigpipe_after_command_setup() {
        for ignored in [false, true] {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", "kill -s PIPE $$; printf survived; exit 17"]);
            unsafe { command.pre_exec(move || restore_sigpipe(ignored)) };
            let out = command.output().unwrap();
            if ignored {
                assert_eq!(out.status.code(), Some(17));
                assert_eq!(out.stdout, b"survived");
            } else {
                assert_eq!(out.status.signal(), Some(libc::SIGPIPE));
                assert!(out.stdout.is_empty());
            }
        }
    }

    #[test]
    fn rust_runtime_ignores_sigpipe_and_command_resets_it_before_hooks() {
        let mut runtime_action: libc::sigaction = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { libc::sigaction(libc::SIGPIPE, std::ptr::null(), &mut runtime_action) },
            0
        );
        assert_eq!(runtime_action.sa_sigaction, libc::SIG_IGN);
        let mut fds = [-1; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        let mut command = Command::new("/bin/sh");
        command.args(["-c", ":"]);
        unsafe {
            command.pre_exec(move || {
                let mut action: libc::sigaction = std::mem::zeroed();
                if libc::sigaction(libc::SIGPIPE, std::ptr::null(), &mut action) != 0 {
                    return Err(io::Error::last_os_error());
                }
                let is_default = u8::from(action.sa_sigaction == libc::SIG_DFL);
                if libc::write(fds[1], (&is_default as *const u8).cast(), 1) != 1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        assert!(command.status().unwrap().success());
        let mut is_default = 0u8;
        assert_eq!(
            unsafe { libc::read(fds[0], (&mut is_default as *mut u8).cast(), 1) },
            1
        );
        unsafe {
            libc::close(fds[0]);
            libc::close(fds[1]);
        }
        assert_eq!(is_default, 1);
    }
}
