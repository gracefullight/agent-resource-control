//! Rust initializes absent standard descriptors as /dev/null before main. Take
//! an allocation-free native snapshot first, then restore the caller's exact
//! closed-descriptor state only for the directly executed child.
use std::{
    os::unix::process::CommandExt,
    process::Command,
    sync::atomic::{AtomicU8, Ordering},
};
static CLOSED: AtomicU8 = AtomicU8::new(0);
extern "C" fn capture() {
    let mut mask = 0;
    for fd in 0..3 {
        // SAFETY: F_GETFD only observes the inherited descriptor table.
        if unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
            mask |= 1 << fd;
        }
    }
    CLOSED.store(mask, Ordering::Relaxed);
}
// Native loaders call these before Rust's generated main/runtime initializer.
#[used]
#[cfg_attr(target_os = "macos", unsafe(link_section = "__DATA,__mod_init_func"))]
#[cfg_attr(target_os = "linux", unsafe(link_section = ".init_array"))]
static INITIALIZE: extern "C" fn() = capture;

pub(crate) fn restore_in_child(command: &mut Command) {
    let mask = CLOSED.load(Ordering::Relaxed);
    // SAFETY: only native close calls run after fork. This hook is registered
    // after terminal handoff, and never touches ARC's close-on-exec descriptors.
    unsafe {
        command.pre_exec(move || {
            for fd in 0..3 {
                if mask & (1 << fd) != 0 {
                    libc::close(fd);
                }
            }
            Ok(())
        });
    }
}
