//! Contextual analysis over a transient metadata port. No I/O or execution.
use super::*;
use crate::{budget::Budget, parser::Parser, JobClass, Locality, Reason, MAX_INPUT_BYTES};
use std::ffi::OsString;

type Result<T> = std::result::Result<T, (Reason, ScriptIssue)>;
fn syntax(reason: Reason) -> (Reason, ScriptIssue) {
    (
        reason,
        if reason == Reason::UnresolvedExpansion {
            ScriptIssue::Dynamic
        } else {
            ScriptIssue::Unsupported
        },
    )
}
fn unsupported() -> (Reason, ScriptIssue) {
    (Reason::UnresolvedScript, ScriptIssue::Unsupported)
}

/// Analyze an original native Bun invocation. Non-script commands retain the
/// syntax-only API's classification. Counters and snapshots are per invocation.
pub fn analyze_script_argv(
    argv: &[OsString],
    cwd: &Path,
    metadata: &mut impl PackageMetadata,
) -> ScriptResolution {
    let mut graph = Graph {
        source: metadata,
        result: ScriptResolution {
            classification: Classification::unknown(Reason::UnresolvedScript),
            analysis: Budget::default(),
            metadata: MetadataStats::default(),
            graph_edges: 0,
            packages: Vec::new(),
            issue: None,
        },
        active: Vec::new(),
        leaves: Vec::new(),
    };
    let result = graph.native(argv, cwd);
    match result {
        Ok(classification) => graph.result.classification = classification,
        Err((reason, issue)) => {
            graph.result.classification = Classification::unknown(reason);
            graph.result.issue = Some(issue);
        }
    }
    graph.result
}

struct Graph<'a, M> {
    source: &'a mut M,
    result: ScriptResolution,
    active: Vec<(PathBuf, String)>,
    leaves: Vec<Classification>,
}
impl<M: PackageMetadata> Graph<'_, M> {
    fn native(&mut self, argv: &[OsString], cwd: &Path) -> Result<Classification> {
        let b = &mut self.result.analysis;
        let mut words = Vec::new();
        let mut bytes = 0usize;
        for arg in argv {
            bytes = bytes
                .checked_add(arg.as_encoded_bytes().len())
                .filter(|n| *n <= MAX_INPUT_BYTES)
                .ok_or_else(|| syntax(Reason::InputLimit))?;
            b.token().map_err(syntax)?;
            b.charge(1 + arg.as_encoded_bytes().len()).map_err(syntax)?;
            let word = arg
                .to_str()
                .filter(|v| !v.contains('\0'))
                .ok_or_else(|| syntax(Reason::UnsupportedSyntax))?;
            words.push(word.to_owned());
        }
        if !super::commands::is_bun(words.first().map(String::as_str).unwrap_or("")) {
            return Ok(crate::classify_argv(argv));
        }
        let Some(call) = super::commands::script_call(&words, cwd, b).map_err(syntax)? else {
            return Ok(crate::classify_argv(argv));
        };
        b.invocation().map_err(syntax)?;
        self.script(&call.cwd, &call.name, &call.args, 0)?;
        let class = crate::aggregate(&self.leaves, &mut self.result.analysis).map_err(syntax)?;
        if class.locality == Locality::Remote {
            return Ok(class);
        }
        let reason = match class.classification {
            JobClass::Build => Reason::BunScriptBuild,
            JobClass::Test => Reason::BunScriptTest,
            JobClass::Lint => Reason::BunScriptLint,
            JobClass::Typecheck => Reason::BunScriptTypecheck,
            _ => return Err((Reason::UnresolvedCompound, ScriptIssue::Mixed)),
        };
        Ok(Classification::new(class.classification, reason))
    }
    fn edge(&mut self, depth: usize) -> Result<()> {
        self.result.analysis.depth(depth).map_err(syntax)?;
        if self.result.graph_edges >= MAX_GRAPH_EDGES {
            return Err((Reason::InvocationLimit, ScriptIssue::GraphLimit));
        }
        self.result.analysis.charge(1).map_err(syntax)?;
        self.result.graph_edges += 1;
        Ok(())
    }
    fn script(&mut self, cwd: &Path, name: &str, args: &[String], depth: usize) -> Result<()> {
        self.edge(depth)?;
        let package = self
            .source
            .package(cwd, &mut self.result.metadata)
            .map_err(|e| (Reason::UnresolvedScript, ScriptIssue::Metadata(e)))?;
        let identity = (package.package_dir.clone(), name.to_owned());
        if self.active.contains(&identity) {
            return Err((Reason::UnresolvedScript, ScriptIssue::Cycle));
        }
        if !self
            .result
            .packages
            .iter()
            .any(|p| p.package_dir == package.package_dir)
        {
            self.result.packages.push(package.clone());
        }
        let body = package.scripts.get(name).ok_or_else(unsupported)?;
        self.active.push(identity);
        // Lifecycle bodies are not recursively wrapped in their own pre/post
        // hooks. Only an explicit nested `bun run` starts another lifecycle trio.
        if let Some(pre) = package.scripts.get(&format!("pre{name}")) {
            self.edge(depth + 1)?;
            self.body(pre, &[], &package.package_dir, depth + 1)?;
        }
        self.body(body, args, &package.package_dir, depth)?;
        if let Some(post) = package.scripts.get(&format!("post{name}")) {
            self.edge(depth + 1)?;
            self.body(post, &[], &package.package_dir, depth + 1)?;
        }
        self.active.pop();
        Ok(())
    }
    fn body(&mut self, body: &str, args: &[String], cwd: &Path, depth: usize) -> Result<()> {
        // Bun appends shell-escaped passthrough arguments to the script body,
        // not to every leaf of a compound. Parse precisely that placement.
        let mut length = body.len();
        for arg in args {
            self.result.analysis.charge(1 + arg.len()).map_err(syntax)?;
            let quotes = arg.bytes().filter(|c| *c == b'\'').count();
            length = length
                .checked_add(3 + arg.len() + 3 * quotes)
                .ok_or_else(|| syntax(Reason::InputLimit))?;
        }
        if length > MAX_INPUT_BYTES {
            return Err(syntax(Reason::InputLimit));
        }
        let mut command = String::with_capacity(length);
        command.push_str(body);
        for arg in args {
            command.push_str(" '");
            for c in arg.chars() {
                if c == '\'' {
                    command.push_str("'\\''");
                } else {
                    command.push(c);
                }
            }
            command.push('\'');
        }
        let mut invocations = Vec::new();
        Parser::new(&command)
            .commands(depth, &mut self.result.analysis, &mut invocations)
            .map_err(syntax)?;
        if invocations.is_empty() {
            return Err(unsupported());
        }
        for invocation in invocations {
            if let Some(call) =
                super::commands::script_call(&invocation.words, cwd, &mut self.result.analysis)
                    .map_err(syntax)?
            {
                self.script(&call.cwd, &call.name, &call.args, invocation.depth + 1)?;
            } else {
                let leaf = super::commands::leaf(&invocation.words, &mut self.result.analysis)
                    .map_err(syntax)?;
                if leaf.classification == JobClass::Unknown {
                    return Err(unsupported());
                }
                self.result.analysis.charge(1).map_err(syntax)?;
                self.leaves.push(leaf);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoMetadata;
    impl PackageMetadata for NoMetadata {
        fn package(
            &mut self,
            _: &Path,
            _: &mut MetadataStats,
        ) -> std::result::Result<PackageSnapshot, MetadataError> {
            panic!("No I/O at a rejected graph edge")
        }
        fn revalidate(
            &mut self,
            _: &[PackageSnapshot],
            _: &mut MetadataStats,
        ) -> std::result::Result<(), MetadataError> {
            unreachable!()
        }
    }
    #[test]
    fn graph_edge_limit_is_checked_before_traversal_without_committing_an_overrun() {
        // The invocation bound usually wins first for leaf-bearing graphs;
        // independently retain the frozen graph-edge guard for all traversals.
        let mut graph = Graph {
            source: &mut NoMetadata,
            result: ScriptResolution {
                classification: Classification::unknown(Reason::UnresolvedScript),
                analysis: Budget::default(),
                metadata: MetadataStats::default(),
                graph_edges: 0,
                packages: vec![],
                issue: None,
            },
            active: vec![],
            leaves: vec![],
        };
        for _ in 0..MAX_GRAPH_EDGES {
            graph.edge(0).unwrap();
        }
        let before = graph.result.analysis;
        assert_eq!(
            graph.edge(0),
            Err((Reason::InvocationLimit, ScriptIssue::GraphLimit))
        );
        assert_eq!(graph.result.graph_edges, MAX_GRAPH_EDGES);
        assert_eq!(graph.result.analysis, before);
    }
}
