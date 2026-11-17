//! Watchdog and ownership for each fixture subprocess.
use std::{
    io::Read,
    os::unix::process::CommandExt,
    process::{Child, Command, Output, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const WATCHDOG: Duration = Duration::from_secs(120);

struct ChildGuard {
    child: Option<Child>,
    group: Option<i32>,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            if let Some(group) = self.group {
                // SAFETY: the group was created for this fixture process.
                unsafe {
                    libc::kill(-group, libc::SIGKILL);
                }
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

type Reader = JoinHandle<std::io::Result<Vec<u8>>>;

pub(super) struct MonitoredChild {
    guard: ChildGuard,
    stdout: Reader,
    stderr: Reader,
    done: mpsc::Sender<()>,
    watchdog: JoinHandle<()>,
    expired: Arc<AtomicBool>,
    operation: String,
}

impl MonitoredChild {
    pub(super) fn finish(mut self) -> Output {
        let status = self
            .guard
            .child
            .as_mut()
            .unwrap()
            .wait()
            .expect("wait for Cursor fixture child");
        let stdout = self.stdout.join().unwrap().unwrap();
        let stderr = self.stderr.join().unwrap().unwrap();
        self.done.send(()).ok();
        self.watchdog.join().unwrap();
        self.guard.child.take();
        assert!(
            !self.expired.load(Ordering::SeqCst),
            "Cursor fixture timed out waiting for {}",
            self.operation
        );
        Output {
            status,
            stdout,
            stderr,
        }
    }
}

pub(super) fn spawn(command: &mut Command, operation: &str) -> MonitoredChild {
    start(command, operation, false)
}

pub(super) fn output(command: &mut Command, operation: &str) -> Output {
    spawn(command, operation).finish()
}

pub(super) fn nested_output(command: &mut Command, operation: &str) -> Output {
    start(command, operation, true).finish()
}

fn start(command: &mut Command, operation: &str, nested: bool) -> MonitoredChild {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if !nested {
        command.process_group(0);
    }
    let mut child = command.spawn().expect("spawn Cursor fixture child");
    let pid = child.id() as i32;
    // Nested commands inherit the worker's group. The parent worker watchdog
    // can then stop the entire fixture if the worker itself stalls.
    let group = if nested { None } else { Some(pid) };
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let stdout = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let (done, receiver) = mpsc::channel();
    let expired = Arc::new(AtomicBool::new(false));
    let watchdog_flag = Arc::clone(&expired);
    let label = operation.to_owned();
    let watchdog = thread::spawn(move || {
        if matches!(
            receiver.recv_timeout(WATCHDOG),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            watchdog_flag.store(true, Ordering::SeqCst);
            eprintln!("Cursor fixture timed out waiting for {label}");
            // SAFETY: signal the owned group, or the direct nested child.
            unsafe {
                libc::kill(group.map_or(pid, |group| -group), libc::SIGKILL);
            }
        }
    });
    MonitoredChild {
        guard: ChildGuard {
            child: Some(child),
            group,
        },
        stdout,
        stderr,
        done,
        watchdog,
        expired,
        operation: operation.to_owned(),
    }
}
