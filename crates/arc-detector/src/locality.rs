//! Transient, bounded endpoint evidence. Classification performs no I/O.
use crate::{
    budget::{Budget, Checked},
    parser::Parser,
    Classification, ClassifyRequest, Reason, MAX_INPUT_BYTES, MAX_TOKENS,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
};

mod pnpm;

pub const MAX_CONTEXT_AGE_MS: u64 = 5_000;
pub const MAX_CONTEXT_BYTES: usize = 8_192;
pub const MAX_CONTEXT_VALUE_BYTES: usize = 1_024;
pub const MAX_CONTEXT_ENTRIES: usize = 32;

/// Only endpoint selectors are copied. Values remain transient and may be sensitive.
pub const CONTEXT_ENV_KEYS: &[&str] = &[
    "GH_HOST",
    "GH_REPO",
    "GITLAB_HOST",
    "GITLAB_URL",
    "GLAB_HOST",
    "DOCKER_HOST",
    "DOCKER_CONTEXT",
    "DOCKER_CONFIG",
    "DOCKER_BUILDKIT",
    "BUILDX_BUILDER",
    "KUBECONFIG",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_CONFIG",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_NOSYSTEM",
    "GIT_CONFIG_COUNT",
    "GIT_SSH",
    "GIT_SSH_COMMAND",
];

/// Extract endpoint-capture invocations using the detector's bounded grammar.
/// Literal `cd` builtins are omitted only when every remaining invocation is
/// provably local, so caller-directory endpoint facts are never reused after
/// a directory change. Rejected parses do not trigger probes or evaluation.
pub fn capture_shell_argv(request: &ClassifyRequest) -> Option<Vec<Vec<OsString>>> {
    fn collect(
        source: &str,
        depth: usize,
        budget: &mut Budget,
        output: &mut Vec<Vec<OsString>>,
        changed_directory: &mut bool,
    ) -> Checked<()> {
        let mut commands = Vec::new();
        Parser::new(source).commands(depth, budget, &mut commands)?;
        for command in commands {
            let words = command.words;
            if command.first_plain
                && words.len() == 2
                && words[0] == "cd"
                && !words[1].is_empty()
                && !words[1].starts_with('-')
            {
                *changed_directory = true;
                continue;
            }
            if matches!(words.first().map(String::as_str), Some("sh"))
                && words.len() == 3
                && matches!(words[1].as_str(), "-c" | "-lc")
            {
                collect(
                    &words[2],
                    command.depth + 1,
                    budget,
                    output,
                    changed_directory,
                )?;
            } else {
                output.push(words.into_iter().map(OsString::from).collect());
            }
        }
        Ok(())
    }
    if request.command.len() > MAX_INPUT_BYTES
        || !matches!(request.shell_kind.as_str(), "sh" | "bash" | "zsh")
    {
        return None;
    }
    let mut output = Vec::new();
    let mut changed_directory = false;
    collect(
        &request.command,
        0,
        &mut Budget::default(),
        &mut output,
        &mut changed_directory,
    )
    .ok()?;
    if changed_directory {
        let empty_context = EndpointContext {
            captured_at_ms: 1,
            ..EndpointContext::default()
        };
        if output.is_empty()
            || output.iter().any(|argv| {
                classify_argv_with_context(argv, &empty_context, 1).locality
                    != crate::Locality::HostLocal
            })
        {
            return None;
        }
    }
    Some(output)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EndpointKind {
    CiHost,
    GitRemote,
    /// Effective destination of a bare push; selector is always `@bare-push`.
    GitPushDefault,
    SshDestination,
    DockerContext,
    DockerBuilder,
    KubernetesContext,
    KubernetesPod,
    /// Complete selected Makefile contents for the literal `make test` case.
    /// The caller must establish that this file is the one Make will read,
    /// including Makefile precedence and environment/options that can change
    /// the selected files, and revalidate it at the execution boundary.
    Makefile,
    /// A bounded, revalidated pnpm script matching the selected finite case.
    PnpmScript,
    /// The caller verified the selected non-login shell's startup is inert.
    ShellStartup,
    /// The selected act invocation and its explicitly selected Docker endpoint.
    ActLocal,
}

/// A selector is bound to data from a fresh, bounded caller-side snapshot.
/// Except for `Makefile`, endpoints are concrete endpoints, not assertions.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointFact {
    pub kind: EndpointKind,
    pub selector: String,
    pub endpoints: Vec<String>,
}

/// Never persist or log these raw selectors and endpoints.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointContext {
    pub captured_at_ms: u64,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub facts: Vec<EndpointFact>,
}

impl EndpointContext {
    /// The supplied clock permits deterministic tests. Future and stale facts
    /// are never used, even when only one field would affect an invocation.
    pub fn is_valid_at(&self, now_ms: u64) -> bool {
        if self.captured_at_ms == 0
            || now_ms
                .checked_sub(self.captured_at_ms)
                .is_none_or(|age| age > MAX_CONTEXT_AGE_MS)
            || self.environment.len() + self.facts.len() > MAX_CONTEXT_ENTRIES
        {
            return false;
        }
        let mut bytes = 0usize;
        let mut entries = self.environment.len() + self.facts.len();
        let mut names = BTreeSet::new();
        let mut add = |s: &str, allow_empty: bool| {
            if (!allow_empty && s.is_empty())
                || s.len() > MAX_CONTEXT_VALUE_BYTES
                || s.contains('\0')
            {
                return false;
            }
            bytes = match bytes.checked_add(s.len()) {
                Some(n) if n <= MAX_CONTEXT_BYTES => n,
                _ => return false,
            };
            true
        };
        for (key, value) in &self.environment {
            if !CONTEXT_ENV_KEYS.contains(&key.as_str()) || !add(key, false) || !add(value, true) {
                return false;
            }
        }
        for fact in &self.facts {
            entries += fact.endpoints.len();
            if entries > MAX_CONTEXT_ENTRIES
                || fact.endpoints.is_empty()
                || !add(&fact.selector, false)
                || !names.insert((fact.kind, fact.selector.as_str()))
                || !fact.endpoints.iter().all(|endpoint| add(endpoint, false))
            {
                return false;
            }
        }
        true
    }
}

/// Returns None only for an unowned program. Some(UNKNOWN) is terminal.
pub(crate) fn recognize(
    words: &[String],
    context: &EndpointContext,
    budget: &mut Budget,
) -> Checked<Option<Classification>> {
    if words.first().is_some_and(|word| word == "make") {
        return recognize_makefile(words, context, budget).map(Some);
    }
    if words.first().is_some_and(|word| word == "pnpm") {
        return pnpm::recognize_pnpm(words, context, budget).map(Some);
    }
    if let Some(result) = crate::remote_ci::recognize(words, context, budget)? {
        return Ok(Some(result));
    }
    crate::remote_endpoints::recognize(words, context, budget)
}

// This deliberately recognizes just the SRS fixture, not Make's language.
// The fact contains the entire selected file, not merely a matching recipe;
// dependencies, includes, variables and additional recipes remain unknown.
fn recognize_makefile(
    words: &[String],
    context: &EndpointContext,
    budget: &mut Budget,
) -> Checked<Classification> {
    if words.len() == 2 && budget.eq(&words[1], "test")? {
        for fact in &context.facts {
            budget.charge(1)?;
            if fact.kind == EndpointKind::Makefile && budget.eq(&fact.selector, "test")? {
                if let [contents] = fact.endpoints.as_slice() {
                    if budget.eq(contents, "test:\n\tcargo test\n")? {
                        return Ok(Classification::new(
                            crate::JobClass::Test,
                            Reason::CargoTest,
                        ));
                    }
                }
                break;
            }
        }
    }
    Ok(Classification::unknown(Reason::UnresolvedScript))
}

/// Contextual shell recognition extends the frozen protocol-1 grammar only
/// for literal local commands. Assignment values may change endpoint facts,
/// so an assignment is never used to justify a remote or mixed result.
pub(crate) fn recognize_shell(
    words: &[String],
    assignments: usize,
    context: &EndpointContext,
    budget: &mut Budget,
) -> Checked<Classification> {
    if assignments > 0 {
        return Ok(local_leaf(&words[assignments..], budget)?
            .unwrap_or_else(|| Classification::unknown(Reason::UnresolvedCommand)));
    }
    if let Some(result) = recognize(words, context, budget)? {
        return Ok(result);
    }
    if let Some(result) = local_leaf(words, budget)? {
        return Ok(result);
    }
    crate::rules::recognize(words, budget)
}

pub(crate) fn safe_shell_startup(context: &EndpointContext, shell: &str) -> bool {
    context.facts.iter().any(|fact| {
        fact.kind == EndpointKind::ShellStartup
            && fact.selector == shell
            && fact.endpoints.as_slice() == ["safe-nonlogin-v1"]
    })
}

// Unwrap only finite literal forms that lead to a local workload. In
// particular, wrappers and environment assignments never lend endpoint facts
// to a remote command. Executable lookup and shell evaluation are not used.
fn local_leaf(words: &[String], budget: &mut Budget) -> Checked<Option<Classification>> {
    let mut words = words;
    for _ in 0..crate::MAX_DEPTH {
        let Some(head) = words.first() else {
            return Ok(None);
        };
        budget.charge(1 + head.len())?;
        if matches!(head.as_str(), "env" | "nice") && words.len() > 1 {
            words = &words[1..];
        } else if head == "timeout" && words.len() > 2 {
            budget.charge(1 + words[1].len())?;
            if words[1].is_empty() || !words[1].bytes().all(|ch| ch.is_ascii_digit()) {
                return Ok(None);
            }
            words = &words[2..];
        } else {
            break;
        }
    }
    let Some(head) = words.first() else {
        return Ok(None);
    };
    budget.charge(1 + head.len())?;
    if head == "bun" && words.get(1).is_some_and(|word| word == "test") {
        return crate::rules::recognize_bun_test(&words[2..], budget).map(Some);
    }
    if head == "cargo" {
        let result = crate::rules::recognize(words, budget)?;
        return Ok((result.locality == crate::Locality::HostLocal).then_some(result));
    }
    if !head.ends_with('/') && head.contains('/') && head.rsplit('/').next() == Some("cargo") {
        budget.charge(words.iter().map(|word| 1 + word.len()).sum())?;
        let mut literal = words.to_vec();
        literal[0] = "cargo".into();
        let result = crate::rules::recognize(&literal, budget)?;
        return Ok((result.locality == crate::Locality::HostLocal).then_some(result));
    }
    Ok(None)
}

pub fn classify_with_context(
    request: &ClassifyRequest,
    context: &EndpointContext,
    now_ms: u64,
) -> Classification {
    analyze_with_context(request, context, now_ms).0
}

pub fn analyze_with_context(
    request: &ClassifyRequest,
    context: &EndpointContext,
    now_ms: u64,
) -> (Classification, crate::AnalysisStats) {
    let mut budget = Budget::default();
    let result = (|| {
        if !context.is_valid_at(now_ms) {
            return Err(Reason::EndpointAmbiguous);
        }
        if request.command.len() > MAX_INPUT_BYTES {
            return Err(Reason::InputLimit);
        }
        if !budget.one_of(&request.shell_kind, &["sh", "bash", "zsh"])? {
            return Err(Reason::UnsupportedShell);
        }
        let mut leaves = Vec::new();
        Parser::with_context(&request.command, context).parse(0, &mut budget, &mut leaves)?;
        crate::aggregate(&leaves, &mut budget)
    })();
    (result.unwrap_or_else(Classification::unknown), budget)
}

pub fn classify_argv_with_context(
    argv: &[OsString],
    context: &EndpointContext,
    now_ms: u64,
) -> Classification {
    analyze_argv_with_context(argv, context, now_ms).0
}

pub fn analyze_argv_with_context(
    argv: &[OsString],
    context: &EndpointContext,
    now_ms: u64,
) -> (Classification, crate::AnalysisStats) {
    let mut budget = Budget::default();
    let result = (|| {
        if !context.is_valid_at(now_ms) {
            return Err(Reason::EndpointAmbiguous);
        }
        if argv.len() > MAX_TOKENS {
            return Err(Reason::TokenLimit);
        }
        let mut bytes = 0usize;
        let mut words = Vec::with_capacity(argv.len());
        for arg in argv {
            bytes = bytes
                .checked_add(arg.as_encoded_bytes().len())
                .ok_or(Reason::InputLimit)?;
            if bytes > MAX_INPUT_BYTES {
                return Err(Reason::InputLimit);
            }
            budget.token()?;
            budget.charge(1 + arg.as_encoded_bytes().len())?;
            let word = arg
                .to_str()
                .filter(|word| !word.contains('\0'))
                .ok_or(Reason::UnsupportedSyntax)?;
            words.push(word.to_owned());
        }
        if words.first().is_none_or(String::is_empty) {
            return Err(Reason::EmptyCommand);
        }
        budget.invocation()?;
        if words.len() == 3
            && words[1] == "-c"
            && (words[0] == "sh"
                || matches!(words[0].as_str(), "bash" | "zsh")
                    && safe_shell_startup(context, &words[0]))
        {
            budget.depth(1)?;
            let mut leaves = Vec::new();
            Parser::with_context(&words[2], context).parse(1, &mut budget, &mut leaves)?;
            return crate::aggregate(&leaves, &mut budget);
        }
        if let Some(result) = recognize(&words, context, &mut budget)? {
            return Ok(result);
        }
        if let Some(result) = local_leaf(&words, &mut budget)? {
            return Ok(result);
        }
        // Maintain the original native recognizers and budget for unowned programs.
        if let Some(result) = crate::native::recognize(&words, &mut budget)? {
            return Ok(result);
        }
        if let Some(result) = crate::python::recognize(&words, &mut budget)? {
            return Ok(result);
        }
        if Budget::eq(&mut budget, &words[0], "bun")?
            && match words.get(1) {
                Some(word) => Budget::eq(&mut budget, word, "test")?,
                None => false,
            }
        {
            crate::rules::recognize_bun_test(&words[2..], &mut budget)
        } else {
            crate::rules::recognize(&words, &mut budget)
        }
    })();
    (result.unwrap_or_else(Classification::unknown), budget)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod capture_contract_tests {
    use super::*;

    #[test]
    fn shell_capture_uses_the_bounded_literal_grammar_including_nested_shells() {
        let request = ClassifyRequest {
            command: "gh workflow run 'build.yml' && sh -c 'docker build .'".into(),
            shell_kind: "sh".into(),
        };
        assert_eq!(
            capture_shell_argv(&request).unwrap(),
            vec![
                vec![
                    OsString::from("gh"),
                    OsString::from("workflow"),
                    OsString::from("run"),
                    OsString::from("build.yml")
                ],
                vec![
                    OsString::from("docker"),
                    OsString::from("build"),
                    OsString::from(".")
                ],
            ]
        );
        for command in [
            "gh workflow run $(touch marker)",
            "if true; then gh run list; fi",
        ] {
            assert!(capture_shell_argv(&ClassifyRequest {
                command: command.into(),
                shell_kind: "sh".into()
            })
            .is_none());
        }
        assert!(capture_shell_argv(&ClassifyRequest {
            command: "gh run list".into(),
            shell_kind: "fish".into()
        })
        .is_none());
    }
}

#[cfg(test)]
mod shell_closure_tests;
