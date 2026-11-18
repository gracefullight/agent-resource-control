//! Tool outcomes are independently checked, including expected negative diagnostics.
use super::{projects::*, *};

fn entry_observed<T>(
    state: &mut T,
    count: impl Fn(&T) -> usize,
    exited: impl FnOnce(&mut T) -> bool,
) -> Result<bool, ()> {
    if count(state) == 1 {
        return Ok(true);
    }
    if exited(state) {
        // The shim may have entered and the wrapper exited between the first
        // witness read and the exit check. Its append is complete by exit.
        return if count(state) == 1 { Ok(true) } else { Err(()) };
    }
    Ok(false)
}

fn start_observed(case: &Case, project: &Path, argv: &[&str]) -> Run {
    let mut run = Run::start(case, project, argv);
    eventually(|| {
        entry_observed(
            &mut run,
            |run| run.count("bun"),
            |run| run.wrapper.child.try_wait().unwrap().is_some(),
        )
        .unwrap_or_else(|()| {
            panic!(
                "Bun command exited before shim entry: {}",
                log(&project.join("stderr"))
            )
        })
    });
    run
}

pub fn run(read: Read) {
    let case = Case::new("real-outputs", read, false);
    let mut observations = Vec::new();
    // Each invocation has its own source, output and launch witnesses. Keep
    // two independent cases in flight, observing shim entry before the next
    // admission request reaches the daemon.
    for batch in [
        ("bun-build", Kind::BunBuild),
        ("vite-build", Kind::Vite),
        ("bun-test", Kind::Test),
        ("biome-check", Kind::Lint),
        ("tsc-no-emit", Kind::Typecheck),
    ]
    .chunks(2)
    {
        let mut successes = Vec::new();
        for &(name, kind) in batch {
            let p = project(&case, name, kind, false);
            let argv = kind.argv();
            let run = start_observed(&case, &p, &argv);
            successes.push((name, kind, p, argv, run));
        }
        for (name, kind, p, argv, mut run) in successes {
            run.finish(0);
            assert_eq!(run.count("bun"), 1, "original bun run must execute once");
            let check = validate_artifact(&case, &p, kind);
            let record = case.terminal(&run, kind.class(), kind.reason(), 0);
            observations.push(json!({"name":name,"argv":argv,"cwd":p,"originalStarts":1,"independentOutput":check,"record":record,"stdout":log(&p.join("stdout")),"stderr":log(&p.join("stderr"))}));
        }
    }
    for batch in [
        (
            "bad-lint",
            Kind::Lint,
            "debugger;\nexport const fixture = 42;\n",
            1,
            "lint/suspicious/noDebugger",
        ),
        (
            "bad-types",
            Kind::Typecheck,
            "export const fixture: number = 'wrong';\n",
            1,
            "TS2322",
        ),
        ("bad-test", Kind::Test, "", 1, "1 fail"),
    ]
    .chunks(2)
    {
        let mut failures = Vec::new();
        for &(name, kind, body, code, expected) in batch {
            let p = project(&case, name, kind, false);
            if kind == Kind::Test {
                write(&p.join("evidence.test.ts"),"import {test,expect} from 'bun:test'; test('expected failure',()=>expect(1).toBe(2));\n");
            } else {
                write(&p.join("good.ts"), body);
            }
            let run = start_observed(&case, &p, &kind.argv());
            failures.push((name, kind, code, expected, p, run));
        }
        for (name, kind, code, expected, p, mut run) in failures {
            run.finish(code);
            let out = format!("{}{}", log(&p.join("stdout")), log(&p.join("stderr")));
            assert!(
                out.contains(expected),
                "independent diagnostic {expected}: {out}"
            );
            assert_eq!(run.count("bun"), 1);
            if kind == Kind::Typecheck {
                validate_artifact(&case, &p, kind);
            }
            observations.push(json!({"name":name,"argv":kind.argv(),"outcome":code,"expectedDiagnostic":expected,"output":out,"record":case.terminal(&run,kind.class(),kind.reason(),code)}));
        }
    }
    case.save(
        "real-tools",
        json!({"realTools":true,"injectedPressure":false,"jobs":observations}),
    );
    case.done();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shim_with_identity(
        project: &Path,
        capture: impl FnOnce(u32) -> std::io::Result<ProcessIdentity>,
    ) -> std::io::Result<(Run, std::process::ChildStdin)> {
        let child = Command::new("/bin/sh")
            .args([
                "-c",
                "read -r release || exec /bin/sleep 300; printf 'bun %s %s\\n' \"$$\" \"$PPID\" >> \"$1\"",
                "shim-witness",
            ])
            .arg(project.join("launches"))
            .stdin(Stdio::piped())
            .spawn()?;
        let mut child = native::OwnedChild::new(child);
        let identity = capture(child.child.id())?;
        let input = child.child.stdin.take().unwrap();
        let run = Run {
            identity,
            wrapper: child,
            project: project.to_path_buf(),
            fifo: None,
        };
        Ok((run, input))
    }

    #[test]
    fn shim_identity_capture_failure_reaps_the_spawned_child() {
        let mut pid = 0;
        let error = shim_with_identity(Path::new("/unused"), |child_pid| {
            pid = child_pid;
            assert_eq!(unsafe { libc::kill(pid as i32, 0) }, 0);
            Err(std::io::Error::other("injected identity capture failure"))
        })
        .err()
        .expect("capture must fail");
        assert_eq!(error.to_string(), "injected identity capture failure");

        let mut status = 0;
        let waited = unsafe { libc::waitpid(pid as i32, &mut status, libc::WNOHANG) };
        let wait_error = std::io::Error::last_os_error();
        if waited == 0 {
            // Keep the regression safe even against the unfixed implementation.
            assert_eq!(unsafe { libc::kill(pid as i32, libc::SIGKILL) }, 0);
            eventually(|| unsafe { libc::waitpid(pid as i32, &mut status, libc::WNOHANG) } != 0);
        }
        assert_eq!(waited, -1, "spawned child was not reaped");
        assert_eq!(wait_error.raw_os_error(), Some(libc::ECHILD));
    }

    #[test]
    fn shim_entry_recorded_between_first_read_and_exit_is_observed() {
        let project = std::env::temp_dir().join(format!(
            "arc-bun-entry-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::create_dir(&project).unwrap();
        let (mut run, mut input) = shim_with_identity(&project, ProcessIdentity::capture).unwrap();

        let observed = entry_observed(
            &mut run,
            |run| run.count("bun"),
            |run| {
                assert_eq!(run.count("bun"), 0, "first witness read precedes release");
                writeln!(input, "release").unwrap();
                drop(input);
                eventually(|| run.wrapper.child.try_wait().unwrap().is_some());
                true
            },
        );
        assert_eq!(observed, Ok(true));
        assert_eq!(run.count("bun"), 1);
        assert!(run.wrapper.child.wait().unwrap().success());
        fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn shim_entry_still_requires_a_witness_when_wrapper_exits() {
        assert_eq!(entry_observed(&mut (), |_| 0, |_| false), Ok(false));
        assert_eq!(entry_observed(&mut (), |_| 0, |_| true), Err(()));
        assert_eq!(entry_observed(&mut (), |_| 2, |_| true), Err(()));
    }
}
