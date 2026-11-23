//! Temporary package workspaces and the built CLI reads of the lane C test.
use crate::journey::{output_with_input, Sandbox};
use crate::runtime::Fixture;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs,
    os::unix::{fs::MetadataExt, io::AsRawFd, net::UnixDatagram, process::CommandExt},
    path::{Path, PathBuf},
    time::Duration,
};

pub const CI_A: (&str, &str) = ("ci", "bun test");
pub const CI_B: (&str, &str) = ("ci", "bun build x.ts");

/// A workspace root: `package.json` `workspaces` and `pnpm-workspace.yaml`
/// both name `packages/*`, so every runner sees the same members.
pub struct Workspace {
    pub root: PathBuf,
}

pub fn manifest(name: &str, scripts: &[(&str, &str)]) -> String {
    let scripts: serde_json::Map<_, _> = scripts
        .iter()
        .map(|(k, v)| (k.to_string(), Value::from(*v)))
        .collect();
    json!({"name": name, "scripts": scripts}).to_string()
}

impl Workspace {
    pub fn new(s: &Sandbox, name: &str, members: &[(&str, &[(&str, &str)])]) -> Self {
        let root = s.project(name);
        fs::write(
            root.join("package.json"),
            json!({"name": "root", "private": true, "workspaces": ["packages/*"]}).to_string(),
        )
        .unwrap();
        fs::write(
            root.join("pnpm-workspace.yaml"),
            "packages:\n  - 'packages/*'\n",
        )
        .unwrap();
        let workspace = Self { root };
        for (member, scripts) in members {
            workspace.member(member, scripts);
        }
        workspace
    }
    pub fn manifest_path(&self, member: &str) -> PathBuf {
        self.root.join("packages").join(member).join("package.json")
    }
    pub fn member(&self, member: &str, scripts: &[(&str, &str)]) {
        let path = self.manifest_path(member);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, manifest(member, scripts)).unwrap();
    }
    /// Write the same bytes again and move the mtime: a new identity only.
    pub fn touch_identical(&self, path: &Path) {
        let bytes = fs::read(path).unwrap();
        let before = fs::metadata(path).unwrap().modified().unwrap();
        fs::write(path, &bytes).unwrap();
        let file = fs::File::options().write(true).open(path).unwrap();
        file.set_modified(before + Duration::from_secs(5)).unwrap();
        drop(file);
        assert_ne!(fs::metadata(path).unwrap().modified().unwrap(), before);
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    /// Replace the file by rename: a new inode with new content.
    pub fn replace(&self, path: &Path, text: &str) {
        let before = fs::metadata(path).unwrap();
        let next = path.with_extension("next");
        fs::write(&next, text).unwrap();
        fs::rename(&next, path).unwrap();
        let after = fs::metadata(path).unwrap();
        assert_ne!((before.dev(), before.ino()), (after.dev(), after.ino()));
    }
}

/// `arcctl classify --json --diagnostics` with marked stubs on PATH: any
/// execution of a runner would be recorded in `marker`.
pub fn classify(
    f: &Fixture,
    s: &Sandbox,
    cwd: &Path,
    command: &str,
    env: &[(&str, &str)],
) -> Value {
    let mut cli = s.arcctl(f, cwd);
    cli.args(["classify", "--json", "--diagnostics", "--", command])
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", marker(s));
    for (key, value) in env {
        cli.env(key, value);
    }
    let output = output_with_input(&mut cli, b"", "arcctl classify");
    assert!(output.status.success(), "{command}: {output:?}");
    assert!(output.stderr.is_empty(), "{command}: {output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

/// A private datagram identifies the branch taken by this particular child.
/// The sender is inherited only in that child, and the observer reports only
/// the four classification fields. An absent or extra observation fails.
fn classify_ordinary(f: &Fixture, s: &Sandbox, cwd: &Path, command: &str) -> (Value, String) {
    let (receiver, sender) = UnixDatagram::pair().unwrap();
    receiver.set_nonblocking(true).unwrap();
    let fd = sender.as_raw_fd();
    let mut cli = s.arcctl(f, cwd);
    cli.args(["classify", "--json", "--", command])
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", marker(s))
        .env("ARC_TEST_ANALYSIS_OBSERVER_FD", fd.to_string());
    // SAFETY: after fork, only async-signal-safe fcntl calls are made before
    // exec; the parent's descriptor remains close-on-exec in other children.
    unsafe {
        cli.pre_exec(move || {
            let flags = libc::fcntl(fd, libc::F_GETFD);
            if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let output = output_with_input(&mut cli, b"", "ordinary arcctl classify");
    assert!(output.status.success(), "{command}: {output:?}");
    assert!(output.stderr.is_empty(), "{command}: {output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 4, "{value}");
    let mut buffer = [0; 1024];
    let received = receiver
        .recv(&mut buffer)
        .expect("missing classification branch observation");
    let observed: Value = serde_json::from_slice(&buffer[..received]).unwrap();
    assert_eq!(observed.as_object().unwrap().len(), 2, "{observed}");
    assert_eq!(
        observed["result"], value,
        "observation belongs to this invocation"
    );
    assert_eq!(
        receiver.recv(&mut buffer).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "the child reported more than one analysis"
    );
    (value, observed["branch"].as_str().unwrap().to_owned())
}

/// Exercise ordinary classifications before and after an edit without relying
/// on a later diagnostic command or on cache-file metadata to infer reuse.
pub fn ordinary_cache_reuse(f: &Fixture, s: &Sandbox) {
    let single = s.project("single-cache");
    let manifest_path = single.join("package.json");
    fs::write(
        &manifest_path,
        manifest(
            "single",
            &[("test", "bun test"), ("build", "bun build x.ts")],
        ),
    )
    .unwrap();
    let entries = || -> BTreeSet<_> {
        fs::read_dir(cache_dir(s))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect()
    };
    let before = entries();
    let (ordinary, branch) = classify_ordinary(f, s, &single, "bun run build");
    assert_eq!(branch, "computed");
    assert_eq!(
        result(&ordinary),
        ["BUILD", "HOST_LOCAL", "BUN_SCRIPT_BUILD", "NONE"]
    );
    let added: Vec<_> = entries().difference(&before).cloned().collect();
    assert_eq!(
        added.len(),
        1,
        "ordinary classification populated one entry: {added:?}"
    );
    let entry = &added[0];
    let (inode, bytes) = (fs::metadata(entry).unwrap().ino(), fs::read(entry).unwrap());
    let (repeated, branch) = classify_ordinary(f, s, &single, "bun run build");
    assert_eq!(
        branch, "recorded",
        "second ordinary invocation reused the recorded result"
    );
    assert_eq!(repeated, ordinary);
    assert_eq!(fs::metadata(entry).unwrap().ino(), inode);
    assert_eq!(fs::read(entry).unwrap(), bytes);
    let build = classify(f, s, &single, "bun run build", &[]);
    assert_eq!(cache(&build), "hit", "{build}");
    assert_eq!(result(&build), result(&ordinary));
    assert_eq!(crate::journey::starts(&marker(s)), 0);
    assert_eq!(
        exec(f, s, &single, "bun run build"),
        ("BUILD".to_owned(), "BUN_SCRIPT_BUILD".to_owned())
    );
    assert_eq!(last_job_result(f), result(&ordinary));

    fs::write(
        &manifest_path,
        manifest("single", &[("test", "bun test"), ("build", "bun test")]),
    )
    .unwrap();
    let (changed, branch) = classify_ordinary(f, s, &single, "bun run build");
    assert_eq!(branch, "computed", "manifest edit invalidated the result");
    assert_eq!(
        result(&changed),
        ["TEST", "HOST_LOCAL", "BUN_SCRIPT_TEST", "NONE"]
    );
    assert_ne!(
        fs::metadata(entry).unwrap().ino(),
        inode,
        "manifest edit replaced the entry"
    );
    let (reused, branch) = classify_ordinary(f, s, &single, "bun run build");
    assert_eq!(branch, "recorded", "the edited result was reused");
    assert_eq!(reused, changed);
    let diagnostic = classify(f, s, &single, "bun run build", &[]);
    assert_eq!(cache(&diagnostic), "hit", "{diagnostic}");
    assert_eq!(result(&diagnostic), result(&changed));
    assert_eq!(
        crate::journey::starts(&marker(s)),
        0,
        "classify did not execute a fixture script"
    );
    assert_eq!(
        exec(f, s, &single, "bun run build"),
        ("TEST".to_owned(), "BUN_SCRIPT_TEST".to_owned())
    );
    assert_eq!(last_job_result(f), result(&changed));
}

pub fn marker(s: &Sandbox) -> PathBuf {
    s.home.join("classify-starts")
}

/// `(member, script, class)` of the diagnostics, in member order.
pub fn members(value: &Value) -> Vec<(String, String, String)> {
    value["analysis"]["members"]
        .as_array()
        .unwrap_or_else(|| panic!("{value}"))
        .iter()
        .map(|m| {
            (
                m["member"].as_str().unwrap().to_owned(),
                m["script"].as_str().unwrap().to_owned(),
                m["classification"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}
pub fn member(name: &str, script: &str, class: &str) -> (String, String, String) {
    (
        format!("packages/{name}"),
        script.to_owned(),
        class.to_owned(),
    )
}
pub fn cache(value: &Value) -> &str {
    value["analysis"]["cache"]
        .as_str()
        .unwrap_or_else(|| panic!("{value}"))
}
/// The four ordinary fields, which must not change with the diagnostics.
pub fn result(value: &Value) -> [String; 4] {
    ["classification", "locality", "reason", "uncertainty"].map(|k| {
        value[k]
            .as_str()
            .unwrap_or_else(|| panic!("{k}: {value}"))
            .to_owned()
    })
}

/// The cache directory beside the CLI's default database location.
pub fn cache_dir(s: &Sandbox) -> PathBuf {
    let base = if cfg!(target_os = "macos") {
        s.home.join("Library/Application Support")
    } else {
        s.home.join(".local/share")
    };
    base.join("arc/analysis-cache")
}

/// The last recorded job: its class and classification reason.
pub fn last_job(f: &Fixture) -> (String, String) {
    crate::journey::database(f)
        .query_row(
            "SELECT j.job_class,l.classification_reason FROM jobs j
             JOIN job_lifecycle l ON l.job_id=j.id ORDER BY j.rowid DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}
fn last_job_result(f: &Fixture) -> [String; 4] {
    crate::journey::database(f)
        .query_row(
            "SELECT j.job_class,j.locality,l.classification_reason,l.classification_uncertainty \
             FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id ORDER BY j.rowid DESC LIMIT 1",
            [],
            |r| Ok([r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?]),
        )
        .unwrap()
}
pub fn jobs(f: &Fixture) -> i64 {
    crate::journey::database(f)
        .query_row("SELECT count(*) FROM jobs", [], |r| r.get(0))
        .unwrap()
}
pub fn exec(f: &Fixture, s: &Sandbox, cwd: &Path, command: &str) -> (String, String) {
    exec_with_env(f, s, cwd, command, &[])
}

pub fn exec_with_env(
    f: &Fixture,
    s: &Sandbox,
    cwd: &Path,
    command: &str,
    env: &[(&str, &str)],
) -> (String, String) {
    let before = jobs(f);
    // The runner invocation itself, as an agent's shell would pass it.
    let mut argv = vec!["exec".to_owned(), "--".to_owned()];
    argv.extend(command.split(' ').map(|w| w.trim_matches('\'').to_owned()));
    let mut exec = s.arcctl(f, cwd);
    exec.args(&argv).env("ARC_FIXTURE_MODE", "echo");
    for (key, value) in env {
        exec.env(key, value);
    }
    let output = output_with_input(&mut exec, b"input\n", "arcctl exec");
    assert_eq!(output.status.code(), Some(17), "{command}: {output:?}");
    assert_eq!(jobs(f), before + 1, "{command}");
    last_job(f)
}
