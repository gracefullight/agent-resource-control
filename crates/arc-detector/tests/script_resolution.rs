#[path = "script_resolution/budgets.rs"]
mod budgets;

use arc_detector::{analyze_script_argv, scripts::*, *};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

#[derive(Default)]
struct Memory {
    packages: BTreeMap<PathBuf, PackageSnapshot>,
    retained: BTreeSet<PathBuf>,
}
fn normal(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => (),
            _ => result.push(part),
        }
    }
    result
}
impl Memory {
    fn add(&mut self, dir: &str, scripts: &[(&str, &str)]) {
        self.packages.insert(
            dir.into(),
            PackageSnapshot {
                package_dir: dir.into(),
                fingerprint: MetadataFingerprint {
                    bytes: 100,
                    sha256: [0; 32],
                },
                scripts: scripts
                    .iter()
                    .map(|(n, b)| (n.to_string(), b.to_string()))
                    .collect(),
            },
        );
    }
    fn resolve(&mut self, argv: &[&str]) -> ScriptResolution {
        analyze_script_argv(
            &argv.iter().map(OsString::from).collect::<Vec<_>>(),
            Path::new("/project"),
            self,
        )
    }
}
impl PackageMetadata for Memory {
    fn package(
        &mut self,
        cwd: &Path,
        budget: &mut MetadataStats,
    ) -> Result<PackageSnapshot, MetadataError> {
        let mut dir = normal(cwd);
        loop {
            budget.lookup()?;
            if let Some(snapshot) = self.packages.get(&dir) {
                if self.retained.insert(dir) {
                    budget.file()?;
                    budget.read(snapshot.fingerprint.bytes)?;
                    budget.scripts(&snapshot.scripts)?;
                }
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
    ) -> Result<(), MetadataError> {
        panic!("Pure analysis must not perform a revalidation or any process operation")
    }
}

fn supported(body: &str, class: JobClass, reason: Reason) {
    let mut metadata = Memory::default();
    metadata.add("/project", &[("totally-misleading", body)]);
    let result = metadata.resolve(&["bun", "run", "totally-misleading"]);
    assert_eq!(
        result.classification.classification, class,
        "{body}: {result:?}"
    );
    assert_eq!(result.classification.reason, reason);
    assert_eq!(result.classification.locality, Locality::HostLocal);
    assert_eq!(result.issue, None);
    assert!(result.classification.is_consistent());
}
#[test]
fn leaf_commands_not_script_names_determine_all_four_classes_and_same_class_chains() {
    for body in [
        "bun build src/index.ts --outdir dist",
        "vite build --mode production",
        "vite build && bun build src/a.ts --outfile dist/a.js",
    ] {
        supported(body, JobClass::Build, Reason::BunScriptBuild);
    }
    for body in [
        "bun test",
        "bun test tests --timeout 3000",
        "bun test a.test.ts && bun test b.test.ts",
    ] {
        supported(body, JobClass::Test, Reason::BunScriptTest);
    }
    supported(
        "biome check . --error-on-warnings",
        JobClass::Lint,
        Reason::BunScriptLint,
    );
    for body in [
        "tsc --noEmit -p tsconfig.json && tsc --noEmit -p tsconfig.test.json",
        "tsc --noEmit true --strict false --project tsconfig.json",
        "tsc --noEmit true file.ts",
    ] {
        supported(body, JobClass::Typecheck, Reason::BunScriptTypecheck);
    }
}

fn assert_typecheck_bypass(body: &str, forwarded: &[&str]) {
    let mut source = Memory::default();
    source.add("/project", &[("entry", "bun run target"), ("target", body)]);
    let mut argv = vec!["bun", "run", "entry"];
    argv.extend_from_slice(forwarded);
    let result = source.resolve(&argv);
    assert_eq!(
        result.classification.classification,
        JobClass::Unknown,
        "{body} {forwarded:?}: {result:?}"
    );
    assert_eq!(result.classification.uncertainty, Uncertainty::Observe);
    assert!(!result.classification.is_bun_script());
    assert_eq!(result.issue, Some(ScriptIssue::Unsupported));
}

#[test]
fn typecheck_response_files_in_bodies_lifecycles_and_forwarded_args_bypass() {
    for body in [
        "tsc --noEmit @flags",
        "tsc --noEmit '@flags with spaces'",
        "tsc @flags --noEmit",
        "tsc --noEmit -p @flags",
    ] {
        assert_typecheck_bypass(body, &[]);
    }
    for forwarded in [vec!["@flags"], vec!["--", "@flags with spaces"]] {
        assert_typecheck_bypass("tsc --noEmit", &forwarded);
    }
    for hook in ["pretarget", "posttarget"] {
        let mut source = Memory::default();
        source.add(
            "/project",
            &[("target", "tsc --noEmit"), (hook, "tsc --noEmit @flags")],
        );
        let result = source.resolve(&["bun", "run", "target"]);
        assert_eq!(result.classification.classification, JobClass::Unknown);
        assert_eq!(result.classification.uncertainty, Uncertainty::Observe);
        assert_eq!(result.issue, Some(ScriptIssue::Unsupported));
    }
}

#[test]
fn typecheck_option_spellings_and_arities_require_separated_arguments() {
    for body in [
        "tsc --noEmit=true",
        "tsc --noEmit --project=tsconfig.json",
        "tsc --noEmit -p=tsconfig.json",
        "tsc --noEmit --strict=true",
        "tsc --noEmit --strict=false",
        "tsc --noEmit --target=esnext",
        "tsc --noEmit --pretty=false",
        "tsc --noEmit -p",
        "tsc --noEmit --project --strict",
        "tsc --noEmit --target",
        "tsc --noEmit --project ''",
        "tsc --noEmit --noEmit false",
    ] {
        assert_typecheck_bypass(body, &[]);
    }
    for forwarded in [
        vec!["--noEmit=true"],
        vec!["--project=tsconfig.json"],
        vec!["--", "--strict=false"],
        vec!["--noEmit", "false"],
        vec!["-p"],
    ] {
        assert_typecheck_bypass("tsc --noEmit", &forwarded);
    }
    for body in [
        "tsc --noEmit true -p tsconfig.json",
        "tsc --project tsconfig.json --noEmit",
        "tsc --noEmit --strict true --skipLibCheck false --pretty false file.ts",
        "tsc --noEmit --target esnext --module esnext --moduleResolution bundler --lib esnext,dom --types bun --jsx react-jsx --tsBuildInfoFile build/cache.tsbuildinfo",
        "tsc --noEmit -t esnext -m esnext --incremental true --allowJs false --checkJs false --isolatedModules true --esModuleInterop true --resolveJsonModule true --strictNullChecks false --noUnusedLocals true --noUnusedParameters false --forceConsistentCasingInFileNames true file.ts",
    ] {
        supported(body, JobClass::Typecheck, Reason::BunScriptTypecheck);
    }
    let mut source = Memory::default();
    source.add("/project", &[("target", "tsc --noEmit")]);
    assert_eq!(
        source
            .resolve(&[
                "bun",
                "run",
                "target",
                "--",
                "true",
                "--strict",
                "false",
                "-p",
                "tsconfig.json",
            ])
            .classification
            .classification,
        JobClass::Typecheck
    );
}

#[test]
fn root_desktop_delegations_explicit_cwd_and_parent_package_execution_context() {
    let mut source = Memory::default();
    source.add(
        "/project",
        &[
            ("desktop:build", "bun run --cwd apps/desktop build"),
            ("desktop:typecheck", "bun run --cwd apps/desktop typecheck"),
        ],
    );
    source.add(
        "/project/apps/desktop",
        &[
            ("build", "vite build"),
            (
                "typecheck",
                "tsc --noEmit -p tsconfig.json && tsc --noEmit -p tsconfig.test.json",
            ),
        ],
    );
    for (script, class) in [
        ("desktop:build", JobClass::Build),
        ("desktop:typecheck", JobClass::Typecheck),
    ] {
        let result = source.resolve(&["bun", "run", script]);
        assert_eq!(result.classification.classification, class, "{result:?}");
        assert_eq!(result.packages.len(), 2);
        assert_eq!(result.graph_edges, 2);
    }
    for args in [
        vec!["bun", "run", "--cwd", "apps/desktop", "typecheck"],
        vec!["bun", "--cwd=/project/apps/desktop", "run", "typecheck"],
        vec!["bun", "run", "--cwd=apps/desktop", "typecheck"],
    ] {
        assert_eq!(
            source.resolve(&args).classification.classification,
            JobClass::Typecheck
        );
    }
    source.add(
        "/project/other",
        &[("entry", "bun run --cwd ../apps/desktop typecheck")],
    );
    // Discovery can find a parent package; the body resolves cwd from the
    // package owning the script, not the nested caller directory.
    let result = source.resolve(&["bun", "run", "--cwd", "other/deep", "entry"]);
    assert_eq!(result.classification.classification, JobClass::Typecheck);
}

#[test]
fn lifecycle_is_once_in_order_without_recursive_hooks_or_forwarded_flags() {
    let mut source = Memory::default();
    source.add(
        "/project",
        &[
            ("build", "vite build"),
            ("prebuild", "bun build pre.ts"),
            ("postbuild", "bun build post.ts"),
            ("preprebuild", "echo poison"),
        ],
    );
    let result = source.resolve(&["bun", "run", "build", "--mode", "production"]);
    assert_eq!(result.classification.classification, JobClass::Build);
    assert_eq!(result.graph_edges, 3);
    assert_eq!(result.analysis.invocations, 4);
    source.add(
        "/project",
        &[("build", "vite build"), ("prebuild", "biome check .")],
    );
    assert_eq!(
        source.resolve(&["bun", "run", "build"]).issue,
        Some(ScriptIssue::Mixed)
    );
    source.add(
        "/project",
        &[("build", "vite build"), ("postbuild", "unknown-tool")],
    );
    assert_eq!(
        source.resolve(&["bun", "run", "build"]).issue,
        Some(ScriptIssue::Unsupported)
    );
}

#[test]
fn forwarded_args_attach_only_at_body_end_with_exact_delimiter_and_nested_placement() {
    let mut source = Memory::default();
    source.add(
        "/project",
        &[
            ("outer", "bun build pre.ts && vite build"),
            ("nested", "bun run outer"),
            ("tc", "tsc --noEmit"),
        ],
    );
    // --mode is valid for the final Vite command, not the preceding Bun build.
    for args in [
        vec!["bun", "run", "outer", "--mode", "production"],
        vec!["bun", "run", "nested", "--", "--mode", "two words"],
        vec!["bun", "run", "--", "outer", "--mode=production"],
    ] {
        assert_eq!(
            source.resolve(&args).classification.classification,
            JobClass::Build
        );
    }
    assert_eq!(
        source
            .resolve(&["bun", "run", "tc", "--", "-p", "a b's$HOME.json"])
            .classification
            .classification,
        JobClass::Typecheck
    );
    assert_eq!(
        source
            .resolve(&["bun", "run", "outer", "--", "--", "--watch"])
            .classification
            .uncertainty,
        Uncertainty::Observe
    );
    // Cwd after the script is a tool argument, never a Bun lookup option.
    assert_eq!(
        source
            .resolve(&["bun", "run", "outer", "--cwd", "elsewhere"])
            .classification
            .classification,
        JobClass::Unknown
    );
    assert_eq!(
        source
            .resolve(&["bun", "--cwd", "/project", "run", "outer"])
            .classification
            .classification,
        JobClass::Unknown
    );
}

#[test]
fn malformed_dynamic_mixed_watch_hot_fanout_and_unsupported_graphs_bypass() {
    for body in [
        "vite --host 127.0.0.1",
        "vite build --watch",
        "bun build x.ts --watch",
        "bun test --watch",
        "tsc --noEmit -w",
        "tsc --noEmit --watch=true",
        "tsc --build --noEmit",
        "tsc --noEmit false",
        "tsc --noEmit=false",
        "tsc -p tsconfig.json",
        "biome check --watch",
        "biome check --mystery x",
        "bun run --workspaces build",
        "bun --hot run build",
        "bun run --filter=other build",
        "bun run --parallel a b",
        "bun run no-such-script",
        "bun run ./build.ts",
        "bun run '*.ts'",
        "vite build && bun test",
        "vite build && ssh remote.example build",
        "bun test;",
        "(vite build",
        "echo should-not-run",
        "sh -c 'vite build'",
        "VAR=value vite build",
        "bun test > log",
        "bun test $(touch poison)",
        "bun test $ARGS",
        "bun test `touch poison`",
        "bun test &&",
    ] {
        let mut metadata = Memory::default();
        metadata.add("/project", &[("test", body)]);
        let result = metadata.resolve(&["bun", "run", "test"]);
        // A trailing separator is permitted by the maintained grammar.
        if body == "bun test;" {
            assert_eq!(result.classification.classification, JobClass::Test);
            continue;
        }
        assert_eq!(
            result.classification.classification,
            JobClass::Unknown,
            "{body}: {result:?}"
        );
        assert_eq!(result.classification.uncertainty, Uncertainty::Observe);
        assert!(result.issue.is_some());
    }
}

#[test]
fn remote_only_graphs_remain_remote_and_never_become_local_script_reservations() {
    for body in [
        "gh workflow run ci.yml",
        "gh run view && gh run watch",
        "ssh remote.example 'bun run build'",
        "ssh one.example build && ssh two.example test",
    ] {
        let mut source = Memory::default();
        source.add("/project", &[("build", body)]);
        let result = source.resolve(&["bun", "run", "build"]);
        assert_eq!(result.classification.locality, Locality::Remote, "{body}");
        assert!(!result.classification.is_bun_script());
        assert_eq!(result.issue, None);
    }
}

#[test]
fn no_metadata_or_native_non_utf8_inputs_do_not_guess_a_script() {
    let result = Memory::default().resolve(&["bun", "run", "test"]);
    assert_eq!(
        result.issue,
        Some(ScriptIssue::Metadata(MetadataError::Missing))
    );
    use std::os::unix::ffi::OsStringExt;
    let result = analyze_script_argv(
        &["bun".into(), "run".into(), OsString::from_vec(vec![255])],
        Path::new("/project"),
        &mut Memory::default(),
    );
    assert_eq!(result.classification.reason, Reason::UnsupportedSyntax);
    for args in [
        vec!["cargo", "build"],
        vec!["bun", "test"],
        vec!["printf", "literal"],
    ] {
        let native: Vec<_> = args.into_iter().map(OsString::from).collect();
        assert_eq!(
            analyze_script_argv(&native, Path::new("/project"), &mut Memory::default())
                .classification,
            classify_argv(&native)
        );
    }
}
