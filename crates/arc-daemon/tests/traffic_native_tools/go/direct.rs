//! This focused lane check requires real Go, but no daemon/product wiring.
use super::{super::*, projects::*};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::Instant,
};

pub fn assert_result(form: Form, failure: bool, output: &std::process::Output) {
    assert_eq!(
        output.status.code(),
        Some(i32::from(failure)),
        "{form:?}: {output:?}"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if failure {
        let diagnostic = match form {
            Form::GoBuild => "cannot use 7",
            Form::GoTest => "intended native Go assertion failure",
            Form::GoVet => "fmt.Printf format %d",
            _ => unreachable!(),
        };
        assert!(combined.contains(diagnostic), "{form:?}: {combined}");
    } else if form == Form::GoTest {
        assert!(!combined.contains("[no tests to run]"), "{combined}");
        assert!(combined.contains("example.org/nativefixture"), "{combined}");
    }
}

/// The Go runner prints elapsed seconds. Compare every other byte unchanged;
/// elapsed values are observations, not deterministic fixture output.
pub fn streams(form: Form, bytes: &[u8]) -> String {
    let mut result = String::from_utf8(bytes.to_vec()).unwrap();
    if form == Form::GoTest {
        for token in String::from_utf8_lossy(bytes).split_whitespace() {
            let bare = token.trim_matches(['(', ')']);
            if let Some(number) = bare.strip_suffix('s') {
                if !number.is_empty() && number.bytes().all(|c| c.is_ascii_digit() || c == b'.') {
                    result = result.replace(bare, "<native elapsed>s");
                }
            }
        }
    }
    result
}

pub fn dry_streams(bytes: &[u8]) -> String {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    if let Some(work) = text.lines().find_map(|line| line.strip_prefix("WORK=")) {
        assert!(!work.is_empty());
        text.replace(work, "<native-work-directory>")
    } else {
        text
    }
}

pub fn facts(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(path.join("native.json")).unwrap()).unwrap()
}

// A worker owns all of its case projects. Successful siblings must not leave
// their projects behind when another worker panics; the failing worker keeps
// its directory for diagnosis.
struct DirectProjects(PathBuf);

impl DirectProjects {
    fn new(base: &Path, form: Form) -> Self {
        let path = base.join(form.id());
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for DirectProjects {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            fs::remove_dir_all(&self.0).expect("remove successful direct Go projects");
        }
    }
}

fn direct_workers<T: Send>(base: &Path, check: impl Fn(&Path, Form) -> T + Sync) -> Vec<T> {
    std::thread::scope(|scope| {
        let jobs: Vec<_> = FORMS
            .into_iter()
            .map(|form| {
                let check = &check;
                scope.spawn(move || {
                    let projects = DirectProjects::new(base, form);
                    check(&projects.0, form)
                })
            })
            .collect();
        // Join all workers before propagating a panic, so each sibling runs
        // its own cleanup regardless of the order in which failures occur.
        let results = jobs.into_iter().map(|job| job.join()).collect::<Vec<_>>();
        results.into_iter().map(|result| result.unwrap()).collect()
    })
}

#[test]
fn go_offline_success_failure_and_source_barriers() {
    let base = root().join(format!(
        "target/traffic-native-tools/go/direct-{}",
        std::process::id()
    ));
    fs::create_dir_all(&base).unwrap();
    // Each worker owns its cache and project state; a native tool never shares
    // writable state with another form or bypass scenario.
    let results = direct_workers(&base, check_form);
    let tools = &results[0].1;
    let observations = results
        .iter()
        .flat_map(|result| result.0.clone())
        .collect::<Vec<_>>();
    let verified_trees: u64 = results.iter().map(|result| result.2).sum();
    println!(
        "GO_OFFLINE_OBSERVATION {}",
        serde_json::json!({"tools":tools,
        "successFailure":observations,"sourceFifoReadyAndReleasedForms":["go-build","go-test","go-vet"],
        "bypassModes":["compile-only","execution-wrapper","dry-run"],"ownedChildrenReaped":true,
        "verifiedDirectTrees":verified_trees})
    );
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn successful_go_workers_clean_projects_when_a_sibling_panics() {
    let root = std::env::temp_dir().join(format!(
        "arc-go-direct-cleanup-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir_all(&root).unwrap();
    let result = std::panic::catch_unwind(|| {
        direct_workers(&root, |path, form| {
            fs::write(path.join("evidence"), form.id()).unwrap();
            assert_ne!(form, Form::GoTest, "simulated worker failure");
        });
    });
    assert!(result.is_err());
    assert!(!root.join(Form::GoBuild.id()).exists());
    assert!(!root.join(Form::GoVet.id()).exists());
    assert_eq!(
        fs::read(root.join(Form::GoTest.id()).join("evidence")).unwrap(),
        b"go-test"
    );
    fs::remove_dir_all(root).unwrap();
}

fn timed_direct(path: &Path, tools: &Tools, args: &[String], input: &[u8]) -> Output {
    let started = Instant::now();
    let output = direct(path, &tools.go, args, input);
    let elapsed = started.elapsed();
    if elapsed.as_secs() >= 5 {
        println!("GO_FIXTURE_WAIT {} {args:?} {elapsed:?}", path.display());
    }
    output
}

fn check_form(base: &Path, form: Form) -> (Vec<serde_json::Value>, serde_json::Value, u64) {
    let before = super::owned::counts();
    let tools = Tools::installed();
    let mut observations = Vec::new();
    for fail in [false, true] {
        let path = project(
            base,
            &tools,
            &format!("{}-{fail}", form.id()),
            form,
            false,
            fail,
        );
        let arguments = argv(form);
        let input = "stdin\0bytes 日本語\n".as_bytes();
        let output = timed_direct(&path, &tools, &arguments, input);
        assert_result(form, fail, &output);
        let expected_facts = (form == Form::GoTest).then(|| facts(&path));
        let archive = (form == Form::GoBuild && !fail).then(|| sha(&path.join("native output.a")));
        write(&path.join("test-bodies"), "");
        let repeated = timed_direct(&path, &tools, &arguments, input);
        assert_result(form, fail, &repeated);
        assert_eq!(
            streams(form, &output.stdout),
            streams(form, &repeated.stdout)
        );
        assert_eq!(
            streams(form, &output.stderr),
            streams(form, &repeated.stderr)
        );
        if let Some(expected) = expected_facts {
            assert_eq!(facts(&path), expected);
        }
        if let Some(expected) = archive {
            assert_eq!(sha(&path.join("native output.a")), expected);
        }
        if form == Form::GoTest {
            let facts = facts(&path);
            assert_eq!(facts["value"], "literal $HOME ' value 日本語");
            assert_eq!(facts["library"], "native");
            assert_eq!(log(&path.join("test-bodies")).lines().count(), 1);
        } else if form == Form::GoBuild && !fail {
            assert!(fs::metadata(path.join("native output.a")).unwrap().len() > 0);
        }
        observations.push(serde_json::json!({"form":form.id(),"failure":fail,
            "exit":output.status.code(),"stdout":String::from_utf8_lossy(&output.stdout),
            "stderr":String::from_utf8_lossy(&output.stderr)}));
    }
    let path = project(
        base,
        &tools,
        &format!("{}-barrier", form.id()),
        form,
        true,
        false,
    );
    let mut command = Command::new(&tools.go);
    command
        .args(&argv(form)[1..])
        .current_dir(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure(&path, &mut command);
    let mut child = super::owned::GoChild::spawn(&mut command).unwrap();
    let mut writer = None;
    eventually(|| {
        assert!(
            child.child.try_wait().unwrap().is_none(),
            "Go exited before source FIFO readiness"
        );
        gate(&path, &mut writer)
    });
    assert!(child.child.try_wait().unwrap().is_none());
    assert_eq!(log(&path.join("bodies")).lines().count(), 1);
    release(&mut writer);
    let started = Instant::now();
    assert_result(form, false, &child.wait_with_output().unwrap());
    if started.elapsed().as_secs() >= 5 {
        println!(
            "GO_FIXTURE_WAIT {} barrier {:?}",
            path.display(),
            started.elapsed()
        );
    }
    check_bypass(base, &tools, form);
    let after = super::owned::counts();
    assert_eq!(
        after.0 - before.0,
        after.1 - before.1,
        "every direct Go tree must have verified cleanup"
    );
    (observations, tools.receipt(), after.1 - before.1)
}

fn check_bypass(base: &Path, tools: &Tools, form: Form) {
    // These native execution modes are intentionally outside admission grammar.
    let (name, arguments): (&str, Vec<&str>) = match form {
        Form::GoBuild => (
            "compile-only",
            vec!["go", "test", "-c", "-o", "test output"],
        ),
        Form::GoTest => (
            "execution-wrapper",
            vec!["go", "test", "-count=1", "-exec", "/usr/bin/env", "."],
        ),
        Form::GoVet => ("dry-run", vec!["go", "build", "-n", "."]),
        _ => unreachable!(),
    };
    let path = project(base, tools, name, Form::GoTest, false, false);
    let args = arguments.into_iter().map(str::to_owned).collect::<Vec<_>>();
    let output = timed_direct(&path, tools, &args, b"");
    assert_eq!(output.status.code(), Some(0), "{name}: {output:?}");
    if name == "compile-only" {
        assert!(path.join("test output").is_file());
        assert!(!path.join("test-bodies").exists());
    } else if name == "execution-wrapper" {
        assert_eq!(log(&path.join("test-bodies")).lines().count(), 1);
    }
    let repeated = timed_direct(&path, tools, &args, b"");
    assert_eq!(repeated.status.code(), Some(0), "{name}: {repeated:?}");
    if name == "dry-run" {
        assert_eq!(dry_streams(&output.stdout), dry_streams(&repeated.stdout));
        assert_eq!(dry_streams(&output.stderr), dry_streams(&repeated.stderr));
    } else {
        assert_eq!(
            streams(Form::GoTest, &output.stdout),
            streams(Form::GoTest, &repeated.stdout)
        );
        assert_eq!(
            streams(Form::GoTest, &output.stderr),
            streams(Form::GoTest, &repeated.stderr)
        );
    }
}
