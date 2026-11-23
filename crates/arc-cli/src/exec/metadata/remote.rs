//! Independent proof of a workspace fan-out's remote members (lane C).
//!
//! The caller's endpoint facts describe the directory the fan-out was run
//! from, so the resolver never lends them to a member. A member whose script
//! is one GitHub CLI invocation with an explicit repository (`--repo X`,
//! `--repo=X` or `-R X`, anywhere among its options) selects its endpoint
//! independently of the member's Git repository: only the environment and
//! gh's own configuration decide it. That leaf is captured twice, both
//! captures must agree and be valid, and its contextual classification must
//! equal the member's own. Every other member must be local compute. Only
//! then is the fan-out MIXED, and each member keeps its real locality.
use arc_detector::{
    scripts::{cache, ScriptIssue, ScriptResolution},
    Locality,
};
use std::{ffi::OsString, path::Path};

/// Whether `result`'s members are local compute and independently proved
/// remote GitHub CLI invocations, with at least one of each.
pub(super) fn prove_mixed_members(
    result: &ScriptResolution,
    words: &[String],
    root: &Path,
) -> bool {
    if result.issue != Some(ScriptIssue::Mixed) || result.members.is_empty() {
        return false;
    }
    // Passthrough arguments would extend the member's leaf.
    let Some(syntax) = cache::fan_out_syntax(words).filter(|s| !s.has_args) else {
        return false;
    };
    if std::env::vars_os().any(|(name, _)| {
        let name = name.to_string_lossy();
        matches!(name.as_ref(), "BASH_ENV" | "ENV")
            || name.starts_with("BASH_FUNC_")
            || name.starts_with("__BASH_FUNC<")
    }) {
        return false;
    }
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let mut local = false;
    let mut remote = false;
    for member in &result.members {
        if member.classification.locality.is_local_compute() {
            local = true;
            continue;
        }
        if member.classification.locality != Locality::Remote
            || member.script != syntax.script
            || !proved(result, &root, member)
        {
            return false;
        }
        remote = true;
    }
    local && remote
}

fn proved(
    result: &ScriptResolution,
    root: &Path,
    member: &arc_detector::scripts::MemberClassification,
) -> bool {
    let dir = if member.member == Path::new(".") {
        root.to_owned()
    } else {
        root.join(&member.member)
    };
    let Some(package) = result.packages.iter().find(|p| p.package_dir == dir) else {
        return false;
    };
    // A lifecycle hook would add leaves outside the single proved one.
    if package
        .scripts
        .contains_key(&format!("pre{}", member.script))
        || package
            .scripts
            .contains_key(&format!("post{}", member.script))
    {
        return false;
    }
    let Some(body) = package.scripts.get(&member.script) else {
        return false;
    };
    let request = arc_detector::ClassifyRequest {
        command: body.clone(),
        shell_kind: "sh".into(),
    };
    let Some(mut leaves) =
        arc_detector::capture_shell_argv(&request).filter(|leaves| leaves.len() == 1)
    else {
        return false;
    };
    let leaf = leaves.pop().unwrap_or_default();
    if !explicit_repository(&leaf) {
        return false;
    }
    let Some(first) = crate::exec::capture_argv(&leaf) else {
        return false;
    };
    let Some(second) = crate::exec::capture_argv(&leaf) else {
        return false;
    };
    let Some(now) = arc_ipc::locality::now_ms() else {
        return false;
    };
    let (Ok(first_facts), Ok(second_facts)) = (
        serde_json::to_value(&first.facts),
        serde_json::to_value(&second.facts),
    ) else {
        return false;
    };
    first.is_valid_at(now)
        && second.is_valid_at(now)
        && first.environment == second.environment
        && first_facts == second_facts
        && arc_detector::classify_argv_with_context(&leaf, &second, now) == member.classification
}

/// A bare `gh` invocation that names exactly one repository explicitly, so
/// its endpoint does not depend on the directory's Git remotes.
pub(super) fn explicit_repository(argv: &[OsString]) -> bool {
    let Some(words) = argv
        .iter()
        .map(|word| word.to_str())
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    if words.first() != Some(&"gh") || words.len() < 3 {
        return false;
    }
    let mut repositories = 0;
    let mut at = 3;
    while let Some(word) = words.get(at) {
        if matches!(*word, "--repo" | "-R") {
            at += 1;
            match words.get(at) {
                Some(value) if !value.is_empty() && !value.starts_with('-') => repositories += 1,
                _ => return false,
            }
        } else if let Some(value) = word.strip_prefix("--repo=") {
            if value.is_empty() {
                return false;
            }
            repositories += 1;
        }
        at += 1;
    }
    repositories == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_explicit_repository_makes_a_gh_leaf_directory_independent() {
        let argv = |s: &str| s.split(' ').map(OsString::from).collect::<Vec<_>>();
        for proved in [
            "gh workflow run ci.yml --repo github.com/example/project",
            "gh workflow run --repo=github.com/example/project ci.yml",
            "gh workflow run ci.yml --ref main -R example/project",
            "gh run list --repo=example/project",
        ] {
            assert!(explicit_repository(&argv(proved)), "{proved}");
        }
        for unproved in [
            "gh workflow run ci.yml",
            "gh workflow run ci.yml --repo",
            "gh workflow run ci.yml --repo=",
            "gh workflow run ci.yml --repo --ref",
            "gh workflow run ci.yml -R a/b --repo=a/b",
            "/usr/bin/gh workflow run ci.yml --repo a/b",
            "git push --repo a/b",
        ] {
            assert!(!explicit_repository(&argv(unproved)), "{unproved}");
        }
    }
}
