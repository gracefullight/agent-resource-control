//! Owned fixture processes: capture output without pipe deadlocks and reap on failure.
use std::{
    fs::{self, File},
    os::unix::process::CommandExt,
    path::Path,
    process::{Child, Command, Output, Stdio},
    time::{Duration, Instant},
};

struct OwnedChild(Child);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            // SAFETY: each fixture child was placed in its own process group.
            unsafe { libc::kill(-(self.0.id() as i32), libc::SIGKILL) };
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

pub(super) fn output(command: &mut Command, root: &Path, label: &str, guard: Duration) -> Output {
    assert!(guard >= Duration::from_secs(60));
    let id = arc_core::process_identity::random_job_id().unwrap();
    let stdout = root.join(format!("{id}.stdout"));
    let stderr = root.join(format!("{id}.stderr"));
    command
        .stdout(Stdio::from(File::create(&stdout).unwrap()))
        .stderr(Stdio::from(File::create(&stderr).unwrap()))
        .process_group(0);
    let mut child = OwnedChild(command.spawn().expect("spawn owned fixture child"));
    let deadline = Instant::now() + guard;
    let status = loop {
        match child.0.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::yield_now(),
            Ok(None) => panic!(
                "{label}: fixture child did not exit; fixture retained at {}",
                root.display()
            ),
            Err(error) => panic!("{label}: cannot observe fixture child exit: {error}"),
        }
    };
    let output = Output {
        status,
        stdout: fs::read(&stdout).unwrap(),
        stderr: fs::read(&stderr).unwrap(),
    };
    fs::remove_file(stdout).unwrap();
    fs::remove_file(stderr).unwrap();
    output
}
