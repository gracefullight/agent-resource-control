//! The discovery facts that select a project entry, and the invocation
//! directory keys of its results.
use super::*;
use arc_detector::JobClass;
use std::{fs, os::unix::ffi::OsStrExt, path::PathBuf};

fn argv(command: &str) -> Vec<OsString> {
    command.split(' ').map(OsString::from).collect()
}

/// A fixture directory beside the test binary: neither /tmp nor TMPDIR.
struct Dir(PathBuf);
impl Dir {
    fn new(name: &str) -> Self {
        let dir = std::env::current_exe().unwrap().with_file_name(format!(
            "arc-discovery-{name}-{}-{}",
            std::process::id(),
            module_path!().replace("::", "-")
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir.canonicalize().unwrap())
    }
    fn write(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_non_utf8_directory_never_shares_the_roots_result_key() {
    let root = Path::new("/w");
    let key = |dir: &Path| directory_key(root, dir).unwrap();
    let byte = |name: &[u8]| {
        root.join("packages/a")
            .join(std::ffi::OsStr::from_bytes(name))
    };
    let (ff, fe) = (byte(b"\xff"), byte(b"\xfe"));
    assert_eq!(key(root), b"");
    assert_eq!(key(&ff), b"packages/a/\xff");
    assert_ne!(key(&ff), key(&fe));
    assert_eq!(directory_key(root, Path::new("/elsewhere")), None);
    let words = vec!["bun".to_owned(), "run".to_owned(), "ci".to_owned()];
    let result = |dir: &Path| result_key(&key(dir), &words, None);
    assert_ne!(result(root), result(&ff));
    assert_ne!(result(&ff), result(&fe));
    // Nothing is recorded for a directory whose name an entry cannot hold;
    // a fact above the root only chose the entry and is not recorded.
    let observed = |path: PathBuf| root::Observed {
        absent: vec![path],
        ..root::Observed::default()
    };
    assert!(validate::discovered(&observed(ff.join("package.json")), root).is_none());
    let above = std::ffi::OsStr::from_bytes(b"/\xff/package.json");
    let above = validate::discovered(&observed(above.into()), root).unwrap();
    assert!(above.absent.is_empty());
}

#[test]
fn an_ordinary_member_script_records_the_discovery_that_selected_the_workspace() {
    let dir = Dir::new("ordinary");
    dir.write(
        "ws/package.json",
        r#"{"name":"root","workspaces":["packages/*"],"scripts":{"ci":"bun build x.ts"}}"#,
    );
    dir.write(
        "ws/packages/a/package.json",
        r#"{"name":"a","scripts":{"ci":"bun test"}}"#,
    );
    dir.write(
        "ws/packages/b/package.json",
        r#"{"name":"b","scripts":{"ci":"bun build x.ts"}}"#,
    );
    let root = dir.0.join("ws");
    let member = root.join("packages/a");
    let nested = member.join("src");
    fs::create_dir_all(&nested).unwrap();
    let open = || store::Store::at(dir.0.join("state/analysis-cache"));
    let run = |cwd: &Path| {
        let a = analyze_in(open(), &argv("bun run ci"), cwd, None);
        (a.cache, a.classification.classification)
    };
    assert_eq!(run(&member), (Outcome::Miss, JobClass::Test));
    let entry = open().unwrap().load(&store::key(&root)).unwrap();
    let files: Vec<_> = entry.files.iter().map(|f| f.path.as_str()).collect();
    assert!(files.contains(&"package.json"), "{files:?}");
    assert!(entry
        .absent
        .contains(&"packages/pnpm-workspace.yaml".to_owned()));
    assert_eq!(
        entry.memberships,
        [cache::Membership {
            declaration: "package.json".into(),
            globs: vec!["packages/*".into()],
            members: vec!["packages/a".into(), "packages/b".into()],
        }]
    );
    let reused = || {
        assert_eq!(run(&member), (Outcome::Hit, JobClass::Test));
        assert_eq!(run(&nested), (Outcome::Hit, JobClass::Test));
        assert_eq!(run(&root), (Outcome::Hit, JobClass::Build));
    };
    reused();
    // An added or removed sibling, and an edited declaration that still
    // names the member, are misses for the member's recorded result.
    for change in [
        &(|| dir.write("ws/packages/c/package.json", r#"{"name":"c"}"#)) as &dyn Fn(),
        &|| fs::remove_dir_all(root.join("packages/c")).unwrap(),
        &|| {
            dir.write(
                "ws/package.json",
                r#"{"name":"root","workspaces":["packages/a"],"scripts":{"ci":"bun build x.ts"}}"#,
            )
        },
    ] {
        change();
        assert_eq!(run(&member), (Outcome::Miss, JobClass::Test));
        reused();
    }
    // A created declaration in the member makes it a project of its own.
    dir.write("ws/packages/a/pnpm-workspace.yaml", "packages: []\n");
    assert_eq!(run(&member), (Outcome::Miss, JobClass::Test));
    assert!(open().unwrap().load(&store::key(&member)).is_some());
}
