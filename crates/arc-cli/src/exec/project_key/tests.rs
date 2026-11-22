//! Key derivation on temporary directories and the advertisement gate
//! against a lock held by another process. No repository is read and no tool
//! or daemon is started.
use super::*;
use std::{fs, path::PathBuf};

struct Directory(PathBuf);
impl Directory {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("arc-project-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn make(&self, relative: &str) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn key(path: &Path) -> String {
    derive(path).unwrap().as_str().to_owned()
}

#[test]
fn two_subdirectories_of_one_root_share_a_key_and_two_roots_differ() {
    let directory = Directory::new("roots");
    let (first, second) = (directory.make("first"), directory.make("second"));
    fs::create_dir(first.join(".git")).unwrap();
    // A linked worktree marks its root with a `.git` file.
    fs::write(second.join(".git"), "gitdir: elsewhere\n").unwrap();
    let (deep, other) = (directory.make("first/a/b"), directory.make("first/c"));
    assert_eq!(key(&deep), key(&other));
    assert_eq!(key(&deep), key(&first));
    assert_ne!(key(&first), key(&second));
    assert_eq!(key(&directory.make("second/x")), key(&second));

    let value = key(&deep);
    assert_eq!(value.len(), 64);
    assert!(value
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    // The digest is not the path and names no part of it.
    for part in ["first", "arc-project", "/"] {
        assert!(!value.contains(part), "{part}");
    }
}

#[test]
fn without_a_repository_the_working_directory_is_the_project() {
    let directory = Directory::new("plain");
    // Skip when an ancestor of the temporary directory is itself a project.
    if directory.0.ancestors().any(|a| a.join(".git").exists()) {
        return;
    }
    let (first, second) = (directory.make("one"), directory.make("two"));
    assert_ne!(key(&first), key(&second));
    assert_eq!(key(&first), key(&first));
    assert!(derive(&directory.0.join("missing")).is_none());
}

#[test]
fn every_spelling_of_one_directory_gives_the_same_key() {
    let directory = Directory::new("links");
    let root = directory.make("root");
    fs::create_dir(root.join(".git")).unwrap();
    let inside = directory.make("root/inside");
    std::os::unix::fs::symlink(&inside, directory.0.join("alias")).unwrap();
    assert_eq!(key(&directory.0.join("alias")), key(&root));
    assert_eq!(key(&inside.join("..").join("inside")), key(&root));
}

/// A process holding the advertisement lock beside `socket`, as the daemon
/// does. Dropping it ends the process, which releases the lock.
pub(in crate::exec) struct Holder {
    pub(in crate::exec) pid: libc::pid_t,
    release: libc::c_int,
}
impl Holder {
    fn lock(socket: &Path) -> Self {
        Self::lock_named(socket, ADVERTISEMENT)
    }

    /// Hold the lock on the advertisement file `name` beside `socket`.
    pub(in crate::exec) fn lock_named(socket: &Path, name: &str) -> Self {
        let path = socket.parent().unwrap().join(name);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)
            .unwrap();
        let (mut ready, mut hold) = ([0; 2], [0; 2]);
        assert_eq!(unsafe { libc::pipe(ready.as_mut_ptr()) }, 0);
        assert_eq!(unsafe { libc::pipe(hold.as_mut_ptr()) }, 0);
        // SAFETY: the child calls only async-signal-safe functions and exits.
        let pid = unsafe { libc::fork() };
        assert!(pid >= 0);
        if pid == 0 {
            unsafe {
                // A forked copy of a sibling test's descriptor keeps that
                // test's `flock` held (the cadence turn lock) until this
                // process ends. Keep only what this holder needs.
                for fd in 3..libc::getdtablesize().min(4096) {
                    if fd != file.as_raw_fd() && fd != ready[1] && fd != hold[0] {
                        libc::close(fd);
                    }
                }
                let mut lock: libc::flock = std::mem::zeroed();
                lock.l_type = libc::F_WRLCK as _;
                lock.l_whence = libc::SEEK_SET as _;
                let locked = libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &lock) == 0;
                libc::write(ready[1], [u8::from(locked)].as_ptr().cast(), 1);
                let mut byte = 0u8;
                libc::read(hold[0], (&mut byte as *mut u8).cast(), 1);
                libc::_exit(0);
            }
        }
        let mut locked = 0u8;
        unsafe {
            libc::close(ready[1]);
            libc::close(hold[0]);
            assert_eq!(libc::read(ready[0], (&mut locked as *mut u8).cast(), 1), 1);
            libc::close(ready[0]);
        }
        assert_eq!(locked, 1, "the holder took the lock");
        Self {
            pid,
            release: hold[1],
        }
    }
}
impl Drop for Holder {
    fn drop(&mut self) {
        let mut status = 0;
        unsafe {
            libc::close(self.release);
            libc::waitpid(self.pid, &mut status, 0);
        }
    }
}

pub(in crate::exec) fn peer(pids: &[u32]) -> Continuity {
    let continuity = Continuity::default();
    for pid in pids {
        continuity.record(Some(
            arc_core::process_identity::ProcessIdentity::capture(*pid).unwrap(),
        ));
    }
    continuity
}

#[test]
fn a_key_is_sent_only_to_the_daemon_that_holds_the_advertisement() {
    let directory = Directory::new("advertised");
    let socket = directory.0.join("arc.sock");
    let expected = derive(&std::env::current_dir().unwrap());
    assert!(expected.is_some());
    let this = std::process::id();

    // No file, then a file that nobody locks: an earlier daemon's leftover.
    assert_eq!(negotiated(&socket, &peer(&[this])), None);
    fs::write(directory.0.join(ADVERTISEMENT), "").unwrap();
    assert!(!advertised(&socket, this));
    assert_eq!(negotiated(&socket, &peer(&[this])), None);

    let holder = Holder::lock(&socket);
    let daemon = holder.pid as u32;
    assert!(advertised(&socket, daemon));
    assert_eq!(negotiated(&socket, &peer(&[daemon])), expected);
    // The lock is held, but not by the process that answered this run.
    assert!(!advertised(&socket, this));
    assert_eq!(negotiated(&socket, &peer(&[this])), None);
    // No authenticated peer, or two different ones, names no daemon.
    assert_eq!(negotiated(&socket, &peer(&[])), None);
    assert_eq!(negotiated(&socket, &peer(&[daemon, this])), None);

    drop(holder);
    assert!(!advertised(&socket, daemon));
}

#[test]
fn an_unusable_advertisement_sends_no_key() {
    let directory = Directory::new("unusable");
    let this = std::process::id();
    assert!(!advertised(Path::new("arc.sock"), this));
    // A directory or a link where the file belongs is never opened.
    let blocked = directory.make("blocked");
    fs::create_dir(blocked.join(ADVERTISEMENT)).unwrap();
    assert!(!advertised(&blocked.join("arc.sock"), this));
    let linked = directory.make("linked");
    let target = directory.make("target");
    let holder = Holder::lock(&target.join("arc.sock"));
    std::os::unix::fs::symlink(target.join(ADVERTISEMENT), linked.join(ADVERTISEMENT)).unwrap();
    assert!(advertised(&target.join("arc.sock"), holder.pid as u32));
    assert!(!advertised(&linked.join("arc.sock"), holder.pid as u32));
}
