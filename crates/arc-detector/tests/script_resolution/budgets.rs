//! Cycle identity and shared traversal budgets use the same pure fixture.
use super::*;

#[test]
fn cycle_identity_ignores_forwarded_args_and_normalizes_package_aliases() {
    for scripts in [
        vec![("x", "bun run x another-argument")],
        vec![("x", "bun run --cwd ./other/.. y"), ("y", "bun run x")],
        vec![("x", "bun test"), ("prex", "bun run x")],
    ] {
        let mut source = Memory::default();
        source.add("/project", &scripts);
        let result = source.resolve(&["bun", "run", "x"]);
        assert_eq!(result.issue, Some(ScriptIssue::Cycle), "{result:?}");
    }
}

#[test]
fn parsing_budgets_are_shared_across_native_args_all_bodies_and_lifecycle_edges() {
    supported(
        &"bun test;".repeat(MAX_INVOCATIONS - 1),
        JobClass::Test,
        Reason::BunScriptTest,
    );
    supported(
        &format!("tsc --noEmit {}", "x ".repeat(MAX_TOKENS - 5)),
        JobClass::Typecheck,
        Reason::BunScriptTypecheck,
    );
    let bodies = [
        ("x".repeat(MAX_INPUT_BYTES + 1), Reason::InputLimit),
        (
            format!("bun test {}", "x ".repeat(MAX_TOKENS)),
            Reason::TokenLimit,
        ),
        ("bun test;".repeat(MAX_INVOCATIONS), Reason::InvocationLimit),
        (format!("bun test {}", "x".repeat(12000)), Reason::WorkLimit),
    ];
    for (body, reason) in bodies {
        let mut source = Memory::default();
        source.add("/project", &[("x", &body)]);
        let result = source.resolve(&["bun", "run", "x"]);
        assert_eq!(result.classification.reason, reason, "{result:?}");
        assert!(result.analysis.tokens <= MAX_TOKENS);
        assert!(result.analysis.invocations <= MAX_INVOCATIONS);
        assert!(result.analysis.work <= MAX_WORK);
    }
    let mut source = Memory::default();
    let boundary: Vec<_> = (0..=MAX_DEPTH)
        .map(|n| {
            (
                format!("n{n}"),
                if n == MAX_DEPTH {
                    "bun test".into()
                } else {
                    format!("bun run n{}", n + 1)
                },
            )
        })
        .collect();
    source.add(
        "/project",
        &boundary
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_str()))
            .collect::<Vec<_>>(),
    );
    let result = source.resolve(&["bun", "run", "n0"]);
    assert_eq!(result.classification.classification, JobClass::Test);
    assert_eq!(result.analysis.max_depth, MAX_DEPTH);
    let mut source = Memory::default();
    let scripts: Vec<_> = (0..=MAX_DEPTH + 1)
        .map(|n| (format!("n{n}"), format!("bun run n{}", n + 1)))
        .collect();
    source.add(
        "/project",
        &scripts
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_str()))
            .collect::<Vec<_>>(),
    );
    let result = source.resolve(&["bun", "run", "n0"]);
    assert_eq!(result.classification.reason, Reason::NestingLimit);
    assert_eq!(result.analysis.max_depth, MAX_DEPTH);

    let mut source = Memory::default();
    source.add("/project", &[("x", "bun test")]);
    let result = source.resolve(&["bun", "run", "x", &"x".repeat(MAX_INPUT_BYTES)]);
    assert_eq!(result.classification.reason, Reason::InputLimit);
}
