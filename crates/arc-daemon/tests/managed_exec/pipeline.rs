//! A native job controller compares a real producer/peer pipeline with ARC.
use super::{cli::binary, runtime::Fixture, support};
#[test]
fn native_foreground_pipeline_input_signals_stop_resume_and_exit17() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let result = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/managed_exec/pipeline.py"
        ))
        .arg(&f.root)
        .arg(binary("arc-exec-fixture"))
        .arg(binary("arcctl"))
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .output()
        .unwrap();
    eprintln!("{}", String::from_utf8_lossy(&result.stdout));
    eprintln!("{}", String::from_utf8_lossy(&result.stderr));
    daemon.stop(libc::SIGTERM);
    f.report();
    assert!(result.status.success(), "native pipeline comparison failed");
}

#[test]
fn native_session_leader_without_terminal_preserves_execution_and_exit17() {
    use std::{os::unix::process::CommandExt, process::Stdio};
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let mut observations = Vec::new();
    for managed in [false, true] {
        let listener = std::os::unix::net::UnixListener::bind(
            f.root.join(format!("nonterminal-{managed}.sock")),
        )
        .unwrap();
        let mut command = super::cli::child_command(&f, managed);
        command.env(
            "ARC_FIXTURE_CONTROL",
            f.root.join(format!("nonterminal-{managed}.sock")),
        );
        command
            .arg("session-leader-sentinel")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        let mut child = super::runtime::OwnedChild::new(command.spawn().unwrap());
        let (mut barrier, info) = super::cli::accept(&listener);
        assert_eq!(info["tty"], false);
        assert_eq!(
            info["pgid"], info["pid"],
            "nonterminal target owns a separate group"
        );
        use std::io::Write;
        barrier.write_all(b"x").unwrap();
        let status = super::cli::wait(&mut child.child);
        assert_eq!(status.code(), Some(17), "managed={managed}");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        use std::io::Read;
        child
            .child
            .stdout
            .take()
            .unwrap()
            .read_to_end(&mut stdout)
            .unwrap();
        child
            .child
            .stderr
            .take()
            .unwrap()
            .read_to_end(&mut stderr)
            .unwrap();
        observations.push((stdout, stderr));
    }
    assert_eq!(observations[0], observations[1]);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn native_inherited_ignored_sigchld_preserves_target_state_and_completion() {
    use arc_core::process_identity::{probe, IdentityStatus, ProcessIdentity};
    use std::{
        io::Write,
        os::unix::{net::UnixListener, process::CommandExt},
        process::Stdio,
    };
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    for ignored in [false, true] {
        for blocked in [false, true] {
            let control = f.root.join(format!("chld-{ignored}-{blocked}.sock"));
            let listener = UnixListener::bind(&control).unwrap();
            let marker = f.root.join(format!("chld-{ignored}-{blocked}.calls"));
            let mut command = super::cli::child_command(&f, true);
            command
                .env("ARC_FIXTURE_CONTROL", &control)
                .env("ARC_FIXTURE_MARKER", &marker)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            unsafe {
                command.pre_exec(move || {
                    let mut action: libc::sigaction = std::mem::zeroed();
                    action.sa_sigaction = if ignored {
                        libc::SIG_IGN
                    } else {
                        libc::SIG_DFL
                    };
                    action.sa_flags = libc::SA_NOCLDWAIT;
                    libc::sigemptyset(&mut action.sa_mask);
                    if libc::sigaction(libc::SIGCHLD, &action, std::ptr::null_mut()) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    let mut mask = std::mem::zeroed();
                    libc::sigemptyset(&mut mask);
                    if blocked {
                        libc::sigaddset(&mut mask, libc::SIGCHLD);
                    }
                    if libc::sigprocmask(libc::SIG_SETMASK, &mask, std::ptr::null_mut()) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            let mut wrapper = super::runtime::OwnedChild::new(command.spawn().unwrap());
            let owner = ProcessIdentity::capture(wrapper.child.id()).unwrap();
            let (mut barrier, info) = super::cli::accept(&listener);
            let target: ProcessIdentity = serde_json::from_value(info["identity"].clone()).unwrap();
            // Release before assertions too: a fixture failure cannot strand the
            // target behind its barrier. The wrapper remains owned until reap.
            barrier.write_all(b"x").unwrap();
            drop(barrier);
            let status = super::cli::wait(&mut wrapper.child);
            assert_eq!(info["sigchld_ignored"], ignored);
            assert_eq!(info["sigchld_blocked"], blocked);
            assert_eq!(
                status.code(),
                Some(17),
                "ignored={ignored} blocked={blocked}"
            );
            let mut stderr = Vec::new();
            use std::io::Read;
            wrapper
                .child
                .stderr
                .take()
                .unwrap()
                .read_to_end(&mut stderr)
                .unwrap();
            assert_eq!(stderr, b"fixture-stderr\n");
            let job = super::protocol::status(&f, &super::cli::recent(&f));
            assert_eq!(
                job.outcome,
                Some(arc_ipc::jobs::Outcome::Exited { code: 17 })
            );
            assert_eq!(job.child, Some(target.clone()));
            assert_eq!(std::fs::read_to_string(marker).unwrap().lines().count(), 1);
            assert_eq!(probe(&target), IdentityStatus::Missing);
            assert_eq!(probe(&owner), IdentityStatus::Missing);
            eprintln!("inherited CHLD ignored={ignored} blocked={blocked}; target={target:?}; owner={owner:?}; exit=17; completion=17; cleanup verified");
        }
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn native_inherited_sigcont_target_stop_wrapper_continue_and_completion() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let output = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/managed_exec/pipeline.py"
        ))
        .arg("--continuation")
        .arg(&f.root)
        .arg(binary("arc-exec-fixture"))
        .arg(binary("arcctl"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .output()
        .unwrap();
    eprintln!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "native continuation comparison failed"
    );
    let db = rusqlite::Connection::open(&f.database).unwrap();
    for line in String::from_utf8(output.stdout).unwrap().lines() {
        let result: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(result["pass"], true);
        assert_eq!(result["allRecordedIdentitiesAbsent"], true);
        let target: arc_core::process_identity::ProcessIdentity =
            serde_json::from_value(result["producer"]["identity"].clone()).unwrap();
        let id: String = db
            .query_row(
                "SELECT job_id FROM job_lifecycle WHERE child_pid=?1",
                [target.pid],
                |r| r.get(0),
            )
            .unwrap();
        let job = super::protocol::status(&f, &arc_ipc::jobs::JobId::try_from(id).unwrap());
        assert_eq!(job.child, Some(target));
        assert_eq!(
            job.outcome,
            Some(arc_ipc::jobs::Outcome::Exited { code: 17 })
        );
        eprintln!("shared-job continuation: matching lifecycle completion17 verified");
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

// On assertion failure, terminate the verified target first so its still-owned
// wrapper can reap it. Never abandon a stopped grandchild behind a killed owner.
struct SupervisedTarget {
    wrapper: super::runtime::OwnedChild,
    target: arc_core::process_identity::ProcessIdentity,
    owner: arc_core::process_identity::ProcessIdentity,
}
#[cfg(target_os = "macos")]
fn target_stopped(identity: &arc_core::process_identity::ProcessIdentity) -> bool {
    assert_eq!(
        arc_core::process_identity::probe(identity),
        arc_core::process_identity::IdentityStatus::VerifiedMatch
    );
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
    assert_eq!(
        unsafe {
            libc::proc_pidinfo(
                identity.pid as i32,
                libc::PROC_PIDTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdinfo).cast(),
                size,
            )
        },
        size
    );
    info.pbi_status == libc::SSTOP
}
impl Drop for SupervisedTarget {
    fn drop(&mut self) {
        use arc_core::process_identity::{probe, IdentityStatus};
        if probe(&self.target) == IdentityStatus::VerifiedMatch {
            unsafe {
                libc::kill(self.target.pid as i32, libc::SIGKILL);
            }
        }
        if probe(&self.owner) == IdentityStatus::VerifiedMatch {
            unsafe {
                libc::kill(self.wrapper.child.id() as i32, libc::SIGCONT);
            }
        }
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while self.wrapper.child.try_wait().ok().flatten().is_none()
            && std::time::Instant::now() < end
        {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
#[test]
#[cfg(target_os = "macos")]
fn native_nonterminal_inherited_sigcont_target_stop_wrapper_continue_and_completion() {
    use arc_core::process_identity::{probe, IdentityStatus, ProcessIdentity};
    use std::{
        io::Write,
        os::unix::{net::UnixListener, process::CommandExt},
        process::Stdio,
    };
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    for ignored in [false, true] {
        let control = f.root.join(format!("cont-{ignored}.sock"));
        let listener = UnixListener::bind(&control).unwrap();
        let marker = f.root.join(format!("cont-{ignored}.calls"));
        let mut command = super::cli::child_command(&f, true);
        command
            .env("ARC_FIXTURE_CONTROL", &control)
            .env("ARC_FIXTURE_MARKER", &marker)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() < 0
                    || libc::signal(
                        libc::SIGCONT,
                        if ignored {
                            libc::SIG_IGN
                        } else {
                            libc::SIG_DFL
                        },
                    ) == libc::SIG_ERR
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let wrapper = super::runtime::OwnedChild::new(command.spawn().unwrap());
        let owner = ProcessIdentity::capture(wrapper.child.id()).unwrap();
        let (mut barrier, info) = super::cli::accept(&listener);
        let target = serde_json::from_value(info["identity"].clone()).unwrap();
        let mut owned = SupervisedTarget {
            wrapper,
            target,
            owner: owner.clone(),
        };
        assert_eq!(info["sigcont_ignored"], ignored);
        assert_eq!(info["tty"], false);
        assert_eq!(info["pgid"], info["pid"]);
        assert_eq!(probe(&owned.target), IdentityStatus::VerifiedMatch);
        unsafe {
            assert_eq!(libc::kill(owned.target.pid as i32, libc::SIGSTOP), 0);
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !target_stopped(&owned.target) {
            assert!(
                std::time::Instant::now() < deadline,
                "target stop not observed"
            );
            std::thread::yield_now();
        }
        loop {
            let mut status = 0;
            let pid = unsafe {
                libc::waitpid(
                    owner.pid as i32,
                    &mut status,
                    libc::WUNTRACED | libc::WNOHANG,
                )
            };
            assert!(pid >= 0);
            if pid > 0 {
                assert!(libc::WIFSTOPPED(status));
                assert_eq!(libc::WSTOPSIG(status), libc::SIGSTOP);
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "wrapper stop not observed"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(probe(&owner), IdentityStatus::VerifiedMatch);
        unsafe {
            assert_eq!(libc::kill(owner.pid as i32, libc::SIGCONT), 0);
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let mut status = 0;
            let pid = unsafe {
                libc::waitpid(
                    owner.pid as i32,
                    &mut status,
                    libc::WCONTINUED | libc::WNOHANG,
                )
            };
            assert!(pid >= 0);
            if pid > 0 {
                assert!(
                    libc::WIFCONTINUED(status),
                    "wrapper must continue before target exits"
                );
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "wrapper continuation not observed"
            );
            std::thread::yield_now();
        }
        while target_stopped(&owned.target) {
            assert!(
                std::time::Instant::now() < deadline,
                "wrapper continued but target stayed stopped"
            );
            std::thread::yield_now();
        }
        barrier.write_all(b"x").unwrap();
        assert_eq!(
            super::cli::wait(&mut owned.wrapper.child).code(),
            Some(17),
            "ignored={ignored}"
        );
        let job = super::protocol::status(&f, &super::cli::recent(&f));
        assert_eq!(job.child, Some(owned.target.clone()));
        assert_eq!(
            job.outcome,
            Some(arc_ipc::jobs::Outcome::Exited { code: 17 })
        );
        assert_eq!(std::fs::read_to_string(marker).unwrap().lines().count(), 1);
        assert_eq!(probe(&owned.target), IdentityStatus::Missing);
        assert_eq!(probe(&owner), IdentityStatus::Missing);
        eprintln!("nonterminal CONT ignored={ignored}; target={:?}; owner={owner:?}; exit17 completion17 one invocation cleanup verified", owned.target);
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}
