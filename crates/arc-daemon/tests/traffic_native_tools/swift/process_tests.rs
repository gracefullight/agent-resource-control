//! Real POSIX descendants exercise the direct runner without requiring SwiftPM readiness.
use super::*;
use std::{
    ffi::{CStr, CString},
    fs::{self, File, OpenOptions},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    process::Stdio,
    sync::atomic::{AtomicU64, Ordering},
};

struct HeldTree {
    runner: TreeChild,
    directory: PathBuf,
    identities: Vec<ProcessIdentity>,
    // Keep a writer open without sending data so the captured cat blocks in read.
    _gate: File,
}

fn cat_waiting(pid: u32, parent: u32) -> bool {
    // Read the image/parent and thread state of this exact fixture child.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of_val(&info) as i32;
    let written = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    };
    if written != size || info.pbi_ppid != parent || !info.pbi_comm.contains(&0) {
        return false;
    }
    if unsafe { CStr::from_ptr(info.pbi_comm.as_ptr()) }.to_bytes() != b"cat" {
        return false;
    }
    let mut threads = [0u64; 16];
    // PROC_PIDLISTTHREADS returns native thread IDs, not PIDs.
    let bytes = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            6,
            0,
            threads.as_mut_ptr().cast(),
            std::mem::size_of_val(&threads) as i32,
        )
    };
    if bytes <= 0 || !(bytes as usize).is_multiple_of(std::mem::size_of::<u64>()) {
        return false;
    }
    let count = bytes as usize / std::mem::size_of::<u64>();
    if count >= threads.len() {
        return false;
    }
    threads[..count].iter().any(|thread| {
        let mut info: libc::proc_threadinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of_val(&info) as i32;
        let written = unsafe {
            libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDTHREADINFO,
                *thread,
                (&mut info as *mut libc::proc_threadinfo).cast(),
                size,
            )
        };
        written == size && info.pth_run_state == libc::TH_STATE_WAITING
    })
}

impl HeldTree {
    fn start() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = crate::suite::root().join(format!(
            "target/traffic-native-tools/swift-cleanup/{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        let fifo = CString::new(directory.join("gate").to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        let mut command = Command::new("/bin/sh");
        command.args(["-c", r#"
/bin/sh -c '/bin/cat "$ARC_SWIFT_TREE_DIR/gate" & child=$!; printf "%s %s\n" "$$" "$child" > "$ARC_SWIFT_TREE_DIR/branch.pending"; /bin/mv "$ARC_SWIFT_TREE_DIR/branch.pending" "$ARC_SWIFT_TREE_DIR/branch"; wait "$child"' &
wait
"#])
            .env("ARC_SWIFT_TREE_DIR", &directory)
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let runner = TreeChild::spawn(command).unwrap();
        let mut identities = None;
        let mut gate = None;
        crate::suite::eventually(|| {
            let Ok(marker) = fs::read_to_string(directory.join("branch")) else {
                return false;
            };
            let pids: Vec<u32> = marker
                .split_whitespace()
                .map(|pid| pid.parse().unwrap())
                .collect();
            assert_eq!(pids.len(), 2, "branch and cat PIDs required");
            let (branch, cat) = (pids[0], pids[1]);
            let Some(branch_identity) =
                crate::cleanup::child_identity(runner.root.pid, branch).unwrap()
            else {
                return false;
            };
            let Some(cat_identity) = crate::cleanup::child_identity(branch, cat).unwrap() else {
                return false;
            };
            if gate.is_none() {
                // A nonblocking writer opens only once the cat has opened the FIFO
                // for reading. Keep it open, but never write or release the cat.
                match OpenOptions::new()
                    .write(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open(directory.join("gate"))
                {
                    Ok(file) => gate = Some(file),
                    Err(error) if error.raw_os_error() == Some(libc::ENXIO) => return false,
                    Err(error) => panic!("open fixture FIFO writer: {error}"),
                }
            }
            if !cat_waiting(cat, branch)
                || crate::cleanup::capture(runner.root.pid).unwrap().as_ref() != Some(&runner.root)
                || crate::cleanup::child_identity(runner.root.pid, branch)
                    .unwrap()
                    .as_ref()
                    != Some(&branch_identity)
                || crate::cleanup::child_identity(branch, cat)
                    .unwrap()
                    .as_ref()
                    != Some(&cat_identity)
            {
                return false;
            }
            identities = Some(vec![runner.root.clone(), branch_identity, cat_identity]);
            true
        });
        Self {
            runner,
            directory,
            identities: identities.expect("root, native shell and blocked native cat required"),
            _gate: gate.expect("FIFO writer must remain open"),
        }
    }
}
fn assert_gone(identities: &[ProcessIdentity]) {
    for identity in identities {
        assert_ne!(
            crate::cleanup::capture(identity.pid).unwrap().as_ref(),
            Some(identity),
            "owned descendant survived cleanup: {identity:?}"
        );
    }
    assert!(crate::cleanup::take_unwind_errors().is_empty());
}

#[test]
fn swift_direct_timeout_terminates_and_reaps_the_owned_tree() {
    let mut tree = HeldTree::start();
    let error = tree.runner.wait_with_output(Duration::ZERO).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(tree.runner.cleaned);
    for identity in &tree.identities {
        assert!(tree.runner.owned.contains(identity));
    }
    assert_gone(&tree.identities);
    drop(tree.runner);
    fs::remove_dir_all(tree.directory).unwrap();
}

#[test]
fn swift_direct_unwind_cleans_descendants_before_losing_the_parent() {
    let tree = HeldTree::start();
    let identities = tree.identities.clone();
    let directory = tree.directory.clone();
    let result = std::panic::catch_unwind(move || {
        let _runner = tree.runner;
        panic!("intentional Swift direct fixture unwind");
    });
    assert!(result.is_err());
    assert_gone(&identities);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn swift_direct_retains_exited_parent_until_descendants_are_cleaned() {
    use std::os::unix::process::ExitStatusExt;
    let mut tree = HeldTree::start();
    // The native parent can exit while its descendants still hold both output pipes.
    signal(&tree.runner.root, libc::SIGTERM).unwrap();
    let output = tree
        .runner
        .wait_with_output(Duration::from_secs(60))
        .unwrap();
    assert_eq!(output.status.signal(), Some(libc::SIGTERM));
    for identity in &tree.identities {
        assert!(tree.runner.owned.contains(identity));
    }
    assert_gone(&tree.identities);
    drop(tree.runner);
    fs::remove_dir_all(tree.directory).unwrap();
}

#[test]
fn swift_direct_drains_complete_bytes_and_preserves_the_native_exit() {
    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            "printf 'stdout bytes'; printf 'stderr bytes' >&2; exit 7",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = TreeChild::spawn(command).unwrap();
    let output = child.wait_with_output(Duration::from_secs(60)).unwrap();
    assert_eq!(output.stdout, b"stdout bytes");
    assert_eq!(output.stderr, b"stderr bytes");
    assert_eq!(output.status.code(), Some(7));
    assert_gone(&child.owned);
}
