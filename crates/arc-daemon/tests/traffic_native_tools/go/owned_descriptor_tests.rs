use super::*;
use std::{io::Write, process::Stdio};

#[test]
fn go_tracks_a_detached_descendant_after_all_inherited_descriptors_close() {
    use std::{ffi::CString, fs, fs::OpenOptions, os::unix::fs::OpenOptionsExt};
    for timeout in [false, true] {
        let directory = std::env::temp_dir().join(format!(
            "arc-go-closed-fds-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::create_dir(&directory).unwrap();
        let fifo = CString::new(directory.join("gate").to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        let mut command = Command::new("/usr/bin/python3");
        command
            .args([
                "-c",
                r#"
import os, signal
pid=os.fork()
if pid == 0:
 os.setsid(); n=os.open('/dev/null',os.O_RDWR); os.dup2(n,1); os.dup2(n,2)
 os.closerange(3,1024); gate=os.open('gate',os.O_RDWR)
 with open('child.pending','w') as f: f.write(str(os.getpid()))
 os.rename('child.pending','child')
 os.read(gate,1); os._exit(0)
while not os.path.exists('child'): pass
os._exit(0)
"#,
            ])
            .current_dir(&directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = GoChild::spawn(&mut command).unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        while !directory.join("child").is_file() || !child.exited_without_reaping().unwrap() {
            assert!(
                Instant::now() < deadline,
                "closed-fd Go child was not ready"
            );
            thread::yield_now();
        }
        let pid: u32 = fs::read_to_string(directory.join("child"))
            .unwrap()
            .parse()
            .unwrap();
        let identity = crate::cleanup::capture(pid).unwrap().unwrap();
        struct Guard(ProcessIdentity);
        impl Drop for Guard {
            fn drop(&mut self) {
                crate::cleanup::report_errors(crate::cleanup::kill_tree(&self.0, 0));
            }
        }
        let _guard = Guard(identity.clone());
        child.observe().unwrap();
        assert!(
            child.identities().contains(&identity),
            "closed-fd child was not registered"
        );
        if timeout {
            assert_eq!(
                child.wait_until(Instant::now()).unwrap_err().kind(),
                io::ErrorKind::TimedOut
            );
        } else {
            let mut gate = OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(directory.join("gate"))
                .unwrap();
            gate.write_all(b"x").unwrap();
            assert!(child.wait_with_output().unwrap().status.success());
        }
        assert_ne!(
            crate::cleanup::capture(pid).unwrap().as_ref(),
            Some(&identity)
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
