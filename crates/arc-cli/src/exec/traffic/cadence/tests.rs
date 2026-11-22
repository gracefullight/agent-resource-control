//! Virtual sleeps only: no assertion here depends on elapsed time.
use super::*;
use std::cell::{Cell, RefCell};

struct Directory(PathBuf);
impl Directory {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("arc-cadence-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn socket(&self) -> PathBuf {
        self.0.join("arc.sock")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn slept(cadence: &mut Cadence, reason: WaitReason) -> Vec<u64> {
    let slices = RefCell::new(Vec::new());
    cadence.pause(
        reason,
        &|slice| slices.borrow_mut().push(slice.as_millis() as u64),
        &|| false,
    );
    slices.into_inner()
}

#[test]
fn a_waiter_behind_the_head_backs_off_to_the_ceiling_in_slices() {
    let directory = Directory::new("backoff");
    let mut cadence = Cadence::new(&directory.socket());
    let totals: Vec<u64> = (0..9)
        .map(|_| {
            let slices = slept(&mut cadence, WaitReason::Fifo);
            assert!(slices.iter().all(|slice| *slice == 40), "{slices:?}");
            slices.iter().sum()
        })
        .collect();
    assert_eq!(totals, [40, 80, 160, 320, 640, 1280, 2560, 2560, 2560]);
}

#[test]
fn the_head_and_other_holds_keep_the_forty_millisecond_cadence() {
    let directory = Directory::new("head");
    for reason in [
        WaitReason::ClassWindow,
        WaitReason::CpuBudget,
        WaitReason::MemoryBudget,
    ] {
        let mut cadence = Cadence::new(&directory.socket());
        for _ in 0..3 {
            slept(&mut cadence, WaitReason::Fifo);
        }
        assert!(cadence.held.is_some());
        // Becoming the head resets the back-off and takes no turn.
        for _ in 0..80 {
            assert_eq!(slept(&mut cadence, reason), [40]);
            assert!(cadence.held.is_none());
        }
        assert_eq!(slept(&mut cadence, WaitReason::Fifo), [40]);
    }
}

#[test]
fn waiters_behind_the_head_poll_one_at_a_time() {
    let directory = Directory::new("turn");
    let mut first = Cadence::new(&directory.socket());
    let mut second = Cadence::new(&directory.socket());
    assert_eq!(slept(&mut first, WaitReason::Fifo), [40]);
    assert!(first.held.is_some());

    // The second waiter retries in slices while the first is polling. The
    // first poll is answered during the third retry and ends its turn.
    let retries = Cell::new(0);
    let first = RefCell::new(Some(first));
    second.pause(
        WaitReason::Fifo,
        &|slice| {
            assert_eq!(slice, STEP);
            retries.set(retries.get() + 1);
            if retries.get() == 4 {
                first.borrow_mut().take();
            }
        },
        &|| false,
    );
    assert_eq!(retries.get(), 4, "one delay slice and three lock retries");
    assert!(second.held.is_some());

    // The head never waits for a turn, even while one is taken.
    let mut head = Cadence::new(&directory.socket());
    assert_eq!(slept(&mut head, WaitReason::ClassWindow), [40]);
    // A reply ends the turn before the next delay starts.
    let mut third = Cadence::new(&directory.socket());
    let released = Cell::new(false);
    let second = RefCell::new(second);
    third.pause(
        WaitReason::Fifo,
        &|_| {
            if !released.replace(true) {
                slept(&mut second.borrow_mut(), WaitReason::ClassWindow);
            }
        },
        &|| false,
    );
    assert!(third.held.is_some() && second.borrow().held.is_none());
}

#[test]
fn a_signal_ends_a_long_delay_and_a_lock_wait_after_one_slice() {
    let directory = Directory::new("signal");
    let mut cadence = Cadence::new(&directory.socket());
    for _ in 0..7 {
        slept(&mut cadence, WaitReason::Fifo);
    }
    let slices = Cell::new(0);
    let count = |_: Duration| slices.set(slices.get() + 1);
    cadence.pause(WaitReason::Fifo, &count, &|| true);
    assert_eq!(slices.get(), 1);
    assert!(cadence.held.is_none());

    let mut holder = Cadence::new(&directory.socket());
    slept(&mut holder, WaitReason::Fifo);
    let mut blocked = Cadence::new(&directory.socket());
    slices.set(0);
    // Not cancelled during the 40 ms delay, cancelled during the first retry.
    blocked.pause(WaitReason::Fifo, &count, &|| slices.get() == 2);
    assert_eq!(slices.get(), 2);
    assert!(blocked.held.is_none() && holder.held.is_some());
}

#[test]
fn an_unusable_turn_file_leaves_the_plain_cadence() {
    let directory = Directory::new("unusable");
    // A directory where the file belongs, a link to it, and no parent at all.
    std::fs::create_dir(directory.0.join(TURN)).unwrap();
    let mut blocked = Cadence::new(&directory.socket());
    assert_eq!(slept(&mut blocked, WaitReason::Fifo), [40]);
    assert!(blocked.held.is_none());

    let linked = Directory::new("linked");
    std::fs::write(linked.0.join("target"), "").unwrap();
    std::os::unix::fs::symlink(linked.0.join("target"), linked.0.join(TURN)).unwrap();
    let mut followed = Cadence::new(&linked.socket());
    assert_eq!(slept(&mut followed, WaitReason::Fifo), [40]);
    assert!(followed.held.is_none());

    let mut bare = Cadence::new(Path::new("arc.sock"));
    assert_eq!(slept(&mut bare, WaitReason::Fifo), [40]);
    assert!(bare.held.is_none() && bare.turn.is_none());
}

#[test]
fn held_turn_lock_cannot_stop_a_waiter() {
    let directory = Directory::new("bound");
    // A wrapper that took its turn and never polls again, as a stopped
    // process would: it keeps the lock for the whole test.
    let mut holder = Cadence::new(&directory.socket());
    assert_eq!(slept(&mut holder, WaitReason::Fifo), [40]);
    assert!(holder.held.is_some());

    // The injected sleep fails the test instead of letting it spin, should
    // the wait for the turn ever be unbounded again.
    let waited = |cadence: &mut Cadence| -> u32 {
        let slices = Cell::new(0u32);
        cadence.pause(
            WaitReason::Fifo,
            &|slice| {
                assert_eq!(slice, STEP);
                slices.set(slices.get() + 1);
                assert!(slices.get() <= 1024, "a held turn lock stopped the waiter");
            },
            &|| false,
        );
        slices.get()
    };
    const _: () = assert!(TURN_SLICES <= 64, "the bound is at most 64 slices");
    let mut waiter = Cadence::new(&directory.socket());
    // Each wait is its delay, 1 to 64 slices, plus at most TURN_SLICES
    // slices for the turn. Then the waiter polls without the lock.
    for delay in [1, 2, 4, 8, 16, 32, 64, 64, 64] {
        assert_eq!(waited(&mut waiter), delay + TURN_SLICES);
        assert!(waiter.held.is_none() && holder.held.is_some());
    }
    // The bound costs a signal nothing: still one slice.
    let slices = Cell::new(0);
    waiter.pause(WaitReason::Fifo, &|_| slices.set(slices.get() + 1), &|| {
        slices.get() == 65
    });
    assert_eq!(slices.get(), 65, "64 delay slices and one lock retry");
    // Once the holder lets go, the waiter takes turns as before.
    drop(holder);
    assert_eq!(waited(&mut waiter), 64);
    assert!(waiter.held.is_some());
}
