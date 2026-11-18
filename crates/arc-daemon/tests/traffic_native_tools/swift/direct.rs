//! Direct baselines write to owned files while the native barrier is pending.
use super::{process::TreeChild, projects, tools::Tools};
use crate::suite::{log, write, Prepared};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

fn capture_to_project(project: &Path, command: &mut Command) {
    command
        .stdout(File::create(project.join("direct-stdout")).unwrap())
        .stderr(File::create(project.join("direct-stderr")).unwrap());
}

fn tail(path: &Path) -> String {
    const LIMIT: u64 = 16 * 1024;
    let read = || -> std::io::Result<String> {
        let mut file = File::open(path)?;
        let length = file.metadata()?.len();
        file.seek(SeekFrom::Start(length.saturating_sub(LIMIT)))?;
        let mut bytes = Vec::new();
        file.take(LIMIT).read_to_end(&mut bytes)?;
        Ok(format!(
            "{}{}",
            if length > LIMIT {
                "[earlier output omitted]\n"
            } else {
                ""
            },
            String::from_utf8_lossy(&bytes)
        ))
    };
    read().unwrap_or_else(|error| format!("<cannot read {}: {error}>", path.display()))
}

fn diagnostic(project: &Path, pid: u32, reason: &str, cleanup: &str) -> String {
    format!(
        "Swift direct child pid={pid}: {reason}; cleanup={cleanup}\nstdout:\n{}\nstderr:\n{}",
        tail(&project.join("direct-stdout")),
        tail(&project.join("direct-stderr"))
    )
}

fn wait_for_gate<T>(
    child: &mut TreeChild,
    project: &Path,
    mut ready: impl FnMut(u32) -> Option<T>,
    mut expired: impl FnMut() -> bool,
) -> Result<T, String> {
    let pid = child.child.id();
    loop {
        match child.has_exited() {
            Ok(true) => {
                let result = child.wait_with_output_until_exit();
                return Err(diagnostic(
                    project,
                    pid,
                    "exited before native barrier",
                    &format!("{result:?}"),
                ));
            }
            Ok(false) => (),
            Err(error) => {
                return Err(diagnostic(
                    project,
                    pid,
                    "cannot observe child",
                    &error.to_string(),
                ));
            }
        }
        if let Some(gate) = ready(pid) {
            return Ok(gate);
        }
        if expired() {
            // The real 180-second watchdog has elapsed. Force the existing
            // owned-tree cleanup before reading the final diagnostic files.
            let cleanup = child.wait_with_output(Duration::ZERO);
            return Err(diagnostic(
                project,
                pid,
                "180-second native barrier watchdog",
                &format!("{cleanup:?}"),
            ));
        }
        std::thread::yield_now();
    }
}

pub fn direct(tools: &Tools, prepared: &Prepared) -> Output {
    let mut command = Command::new(tools.shims.join("swift"));
    command
        .args(&prepared.argv[1..])
        .current_dir(&prepared.project);
    capture_to_project(&prepared.project, &mut command);
    projects::configure(&prepared.project, &mut command);
    command.stdin(Stdio::piped()).env_remove("TBS_PREDECESSOR");
    if prepared.project.join("release").exists() {
        fs::remove_file(prepared.project.join("release")).unwrap();
    }
    let mut child = TreeChild::spawn(command).unwrap();
    child
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(projects::INPUT)
        .unwrap();
    let end = Instant::now() + Duration::from_secs(180);
    let mut gate = wait_for_gate(
        &mut child,
        &prepared.project,
        |pid| super::evidence::held(&prepared.project, pid),
        || Instant::now() >= end,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    gate.0.write_all(b"release\n").unwrap();
    write(&prepared.project.join("release"), "release");
    let mut result = child.wait_with_output_until_exit().unwrap_or_else(|error| {
        panic!(
            "{}",
            diagnostic(
                &prepared.project,
                child.child.id(),
                "wait after release failed",
                &error.to_string()
            )
        )
    });
    result.stdout = fs::read(prepared.project.join("direct-stdout")).unwrap();
    result.stderr = fs::read(prepared.project.join("direct-stderr")).unwrap();
    assert_eq!(log(&prepared.project.join("launches")).lines().count(), 1);
    assert_eq!(
        super::evidence::body_count(&prepared.project),
        1,
        "required native Swift fixture did not enter exactly once: {result:?}"
    );
    super::streams::validate(&result.stdout, &result.stderr);
    result
}

#[cfg(test)]
#[path = "direct_tests.rs"]
mod tests;
