//! The fan-out grammar and graph over an in-memory workspace port.
use super::*;
use crate::{
    scripts::{
        resolver::analyze_script_argv, MetadataError, MetadataFingerprint, MetadataStats,
        PackageSnapshot, ScriptResolution, MAX_GRAPH_EDGES,
    },
    JobClass, Locality, Uncertainty,
};
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};

#[derive(Default)]
struct Workspace {
    packages: BTreeMap<PathBuf, (Option<String>, PackageSnapshot)>,
    members: Option<Vec<PathBuf>>,
    member_calls: usize,
    /// Report the root as the CLI adapter does, to supply its name.
    report_root: bool,
}
impl Workspace {
    fn add(&mut self, dir: &str, name: Option<&str>, scripts: &[(&str, &str)]) -> &mut Self {
        self.packages.insert(
            dir.into(),
            (
                name.map(str::to_owned),
                PackageSnapshot {
                    package_dir: dir.into(),
                    fingerprint: MetadataFingerprint {
                        bytes: 10,
                        sha256: [0; 32],
                    },
                    scripts: scripts
                        .iter()
                        .map(|(n, b)| (n.to_string(), b.to_string()))
                        .collect(),
                },
            ),
        );
        self
    }
    /// Root `/w` with `a` (TEST) and `b` (BUILD).
    fn two() -> Self {
        let mut w = Self::default();
        w.add("/w", Some("root"), &[("lint", "biome check")])
            .add(
                "/w/packages/a",
                Some("a"),
                &[("test", "bun test"), ("build", "bun build x.ts")],
            )
            .add(
                "/w/packages/b",
                Some("@s/b"),
                &[("test", "vite build"), ("check", "tsc --noEmit")],
            );
        w.members = Some(vec!["/w/packages/a".into(), "/w/packages/b".into()]);
        w
    }
    fn run(&mut self, command: &str) -> ScriptResolution {
        let argv: Vec<OsString> = command.split(' ').map(OsString::from).collect();
        analyze_script_argv(&argv, Path::new("/w"), self)
    }
}
impl PackageMetadata for Workspace {
    fn package(
        &mut self,
        cwd: &Path,
        budget: &mut MetadataStats,
    ) -> std::result::Result<PackageSnapshot, MetadataError> {
        budget.lookup()?;
        let mut dir = cwd.to_path_buf();
        loop {
            if let Some((_, snapshot)) = self.packages.get(&dir) {
                return Ok(snapshot.clone());
            }
            if !dir.pop() {
                return Err(MetadataError::Missing);
            }
        }
    }
    fn revalidate(
        &mut self,
        _: &[PackageSnapshot],
        _: &mut MetadataStats,
    ) -> std::result::Result<(), MetadataError> {
        Ok(())
    }
    fn workspace_members(
        &mut self,
        root: &Path,
        _: &mut MetadataStats,
    ) -> std::result::Result<Option<Vec<WorkspaceMember>>, MetadataError> {
        self.member_calls += 1;
        if root != Path::new("/w") {
            return Err(MetadataError::Missing);
        }
        let root_entry = self.report_root.then(|| PathBuf::from("/w"));
        Ok(self.members.clone().map(|dirs| {
            root_entry
                .into_iter()
                .chain(dirs)
                .map(|dir| WorkspaceMember {
                    name: self.packages.get(&dir).and_then(|(n, _)| n.clone()),
                    dir,
                })
                .collect()
        }))
    }
}

fn summary(r: &ScriptResolution) -> Vec<(String, JobClass)> {
    r.members
        .iter()
        .map(|m| {
            (
                m.member.display().to_string(),
                m.classification.classification,
            )
        })
        .collect()
}
fn observed(r: &ScriptResolution) {
    assert_eq!(r.classification.classification, JobClass::Unknown, "{r:?}");
    assert_eq!(r.classification.uncertainty, Uncertainty::Observe);
    assert!(r.issue.is_some());
}

#[test]
fn every_runner_form_classifies_each_selected_member_and_projects_differing_classes_as_observed() {
    for command in [
        "bun run --filter * test",
        "bun --filter=* test",
        "bun run -F ./packages/* test",
        "npm run test --workspaces",
        "npm run --ws test",
        "npm run test -w a -w packages/b",
        "npm run test --workspace=@s/b --workspace=./packages/a",
        "pnpm -r run test",
        "pnpm -r test",
        "pnpm --recursive --parallel run test",
        "pnpm --filter a --filter @s/* run test",
        "pnpm --filter=./packages/* run test",
        "yarn workspaces foreach -A run test",
        "yarn workspaces foreach --all -p -j 2 --include * run test",
    ] {
        let r = Workspace::two().run(command);
        assert_eq!(
            summary(&r),
            [
                ("packages/a".to_owned(), JobClass::Test),
                ("packages/b".to_owned(), JobClass::Build)
            ],
            "{command}"
        );
        // Two local classes are neither one class nor local-plus-remote.
        assert_eq!(
            r.classification.reason,
            Reason::UnresolvedCompound,
            "{command}"
        );
        assert_eq!(r.issue, Some(ScriptIssue::Mixed), "{command}");
        observed(&r);
        assert_eq!(r.packages.len(), 3, "{command}");
    }
}

#[test]
fn selectors_exclude_members_and_a_single_class_projects_to_that_class() {
    for (command, member) in [
        ("bun run --filter a test", "packages/a"),
        ("bun run --filter ./packages/b test", "packages/b"),
        ("npm run test -w a", "packages/a"),
        ("npm run test --workspace packages/b", "packages/b"),
        ("pnpm --filter !a run test", "packages/b"),
        ("pnpm --filter a test", "packages/a"),
        ("pnpm -r --filter=!@s/b run test", "packages/a"),
        (
            "yarn workspaces foreach -A --exclude a run test",
            "packages/b",
        ),
        (
            "yarn workspaces foreach -W --include @s/* run test",
            "packages/b",
        ),
    ] {
        let r = Workspace::two().run(command);
        let class = if member == "packages/a" {
            JobClass::Test
        } else {
            JobClass::Build
        };
        assert_eq!(summary(&r), [(member.to_owned(), class)], "{command}");
        assert_eq!(r.classification.classification, class, "{command}");
        assert_eq!(r.classification.locality, Locality::HostLocal);
        assert_eq!(r.issue, None);
    }
    // Members without the script are skipped where the runner skips them.
    let r = Workspace::two().run("bun run --filter * build");
    assert_eq!(summary(&r), [("packages/a".to_owned(), JobClass::Build)]);
    assert_eq!(r.classification.reason, Reason::BunScriptBuild);
    let r = Workspace::two().run("npm run build --workspaces --if-present");
    assert_eq!(r.classification.classification, JobClass::Build);
    // Forwarded arguments reach the member body as in a single package.
    let r = Workspace::two().run("bun run --filter a test -- --watch");
    observed(&r);
}

#[test]
fn unresolved_dynamic_and_unsafe_fan_outs_are_unknown_observe() {
    for command in [
        // npm fails for a member without the script, or a `-w` naming nothing.
        "npm run build --workspaces",
        "npm run test -w nobody",
        "npm run test --workspaces --unknown-flag",
        // The root defines the script: its participation is runner config.
        "bun run --filter * lint",
        // Dynamic selectors and unsupported shapes.
        "bun run --filter !a test",
        "bun run --filter ../x test",
        "bun run --filter a? test",
        "pnpm --filter ...a run test",
        "pnpm --filter a^... run test",
        "pnpm --filter {packages/a} run test",
        "pnpm --filter ./packages/** run test",
        "pnpm -r install",
        "pnpm -r ci",
        "pnpm -r -w run test",
        "pnpm -r run",
        "yarn workspaces foreach run test",
        "yarn workspaces foreach -A -R run test",
        "yarn workspaces foreach -A --since run test",
        // Nothing selected runs the script.
        "bun run --filter nobody test",
        "pnpm --filter a run check",
        // Script names are literal.
        "bun run --filter * ./x.ts",
        "npm run --workspaces",
    ] {
        let r = Workspace::two().run(command);
        observed(&r);
    }
    // An adapter without workspace analysis never guesses members.
    let mut none = Workspace::two();
    none.members = None;
    observed(&none.run("bun run --filter * test"));
    // An unnamed member cannot be matched by name; paths still select it.
    let mut unnamed = Workspace::two();
    unnamed
        .packages
        .get_mut(Path::new("/w/packages/b"))
        .unwrap()
        .0 = None;
    observed(&unnamed.run("bun run --filter * test"));
    let r = unnamed.run("npm run test -w packages/b");
    assert_eq!(r.classification.classification, JobClass::Build);
}

#[test]
fn hooks_nesting_and_limits_are_conservative() {
    // pnpm and Yarn do not run pre/post hooks; never count leaves they skip.
    let mut hooks = Workspace::two();
    hooks.add(
        "/w/packages/a",
        Some("a"),
        &[("test", "bun test"), ("pretest", "bun build x.ts")],
    );
    observed(&hooks.run("pnpm -r --filter a run test"));
    observed(&hooks.run("yarn workspaces foreach -A --include a run test"));
    // Bun runs them, so both leaves are classified within the member.
    let r = hooks.run("bun run --filter a test");
    assert_eq!(
        r.members[0].classification.classification,
        JobClass::Unknown
    );
    observed(&r);

    // A root script may fan out with the outermost runner, not another one.
    let mut nested = Workspace::two();
    nested.add(
        "/w",
        Some("root"),
        &[
            ("ci", "bun run --filter a test"),
            ("other", "pnpm -r run test"),
        ],
    );
    let r = nested.run("bun run ci");
    assert_eq!(summary(&r), [("packages/a".to_owned(), JobClass::Test)]);
    assert_eq!(r.classification.reason, Reason::BunScriptTest);
    observed(&nested.run("bun run other"));

    // A workspace run from a member directory is not resolved.
    let mut member = Workspace::two();
    let argv: Vec<OsString> = ["bun", "run", "--filter", "*", "test"]
        .map(OsString::from)
        .to_vec();
    observed(&analyze_script_argv(
        &argv,
        Path::new("/w/packages/a"),
        &mut member,
    ));

    // The graph-edge bound covers every member edge.
    let mut wide = Workspace::default();
    wide.add("/w", Some("root"), &[]);
    let mut dirs = Vec::new();
    for at in 0..MAX_GRAPH_EDGES {
        let dir = format!("/w/p{at}");
        wide.add(&dir, Some(&format!("p{at}")), &[("test", "bun test")]);
        dirs.push(PathBuf::from(dir));
    }
    wide.members = Some(dirs);
    let r = wide.run("bun run --filter * test");
    assert!(
        matches!(
            r.issue,
            Some(ScriptIssue::GraphLimit | ScriptIssue::Metadata(MetadataError::LookupLimit))
        ) || matches!(
            r.classification.reason,
            Reason::InvocationLimit | Reason::WorkLimit | Reason::TokenLimit
        ),
        "{r:?}"
    );
    observed(&r);
}

#[test]
fn glob_wildcards_stay_within_a_segment_only_for_paths() {
    let mut b = Budget::default();
    for (pattern, text, segment, expected) in [
        ("*", "anything/at/all", false, true),
        ("@s/*", "@s/b", false, true),
        ("@s/*", "@t/b", false, false),
        ("packages/*", "packages/a", true, true),
        ("packages/*", "packages/a/b", true, false),
        ("p*s/*b", "packages/ab", true, true),
        ("a", "ab", true, false),
        ("a*", "a", true, true),
    ] {
        assert_eq!(
            glob(pattern, text, segment, &mut b),
            Ok(expected),
            "{pattern} {text}"
        );
    }
    assert!(is_runner("npm") && is_runner("/usr/bin/pnpm") && is_runner("yarn"));
    assert!(!is_runner("bun") && !is_runner("cargo"));
}

#[test]
fn pnpm_resolves_script_shorthand_that_is_not_a_built_in_command() {
    let mut w = Workspace::two();
    w.add(
        "/w/packages/a",
        Some("a"),
        &[
            ("test", "bun test"),
            ("verify:ci", "bun test"),
            ("start", "bun test"),
        ],
    )
    .add(
        "/w/packages/b",
        Some("@s/b"),
        &[("test", "vite build"), ("verify:ci", "vite build")],
    );
    let r = w.run("pnpm -r verify:ci");
    assert_eq!(
        summary(&r),
        [
            ("packages/a".to_owned(), JobClass::Test),
            ("packages/b".to_owned(), JobClass::Build)
        ]
    );
    observed(&r);
    let r = w.run("pnpm --filter a verify:ci");
    assert_eq!(summary(&r), [("packages/a".to_owned(), JobClass::Test)]);
    assert_eq!(r.classification.classification, JobClass::Test);
    assert_eq!(r.issue, None);
    // `t` is the built-in alias of `test`; `start` runs the start script.
    let r = w.run("pnpm --filter a t");
    assert_eq!(r.classification.classification, JobClass::Test);
    assert_eq!(
        w.run("pnpm --filter a start").classification.classification,
        JobClass::Test
    );
    // Shorthand must be defined by every selected member: pnpm may run a
    // binary of that name in a member without the script.
    observed(&w.run("pnpm -r start"));
    observed(&w.run("pnpm -r check"));
    // Built-in commands are never scripts, even when a script shares a name.
    w.add(
        "/w/packages/a",
        Some("a"),
        &[("install", "bun test"), ("why", "bun test")],
    );
    for command in [
        "pnpm --filter a install",
        "pnpm --filter a why",
        "pnpm -r exec",
        "pnpm -r ci",
    ] {
        observed(&w.run(command));
    }
}

#[test]
fn root_participation_follows_the_selection_and_explicit_npm_configuration() {
    let mut w = Workspace::two();
    w.report_root = true;
    w.add("/w", Some("root"), &[("test", "bun build x.ts")]);
    // Name filters that do not match the root exclude it.
    for command in [
        "bun run --filter a test",
        "bun --filter @s/* test",
        "bun run --filter ./packages/a test",
        "npm run test -w a --no-include-workspace-root",
    ] {
        let r = w.run(command);
        assert!(!r.members.is_empty(), "{command}");
        assert!(
            r.members.iter().all(|m| m.member != Path::new(".")),
            "{command}"
        );
    }
    let r = w.run("bun run --filter a test");
    assert_eq!(r.classification.classification, JobClass::Test);
    // A filter matching the root's name: Bun runs it too; not resolved.
    observed(&w.run("bun run --filter * test"));
    observed(&w.run("bun run --filter roo* test"));
    // npm without explicit evidence leaves the root to its configuration.
    observed(&w.run("npm run test -w a"));
    observed(&w.run("npm run test --workspaces"));
    // With the root included, its script is member `.`, classified first.
    let r = w.run("npm run test -w a --include-workspace-root");
    assert_eq!(
        summary(&r),
        [
            (".".to_owned(), JobClass::Build),
            ("packages/a".to_owned(), JobClass::Test)
        ]
    );
    observed(&r);
    let r = w.run("npm run test -w packages/b --include-workspace-root=true");
    assert_eq!(summary(&r).len(), 2);
    assert_eq!(r.classification.classification, JobClass::Build);
    // An included root without the script fails in npm unless --if-present.
    observed(&w.run("npm run check -w @s/b --include-workspace-root"));
    let r = w.run("npm run check -w @s/b --include-workspace-root --if-present");
    assert_eq!(
        summary(&r),
        [("packages/b".to_owned(), JobClass::Typecheck)]
    );
    // An unnamed root cannot be told apart from a name filter.
    let mut unnamed = Workspace::two();
    unnamed.report_root = true;
    unnamed.add("/w", None, &[("test", "bun test")]);
    observed(&unnamed.run("bun run --filter a test"));
    let r = unnamed.run("bun run --filter ./packages/a test");
    assert_eq!(r.classification.classification, JobClass::Test);
    // pnpm and Yarn keep a root script unresolved unless paths are selected.
    observed(&w.run("pnpm --filter a run test"));
    observed(&w.run("yarn workspaces foreach -A --include a run test"));
    assert_eq!(
        w.run("pnpm --filter ./packages/a run test")
            .classification
            .classification,
        JobClass::Test
    );
}
