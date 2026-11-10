use super::{NONTERMINAL, TERMINAL};

#[test]
fn diagnostic_thread_inherits_blocked_signals_and_parent_mask_is_restored() {
    fn mask() -> libc::sigset_t {
        let mut result = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, std::ptr::null(), &mut result) },
            0
        );
        result
    }
    let before = mask();
    let worker = super::super::with_blocked_signals(|| {
        let inside = mask();
        assert_eq!(unsafe { libc::sigismember(&inside, libc::SIGTERM) }, 1);
        std::thread::spawn(mask)
    })
    .unwrap();
    let parent = mask();
    let child = worker.join().unwrap();
    for signal in NONTERMINAL
        .into_iter()
        .chain(TERMINAL)
        .chain([libc::SIGCHLD, libc::SIGCONT])
    {
        assert_eq!(
            unsafe { libc::sigismember(&child, signal) },
            1,
            "signal {signal}"
        );
        assert_eq!(unsafe { libc::sigismember(&before, signal) }, unsafe {
            libc::sigismember(&parent, signal)
        });
    }
}
