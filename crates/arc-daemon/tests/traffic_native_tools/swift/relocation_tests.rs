//! Verify real SwiftPM trees are leased again at their original absolute paths.
use super::*;
use std::sync::atomic::Ordering;

fn executed_artifact(project: &Path, form: Form) -> PathBuf {
    let observed = super::super::evidence::observation(project);
    if form == Form::SwiftBuild {
        return PathBuf::from(observed["executable"].as_str().unwrap())
            .canonicalize()
            .unwrap();
    }
    let bundle = Path::new(observed["entry"]["testBundle"].as_str().unwrap());
    let binaries = fs::read_dir(bundle.join("Contents/MacOS"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    assert_eq!(
        binaries.len(),
        1,
        "unexpected XCTest bundle binaries: {binaries:?}"
    );
    binaries[0].canonicalize().unwrap()
}

fn execute_and_check(tools: &Tools, prepared: &Prepared) {
    let scratch = prepared
        .project
        .join("scratch path")
        .canonicalize()
        .unwrap();
    let output = direct(tools, prepared);
    assert_result(&output, prepared.form, false);
    super::super::streams::validate_form(prepared.form, &output.stdout, &output.stderr);
    record(&prepared.project);
    let used = executed_artifact(&prepared.project, prepared.form);
    assert!(
        used.starts_with(&scratch),
        "SwiftPM executed outside its leased tree: {used:?}"
    );
}

#[test]
fn swift_prepared_build_tree_is_private_and_seeded_once() {
    let tools = Tools::installed();
    let mut first = prepare(
        &tools.directory,
        &tools,
        "private one",
        Form::SwiftBuild,
        true,
        false,
    );
    let mut second = prepare(
        &tools.directory,
        &tools,
        "private two",
        Form::SwiftTest,
        true,
        false,
    );
    first.argv.push("--verbose".into());
    second.argv.push("--verbose".into());
    assert_eq!(SEED_PREPARATIONS.load(Ordering::SeqCst), 4);
    assert_ne!(first.project, second.project);
    let marker = first.project.join("scratch path/case-only");
    fs::write(&marker, b"private").unwrap();
    assert!(!second.project.join("scratch path/case-only").exists());
    fs::remove_file(marker).unwrap();
    std::thread::scope(|scope| {
        let build = scope.spawn(|| execute_and_check(&tools, &first));
        let test = scope.spawn(|| execute_and_check(&tools, &second));
        build.join().unwrap();
        test.join().unwrap();
    });

    let first_path = first.project.clone();
    let second_path = second.project.clone();
    tools.release_success(&first_path);
    tools.release_success(&second_path);

    // Reacquire each exact returned slot independently. Other scenarios may
    // use either free slot while this test waits; no global scenario lock is held.
    std::thread::scope(|scope| {
        let jobs: Vec<_> = [
            (Form::SwiftBuild, &first_path),
            (Form::SwiftTest, &second_path),
        ]
        .into_iter()
        .map(|(form, original)| {
            let tools = &tools;
            scope.spawn(move || {
                let project = tools.reacquire(
                    form,
                    &tools.directory.join(format!("reacquired-{}", form.id())),
                    original,
                );
                assert_eq!(&project, original);
                assert!(!project.join("entry.json").exists());
                assert!(!project.join("launches").exists());
                let mut again = prepare_project(project, tools, form, true, false);
                again.argv.push("--verbose".into());
                execute_and_check(tools, &again);
                tools.release_success(&again.project);
            })
        })
        .collect();
        for job in jobs {
            job.join().unwrap();
        }
    });
    assert_eq!(SEED_PREPARATIONS.load(Ordering::SeqCst), 4);
}
