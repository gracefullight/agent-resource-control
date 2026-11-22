use super::*;
use std::{
    io::{BufRead, BufReader},
    os::unix::process::CommandExt,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

/// Every child is killed and reaped when the test ends, pass or fail.
struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
/// A process this test does not reap (an orphan), killed by PID.
struct Orphan(u32);
impl Drop for Orphan {
    fn drop(&mut self) {
        // SAFETY: plain signal to a PID this test started.
        unsafe { libc::kill(self.0 as libc::pid_t, libc::SIGKILL) };
    }
}

fn spawn(command: &mut Command) -> Owned {
    Owned(command.stdin(Stdio::null()).spawn().unwrap())
}
fn identity(pid: u32) -> ProcessIdentity {
    ProcessIdentity::capture(pid).unwrap()
}
fn pids(tree: &[NativeProcess]) -> Vec<u32> {
    tree.iter().map(|p| p.identity.pid).collect()
}
/// Diagnostic watchdog only; correctness comes from the observed tree.
fn until(label: &str, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !check() {
        assert!(Instant::now() < deadline, "{label}: 60 s watchdog");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn ppid_descendants_are_read_with_their_identity_and_usage() {
    let mut shell = Command::new("/bin/sh");
    shell.args(["-c", "/bin/sleep 60 & echo $!; wait"]);
    let mut root = spawn(shell.stdout(Stdio::piped()));
    let mut line = String::new();
    BufReader::new(root.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let sleeper: u32 = line.trim().parse().unwrap();
    let _sleeper = Orphan(sleeper);
    let root_identity = identity(root.0.id());

    // A process that has just forked or exec'd truthfully reports zero
    // resident bytes until its image is paged in. Wait for that observed
    // state, then assert on the same read.
    let mut tree = Vec::new();
    until("root and child resident", || {
        tree = read_tree(&root_identity);
        let resident = |pid: u32| {
            tree.iter()
                .any(|p| p.identity.pid == pid && p.rss_bytes > 0)
        };
        resident(root_identity.pid) && resident(sleeper)
    });
    let read_root = tree.iter().find(|p| p.identity == root_identity).unwrap();
    assert_eq!(read_root.parent_pid, std::process::id());
    let child = tree.iter().find(|p| p.identity.pid == sleeper).unwrap();
    assert_eq!(child.identity, identity(sleeper));
    assert_eq!(child.parent_pid, root_identity.pid);
    assert!(read_root.rss_bytes > 0 && child.rss_bytes > 0);
    // The test process is the root's parent, never its member: the root does
    // not lead the test's process group, so that group is not read.
    assert!(!pids(&tree).contains(&std::process::id()));
}

#[test]
fn a_detached_group_member_is_read_through_the_group_the_root_leads() {
    let mut shell = Command::new("/bin/sh");
    shell
        .args([
            "-c",
            "/bin/sh -c '/bin/sleep 60 & echo $!'; exec /bin/sleep 60",
        ])
        .process_group(0);
    let mut root = spawn(shell.stdout(Stdio::piped()));
    let mut line = String::new();
    BufReader::new(root.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let orphan: u32 = line.trim().parse().unwrap();
    let _orphan = Orphan(orphan);
    let root_identity = identity(root.0.id());
    // The intermediate shell has exited; the orphan's parent is no member.
    until("orphan reparented", || {
        read_tree(&root_identity)
            .iter()
            .any(|p| p.identity.pid == orphan && p.parent_pid != root_identity.pid)
    });
    let tree = read_tree(&root_identity);
    let member = tree.iter().find(|p| p.identity.pid == orphan).unwrap();
    assert_eq!(member.process_group, root_identity.pid);
    assert_eq!(member.identity, identity(orphan));
}

#[test]
fn a_recycled_root_identity_and_an_outside_process_are_never_read() {
    let mut shell = Command::new("/bin/sh");
    shell.args(["-c", "/bin/sleep 60 & echo $!; wait"]);
    let mut root = spawn(shell.stdout(Stdio::piped()));
    let mut line = String::new();
    BufReader::new(root.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let _sleeper = Orphan(line.trim().parse().unwrap());
    let outside = spawn(Command::new("/bin/sleep").arg("60"));
    let live = identity(root.0.id());

    // The same PID under another start time is another process.
    let recycled = ProcessIdentity {
        start_token: (live.start_token.parse::<u64>().unwrap() + 1).to_string(),
        ..live.clone()
    };
    assert!(read_tree(&recycled).is_empty());
    let other_boot = ProcessIdentity {
        boot_id: "00000000-0000-0000-0000-000000000000".into(),
        ..live.clone()
    };
    assert!(read_tree(&other_boot).is_empty());
    assert!(!pids(&read_tree(&live)).contains(&outside.0.id()));
}

#[test]
fn an_exited_root_yields_nothing_rather_than_a_zero_reading() {
    let mut child = Command::new("/usr/bin/true").spawn().unwrap();
    let root = identity(child.id());
    child.wait().unwrap();
    assert!(read_tree(&root).is_empty());
}

#[test]
fn own_cpu_time_is_reported_in_nanoseconds() {
    // Burn measurable CPU, then compare with this process's getrusage time.
    let start = Instant::now();
    let mut x = 0u64;
    while start.elapsed() < Duration::from_millis(50) {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
    }
    std::hint::black_box(x);
    let me = identity(std::process::id());
    let process = read_tree(&me)
        .into_iter()
        .find(|p| p.identity == me)
        .unwrap();
    // SAFETY: rusage is a plain writable C struct.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    assert_eq!(unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) }, 0);
    let ns = |t: libc::timeval| t.tv_sec as u64 * 1_000_000_000 + t.tv_usec as u64 * 1000;
    let after = ns(usage.ru_utime) + ns(usage.ru_stime);
    // Read before getrusage: never above it, and the same unit (not ticks).
    assert!(process.cpu_time_ns > 0 && process.cpu_time_ns <= after);
    assert!(
        process.cpu_time_ns * 2 > after / 2,
        "{process:?} vs {after}"
    );
}
