//! Keep small Swift failure evidence without retaining copied compiler trees.
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

const PASSED: &str = ".arc-swift-case-passed";

pub(super) fn mark_success(project: &Path) {
    fs::write(project.join(PASSED), b"passed").expect("record Swift case completion");
}

pub(super) struct Projects {
    directory: PathBuf,
    projects: Mutex<Vec<PathBuf>>,
}

impl Projects {
    pub(super) fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            projects: Mutex::new(Vec::new()),
        }
    }

    pub(super) fn register(&self, project: &Path) {
        self.projects
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(project.to_owned());
    }
}

fn remove_directory(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

fn preserve_plugin_evidence(project: &Path, destination: &Path) -> io::Result<()> {
    let outputs = project.join("scratch path/plugins/outputs");
    match fs::symlink_metadata(&outputs) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
        Ok(metadata) if !metadata.file_type().is_dir() => {
            return Err(io::Error::other("Swift plugin outputs are not a directory"));
        }
        Ok(_) => {}
    }
    let mut pending = vec![(outputs.clone(), 0)];
    let mut visited = 0;
    while let Some((directory, depth)) = pending.pop() {
        visited += 1;
        if visited > 4096 || depth > 12 {
            return Err(io::Error::other("Swift plugin evidence traversal bound"));
        }
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push((entry.path(), depth + 1));
            } else if kind.is_file()
                && matches!(
                    entry.file_name().to_str(),
                    Some("entry.json" | "semantic.json" | "bodies" | "ARCGenerated.swift")
                )
            {
                let path = entry.path();
                let relative = path.strip_prefix(&outputs).unwrap();
                let target = destination.join("failure-plugin-outputs").join(relative);
                fs::create_dir_all(target.parent().unwrap())?;
                fs::copy(path, target)?;
            }
        }
    }
    Ok(())
}

fn compact_failure(project: &Path) -> io::Result<()> {
    // Never discard the only diagnostic copy if preservation fails.
    preserve_plugin_evidence(project, project)?;
    let mut cleanup = Ok(());
    for name in [
        "scratch path",
        "module-cache",
        "clang-cache",
        "private-home",
        "config",
    ] {
        if let Err(error) = remove_directory(&project.join(name)) {
            if cleanup.is_ok() {
                cleanup = Err(error);
            }
        }
    }
    cleanup
}

/// Save only the scenario's observations before its in-place tree is reused.
pub(super) fn capture_failure(project: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    preserve_plugin_evidence(project, destination)?;
    for name in [
        "form",
        "sdk-path",
        "fail",
        "release",
        "launches",
        "release-witness",
        "native-child.json",
        "entry.json",
        "semantic.json",
        "bodies",
        "stdout",
        "stderr",
        "direct-stdout",
        "direct-stderr",
    ] {
        let source = project.join(name);
        let metadata = match fs::symlink_metadata(&source) {
            Ok(metadata) if metadata.file_type().is_file() => metadata,
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        // Retain the diagnostic tail while bounding failure artifacts.
        let mut input = fs::File::open(source)?;
        use std::io::{Read, Seek, SeekFrom, Write};
        const LIMIT: u64 = 1024 * 1024;
        input.seek(SeekFrom::Start(metadata.len().saturating_sub(LIMIT)))?;
        let mut output = fs::File::create(destination.join(name))?;
        if metadata.len() > LIMIT {
            output.write_all(b"[earlier output omitted]\n")?;
        }
        std::io::copy(&mut input.take(LIMIT), &mut output)?;
    }
    Ok(())
}

impl Drop for Projects {
    fn drop(&mut self) {
        let failed = std::thread::panicking();
        let projects = self
            .projects
            .get_mut()
            .unwrap_or_else(|error| error.into_inner());
        let mut errors = Vec::new();
        for project in projects {
            let result = if failed && !project.join(PASSED).is_file() {
                compact_failure(project)
            } else {
                remove_directory(project)
            };
            if let Err(error) = result {
                errors.push(format!("{}: {error}", project.display()));
            }
        }
        if !failed {
            if let Err(error) = remove_directory(&self.directory) {
                errors.push(format!("{}: {error}", self.directory.display()));
            }
        } else {
            if let Err(error) = remove_directory(&self.directory.join("observers")) {
                errors.push(format!("{}: {error}", self.directory.display()));
            }
            match fs::remove_dir(&self.directory) {
                Ok(()) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::DirectoryNotEmpty
                    ) => {}
                Err(error) => errors.push(format!("{}: {error}", self.directory.display())),
            }
        }
        if !errors.is_empty() {
            let message = format!("cannot clean owned Swift fixtures: {}", errors.join(", "));
            if failed {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, MetadataExt};

    fn allocated_bytes(path: &Path) -> u64 {
        let metadata = fs::symlink_metadata(path).unwrap();
        let mut blocks = metadata.blocks();
        if metadata.file_type().is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                blocks += allocated_bytes(&entry.unwrap().path()) / 512;
            }
        }
        blocks * 512
    }

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "arc-swift-cleanup-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn populate(project: &Path) {
        let evidence = project.join("scratch path/plugins/outputs/package/arc-swift-evidence");
        fs::create_dir_all(&evidence).unwrap();
        fs::write(evidence.join("entry.json"), b"native entry").unwrap();
        fs::write(evidence.join("semantic.json"), b"native semantics").unwrap();
        fs::write(evidence.join("not-evidence"), b"discard").unwrap();
        fs::create_dir_all(project.join("module-cache")).unwrap();
        fs::write(project.join("module-cache/large"), vec![1; 1024 * 1024]).unwrap();
        fs::write(project.join("direct-stderr"), b"tool diagnostic").unwrap();
    }

    #[test]
    fn swift_failed_evidence_preservation_keeps_original_build_tree() {
        let root = fixture();
        let project = root.join("case");
        let plugins = project.join("scratch path/plugins");
        fs::create_dir_all(&plugins).unwrap();
        fs::write(plugins.join("outputs"), b"unexpected diagnostic input").unwrap();
        fs::create_dir_all(project.join("module-cache")).unwrap();
        assert!(compact_failure(&project).is_err());
        assert_eq!(
            fs::read(plugins.join("outputs")).unwrap(),
            b"unexpected diagnostic input"
        );
        assert!(project.join("module-cache").is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn swift_owned_projects_remove_successful_siblings_on_failure() {
        let root = fixture();
        let tools = root.join("tools");
        let success = tools.join("passed");
        let failure = tools.join("failed");
        populate(&success);
        populate(&failure);
        // A completed case records its own outcome before its sibling panics.
        mark_success(&success);
        assert!(std::panic::catch_unwind(|| {
            let projects = Projects::new(tools.clone());
            projects.register(&success);
            projects.register(&failure);
            panic!("one sibling failed");
        })
        .is_err());
        assert!(!success.exists(), "successful sibling must be removed");
        assert!(failure.is_dir(), "failed sibling evidence must remain");
        assert_eq!(
            fs::read(failure.join("direct-stderr")).unwrap(),
            b"tool diagnostic"
        );
        assert!(!failure.join("scratch path").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn swift_owned_projects_remove_success_and_compact_failure_evidence() {
        let root = fixture();
        let unrelated = root.join("unrelated");
        fs::create_dir_all(&unrelated).unwrap();
        fs::write(unrelated.join("sentinel"), b"keep").unwrap();
        let success_tools = root.join("success tools");
        let success_project = root.join("success project");
        fs::create_dir_all(&success_tools).unwrap();
        populate(&success_project);
        let projects = Projects::new(success_tools.clone());
        projects.register(&success_project);
        drop(projects);
        assert!(!success_tools.exists());
        assert!(!success_project.exists());

        let failure_tools = root.join("failure tools");
        let failure_project = root.join("failure project");
        fs::create_dir_all(&failure_tools).unwrap();
        populate(&failure_project);
        let outputs = failure_project.join("scratch path/plugins/outputs");
        symlink(&unrelated, outputs.join("outside")).unwrap();
        assert!(std::panic::catch_unwind(|| {
            let projects = Projects::new(failure_tools.clone());
            projects.register(&failure_project);
            panic!("simulated fixture failure");
        })
        .is_err());
        let retained = failure_project.join("failure-plugin-outputs/package/arc-swift-evidence");
        assert_eq!(
            fs::read(retained.join("entry.json")).unwrap(),
            b"native entry"
        );
        assert_eq!(
            fs::read(retained.join("semantic.json")).unwrap(),
            b"native semantics"
        );
        assert!(!retained.join("not-evidence").exists());
        assert!(!failure_project
            .join("failure-plugin-outputs/outside")
            .exists());
        assert!(!failure_project.join("scratch path").exists());
        assert!(!failure_project.join("module-cache").exists());
        assert_eq!(
            fs::read(failure_project.join("direct-stderr")).unwrap(),
            b"tool diagnostic"
        );
        assert!(
            allocated_bytes(&failure_project) < 256 * 1024,
            "compacted failure retained a large build artifact"
        );
        assert_eq!(fs::read(unrelated.join("sentinel")).unwrap(), b"keep");
        fs::remove_dir_all(root).unwrap();
    }
}
