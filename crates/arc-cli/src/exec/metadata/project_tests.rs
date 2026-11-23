//! The workspace adapter and the project analysis cache over real files.
use super::*;
use arc_detector::{
    scripts::{MetadataStats, PackageMetadata},
    JobClass, Reason,
};
use std::{fs, path::PathBuf};

/// A fixture directory beside the test binary: neither /tmp nor TMPDIR.
struct Dir(PathBuf);
impl Dir {
    fn new(name: &str) -> Self {
        let dir = std::env::current_exe().unwrap().with_file_name(format!(
            "arc-project-{name}-{}-{}",
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
    fn workspace(&self) -> PathBuf {
        let root = self.0.join("ws");
        self.write(
            "ws/package.json",
            r#"{"name":"root","workspaces":["packages/*"],"scripts":{"lint":"biome check"}}"#,
        );
        self.write("ws/pnpm-workspace.yaml", "packages:\n  - 'apps/*'\n");
        self.write(
            "ws/packages/a/package.json",
            r#"{"name":"a","scripts":{"ci":"bun test"}}"#,
        );
        self.write(
            "ws/packages/b/package.json",
            r#"{"name":"b","scripts":{"ci":"bun build x.ts"}}"#,
        );
        self.write(
            "ws/apps/web/package.json",
            r#"{"name":"web","scripts":{"ci":"vitest run"}}"#,
        );
        // Not members: hidden, node_modules, no manifest, a symlink.
        self.write("ws/packages/.hidden/package.json", r#"{"name":"h"}"#);
        self.write("ws/packages/node_modules/package.json", r#"{"name":"n"}"#);
        fs::create_dir_all(root.join("packages/empty")).unwrap();
        std::os::unix::fs::symlink(root.join("packages/a"), root.join("packages/link")).unwrap();
        root
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn argv(command: &str) -> Vec<OsString> {
    command.split(' ').map(OsString::from).collect()
}
fn members(analysis: &Analysis) -> Vec<(String, JobClass)> {
    analysis
        .members
        .iter()
        .map(|m| {
            (
                m.member.display().to_string(),
                m.classification.classification,
            )
        })
        .collect()
}

#[test]
fn members_follow_each_runners_declaration_and_revalidation_rechecks_membership() {
    let dir = Dir::new("members");
    let root = dir.workspace();
    let mut source = FreshMetadata::for_program("bun");
    let mut result =
        arc_detector::analyze_script_argv(&argv("bun run --filter * ci"), &root, &mut source);
    assert_eq!(result.classification.reason, Reason::UnresolvedCompound);
    let listed: Vec<_> = result.members.iter().map(|m| m.member.clone()).collect();
    assert_eq!(listed, [PathBuf::from("packages/a"), "packages/b".into()]);
    source
        .revalidate(&result.packages, &mut result.metadata)
        .unwrap();
    // pnpm reads only pnpm-workspace.yaml.
    let mut pnpm = FreshMetadata::for_program("/usr/local/bin/pnpm");
    let r = arc_detector::analyze_script_argv(&argv("pnpm -r run ci"), &root, &mut pnpm);
    assert_eq!(r.members.len(), 1);
    assert_eq!(r.classification.classification, JobClass::Test);

    // An added member, a removed member and a changed declaration all fail.
    dir.write("ws/packages/c/package.json", r#"{"name":"c","scripts":{}}"#);
    assert_eq!(
        source.revalidate(&result.packages, &mut MetadataStats::default()),
        Err(arc_detector::scripts::MetadataError::Changed)
    );
    fs::remove_dir_all(root.join("packages/c")).unwrap();
    source
        .revalidate(&result.packages, &mut MetadataStats::default())
        .unwrap();
    fs::remove_dir_all(root.join("packages/b")).unwrap();
    assert!(source
        .revalidate(&result.packages, &mut MetadataStats::default())
        .is_err());
    dir.write("ws/pnpm-workspace.yaml", "packages:\n  - 'packages/*'\n");
    assert!(pnpm
        .revalidate(&r.packages, &mut MetadataStats::default())
        .is_err());
    // A workspace listed twice in one analysis is refused.
    assert!(source
        .workspace_members(&root, &mut MetadataStats::default())
        .is_err());
}

#[test]
fn one_project_entry_serves_every_selector_runner_and_script_and_misses_on_any_change() {
    let dir = Dir::new("cache");
    let root = dir.workspace();
    let open = || store::Store::at(dir.0.join("state/analysis-cache"));
    let run = |command: &str| analyze_in(open(), &argv(command), &root, None);
    let first = run("bun run --filter * ci");
    assert_eq!(first.cache, Outcome::Miss);
    let hit = run("bun run --filter * ci");
    assert_eq!(hit.cache, Outcome::Hit);
    assert_eq!(hit.classification, first.classification);
    assert_eq!(members(&hit), members(&first));
    // Another selector and an ordinary root script reuse the project's
    // validated analysis instead of starting a new entry.
    let selected = run("bun run --filter a ci");
    assert_eq!(selected.cache, Outcome::Hit);
    assert_eq!(selected.classification.classification, JobClass::Test);
    assert_eq!(members(&selected), [("packages/a".into(), JobClass::Test)]);
    let ordinary = run("bun run lint");
    assert_eq!(ordinary.cache, Outcome::Hit);
    assert_eq!(ordinary.classification.classification, JobClass::Lint);
    // pnpm needs its own declaration, which then joins the same entry.
    assert_eq!(run("pnpm -r run ci").cache, Outcome::Miss);
    let web = run("pnpm --filter web run ci");
    assert_eq!(web.cache, Outcome::Hit);
    assert_eq!(members(&web), [("apps/web".into(), JobClass::Test)]);
    assert_eq!(run("bun run --filter b ci").cache, Outcome::Hit);
    let entries = || {
        fs::read_dir(dir.0.join("state/analysis-cache"))
            .unwrap()
            .count()
    };
    assert_eq!(entries(), 1, "one entry per project root");
    // Equal bytes under a new identity are a hit; new bytes are a miss for
    // every classification of the project, recorded or not.
    let path = root.join("packages/a/package.json");
    let bytes = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert_eq!(run("bun run --filter * ci").cache, Outcome::Hit);
    dir.write(
        "ws/packages/a/package.json",
        r#"{"name":"a","scripts":{"ci":"bun build y.ts"}}"#,
    );
    let edited = run("bun run --filter a ci");
    assert_eq!(edited.cache, Outcome::Miss);
    assert_eq!(edited.classification.classification, JobClass::Build);
    let after = run("bun run --filter * ci");
    assert_eq!(after.cache, Outcome::Hit);
    assert_eq!(after.classification.classification, JobClass::Build);
    dir.write(
        "ws/package.json",
        r#"{"name":"root","workspaces":["packages/*"],"scripts":{"lint":"tsc --noEmit"}}"#,
    );
    let lint = run("bun run lint");
    assert_eq!(lint.cache, Outcome::Miss);
    assert_eq!(lint.classification.classification, JobClass::Typecheck);
    // A member that gains a manifest joins the membership.
    assert_eq!(run("bun run --filter * ci").cache, Outcome::Miss);
    dir.write(
        "ws/packages/empty/package.json",
        r#"{"name":"e","scripts":{}}"#,
    );
    let joined = run("bun run --filter * ci");
    assert_eq!(joined.cache, Outcome::Miss);
    assert_eq!(run("bun run --filter * ci").cache, Outcome::Hit);
    assert_eq!(run("bun run --filter b ci").cache, Outcome::Hit);
    fs::remove_dir_all(root.join("packages/empty")).unwrap();
    assert_eq!(run("bun run --filter b ci").cache, Outcome::Miss);
    // Without a store nothing is kept.
    let none = analyze_in(None, &argv("bun run --filter * ci"), &root, None);
    assert_eq!(none.cache, Outcome::None);
    assert!(analyze(&argv("bun run ci"), &root, None).is_some());
    // A corrupt or foreign entry is never used.
    for entry in fs::read_dir(dir.0.join("state/analysis-cache")).unwrap() {
        fs::write(entry.unwrap().path(), b"{\"version\":2}").unwrap();
    }
    assert_eq!(run("bun run --filter * ci").cache, Outcome::Miss);
}

#[test]
fn ordinary_scripts_share_the_project_entry_and_revalidate_content() {
    let dir = Dir::new("ordinary");
    dir.write(
        "project/package.json",
        r#"{"scripts":{"test":"bun test","build":"bun build x.ts"}}"#,
    );
    let root = dir.0.join("project");
    let open = || store::Store::at(dir.0.join("state/analysis-cache"));
    let run = |command: &str| analyze_in(open(), &argv(command), &root, None);
    assert_eq!(run("bun run test").cache, Outcome::Miss);
    let other = run("bun run build");
    assert_eq!(other.cache, Outcome::Hit);
    assert_eq!(other.classification.classification, JobClass::Build);
    assert_eq!(run("bun run build").cache, Outcome::Hit);
    let store = open().unwrap();
    let key = store::key(&root);
    let mut corrupt = store.load(&key).unwrap();
    assert_eq!(corrupt.results.len(), 2);
    // The file identity is untouched; an incorrect digest must still miss.
    corrupt.files[0].sha256 = "0".repeat(64);
    store.save(&key, &corrupt);
    assert_eq!(run("bun run build").cache, Outcome::Miss);
    dir.write(
        "project/package.json",
        r#"{"scripts":{"test":"bun test","build":"bun test"}}"#,
    );
    let changed = run("bun run build");
    assert_eq!(changed.cache, Outcome::Miss);
    assert_eq!(changed.classification.classification, JobClass::Test);
    // The edit dropped the results computed over the old bytes.
    assert_eq!(store.load(&key).unwrap().results.len(), 1);
    assert_eq!(run("bun run test").cache, Outcome::Hit);
}

#[test]
fn a_question_mark_glob_matches_one_character_of_any_width() {
    let dir = Dir::new("unicode");
    dir.write(
        "u/package.json",
        r#"{"name":"root","workspaces":["packages/?"]}"#,
    );
    dir.write(
        "u/packages/a/package.json",
        r#"{"name":"a","scripts":{"ci":"bun test"}}"#,
    );
    dir.write(
        "u/packages/한/package.json",
        r#"{"name":"k","scripts":{"ci":"bun build x.ts"}}"#,
    );
    dir.write(
        "u/packages/ab/package.json",
        r#"{"name":"ab","scripts":{"ci":"bun test"}}"#,
    );
    let root = dir.0.join("u");
    let open = || store::Store::at(dir.0.join("state/analysis-cache"));
    let r = analyze_in(open(), &argv("bun run --filter * ci"), &root, None);
    assert_eq!(
        members(&r),
        [
            ("packages/a".into(), JobClass::Test),
            ("packages/한".into(), JobClass::Build)
        ]
    );
    assert_eq!(r.classification.classification, JobClass::Unknown);
    assert_eq!(
        r.classification.uncertainty,
        arc_detector::Uncertainty::Observe
    );
    let hit = analyze_in(open(), &argv("bun run --filter * ci"), &root, None);
    assert_eq!((hit.cache, members(&hit)), (Outcome::Hit, members(&r)));
}

#[test]
fn changed_file_and_membership_during_validation_fail_closed() {
    let dir = Dir::new("race");
    let root = dir.workspace();
    let path = root.join("packages/a/package.json");
    let replacement = path.with_extension("replacement");
    fs::write(&replacement, r#"{"scripts":{"ci":"bun build x.ts"}}"#).unwrap();
    assert_eq!(
        fresh::race_read(&path, || fs::rename(&replacement, &path).unwrap()),
        Err(arc_detector::scripts::MetadataError::Changed)
    );
    dir.write(
        "ws/packages/a/package.json",
        r#"{"name":"a","scripts":{"ci":"bun test"}}"#,
    );
    let mut source = FreshMetadata::for_program("bun");
    let fresh_result =
        arc_detector::analyze_script_argv(&argv("bun run --filter * ci"), &root, &mut source);
    assert_eq!(
        source.revalidate_inner(
            &fresh_result.packages,
            &mut MetadataStats::default(),
            || {
                fs::write(&path, r#"{"name":"a","scripts":{"ci":"bun build x.ts"}}"#).unwrap();
            }
        ),
        Err(arc_detector::scripts::MetadataError::Changed)
    );
    let store = store::Store::at(dir.0.join("state/analysis-cache")).unwrap();
    let command = argv("bun run --filter * ci");
    let reopen = || Some(store::Store::at(dir.0.join("state/analysis-cache")).unwrap());
    assert_eq!(
        analyze_in(reopen(), &command, &root, None).cache,
        Outcome::Miss
    );
    let key = store::key(&root);
    let mut cached = store.load(&key).unwrap();
    let late_replacement = path.with_extension("late-replacement");
    fs::write(&late_replacement, r#"{"scripts":{"ci":"bun test"}}"#).unwrap();
    assert!(validate::validate_with(&mut cached, &root, || {
        fs::rename(&late_replacement, &path).unwrap();
    })
    .is_none());
    assert_eq!(
        analyze_in(reopen(), &command, &root, None).cache,
        Outcome::Miss
    );
    let mut cached = store.load(&key).unwrap();
    assert!(validate::validate_with(&mut cached, &root, || {
        dir.write(
            "ws/packages/c/package.json",
            r#"{"name":"c","scripts":{"ci":"bun test"}}"#,
        );
    })
    .is_none());
}

#[test]
fn root_member_and_member_subdirectory_share_one_entry_with_per_directory_results() {
    let dir = Dir::new("directories");
    let root = dir.workspace();
    let member = root.join("packages/a");
    let nested = member.join("src/deep");
    fs::create_dir_all(&nested).unwrap();
    let state = dir.0.join("state/analysis-cache");
    let open = || store::Store::at(state.clone());
    let run = |cwd: &Path, command: &str| analyze_in(open(), &argv(command), cwd, None);
    let class = |a: &Analysis| a.classification.classification;
    assert_eq!(run(&root, "bun run --filter * ci").cache, Outcome::Miss);
    // The member's manifest was read by the fan-out: its directory and a
    // subdirectory without a manifest reuse the validated entry.
    for cwd in [&member, &nested] {
        let first = run(cwd, "bun run ci");
        assert_eq!((first.cache, class(&first)), (Outcome::Hit, JobClass::Test));
        assert_eq!(run(cwd, "bun run ci").cache, Outcome::Hit);
    }
    assert_eq!(
        fs::read_dir(&state).unwrap().count(),
        1,
        "one project entry"
    );
    let entry = open().unwrap().load(&store::key(&root)).unwrap();
    assert!(entry
        .absent
        .contains(&"packages/a/src/deep/package.json".to_owned()));
    // Results never answer another directory: the root's fan-out result is
    // not the member's, whose own fan-out is not a workspace root.
    let inside = run(&member, "bun run --filter * ci");
    assert_eq!(class(&inside), JobClass::Unknown);
    assert_eq!(
        class(&run(&root, "bun run --filter * ci")),
        JobClass::Unknown
    );
    // An edited member manifest is a miss in every directory's first use.
    dir.write(
        "ws/packages/a/package.json",
        r#"{"name":"a","scripts":{"ci":"bun build y.ts"}}"#,
    );
    let edited = run(&nested, "bun run ci");
    assert_eq!(
        (edited.cache, class(&edited)),
        (Outcome::Miss, JobClass::Build)
    );
    let reused = run(&member, "bun run ci");
    assert_eq!(
        (reused.cache, class(&reused)),
        (Outcome::Hit, JobClass::Build)
    );
    // A nearer manifest created in the subdirectory is a miss there: the
    // workspace entry's absent path fails, and the unlisted package is a
    // project of its own.
    dir.write(
        "ws/packages/a/src/package.json",
        r#"{"name":"src","scripts":{"ci":"bun test"}}"#,
    );
    let nearer = run(&nested, "bun run ci");
    assert_eq!(
        (nearer.cache, class(&nearer)),
        (Outcome::Miss, JobClass::Test)
    );
    let src = root.join("packages/a/src");
    assert!(open().unwrap().load(&store::key(&src)).is_some());
    // Removed again, the workspace entry validates as it was recorded.
    fs::remove_file(src.join("package.json")).unwrap();
    let removed = run(&nested, "bun run ci");
    assert_eq!(
        (removed.cache, class(&removed)),
        (Outcome::Hit, JobClass::Build)
    );
    // A package the globs do not name keeps a separate entry.
    dir.write(
        "ws/tools/package.json",
        r#"{"name":"tools","scripts":{"ci":"bun test"}}"#,
    );
    let tools = run(&root.join("tools"), "bun run ci");
    assert_eq!(
        (tools.cache, class(&tools)),
        (Outcome::Miss, JobClass::Test)
    );
    assert_eq!(fs::read_dir(&state).unwrap().count(), 3);
}
