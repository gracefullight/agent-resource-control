//! Workspace fan-out (lane C, DET-009 item 2, SRS 8.4).
//!
//! [`parse`] recognizes a closed grammar: `bun [run] --filter <p>`, `npm run
//! <s> --workspaces` and `-w <x>`, `pnpm -r` and `--filter <p>` with `run`
//! or a known script shorthand,
//! and `yarn workspaces foreach -A|-W [--include|--exclude <p>] run <s>`.
//! [`Graph::workspace`] lists the members the root manifest's globs name
//! (`PackageMetadata::workspace_members`), selects them, resolves the named
//! script in each with `Graph::script` (every leaf through the existing rules)
//! and pushes one `MemberClassification` per member that runs it. The
//! resolver then aggregates every leaf into the existing compound or MIXED
//! projection. A dynamic, unresolvable or over-limit graph is an error, so
//! the whole result is UNKNOWN/OBSERVE. Classification executes nothing and
//! reads only the declared manifests.
//!
//! Root participation follows the selection. A Bun name filter that matches
//! the root's name includes it, which stays unresolved when the root defines
//! the script; a filter that does not match excludes it. npm includes the
//! root only with `include-workspace-root`: the CLI resolves that setting
//! from the available npm configuration into an explicit flag, and then the
//! root's script is classified as member `.`; without that evidence a root
//! script is unresolved. For pnpm and Yarn a root script is unresolved unless
//! only member paths are selected. pnpm resolves a script name that is not a
//! built-in command as script shorthand, which every selected member must
//! define. pnpm and Yarn do not run `pre`/`post` hooks, so a member that has
//! one is unresolved; the invocation runs at the workspace root; a nested
//! fan-out must use the outermost runner.
use super::{unsupported, Graph, Result};
use crate::{
    budget::{Budget, Checked},
    scripts::{MemberClassification, PackageMetadata, ScriptIssue, WorkspaceMember},
    Reason,
};
use std::path::Path;

#[path = "workspace/grammar.rs"]
mod grammar;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Runner {
    Bun,
    Npm,
    Pnpm,
    Yarn,
}
impl Runner {
    pub(crate) fn of(program: &str) -> Option<Self> {
        match Path::new(program).file_name()?.to_str()? {
            "bun" => Some(Self::Bun),
            "npm" => Some(Self::Npm),
            "pnpm" => Some(Self::Pnpm),
            "yarn" => Some(Self::Yarn),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Selector {
    /// A member name; `*` matches any sequence.
    Name(String),
    /// A path relative to the root; `*` matches within one segment.
    Path(String),
    /// npm `-w`: an exact name or an exact relative directory.
    Exact(String),
}

/// One parsed fan-out invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FanOut {
    pub(crate) runner: Runner,
    all: bool,
    include: Vec<Selector>,
    exclude: Vec<Selector>,
    script: String,
    args: Vec<String>,
    /// npm without `--if-present` fails in a member that lacks the script.
    require_script: bool,
    /// npm's explicit `include-workspace-root`; `None` when not on the
    /// command line, so configuration decides.
    include_root: Option<bool>,
}
impl FanOut {
    pub(crate) fn script(&self) -> &str {
        &self.script
    }
    pub(crate) fn has_args(&self) -> bool {
        !self.args.is_empty()
    }
    pub(crate) fn include_root(&self) -> Option<bool> {
        self.include_root
    }
}

/// Whether `program` is a workspace runner other than `bun`.
pub(crate) fn is_runner(program: &str) -> bool {
    matches!(
        Runner::of(program),
        Some(Runner::Npm | Runner::Pnpm | Runner::Yarn)
    )
}

pub(super) const R: Reason = Reason::UnresolvedScript;

pub(crate) use grammar::parse;

/// `*` matches any sequence, within one path segment when `segment`.
pub(crate) fn glob(pattern: &str, text: &str, segment: bool, b: &mut Budget) -> Checked<bool> {
    b.charge(1 + pattern.len() + text.len())?;
    let (p, t) = (pattern.as_bytes(), text.as_bytes());
    let (mut pi, mut ti, mut star, mut mark) = (0, 0, None, 0);
    while ti < t.len() {
        b.charge(1)?;
        if pi < p.len() && p[pi] == b'*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if let Some(s) = star.filter(|_| !(segment && t[mark] == b'/')) {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return Ok(false);
        }
    }
    Ok(p[pi..].iter().all(|c| *c == b'*'))
}

impl FanOut {
    fn matches(
        selector: &Selector,
        member: &WorkspaceMember,
        rel: &str,
        b: &mut Budget,
    ) -> Checked<bool> {
        match selector {
            Selector::Path(path) => glob(path, rel, true, b),
            Selector::Exact(x) => Ok(rel == x || member.name.as_deref() == Some(x.as_str())),
            // A runner may match an unnamed member by another identity.
            Selector::Name(name) => match &member.name {
                Some(own) => glob(name, own, false, b),
                None => Err(R),
            },
        }
    }
    /// Whether the root participates: `Some(false)` when the selection
    /// provably excludes it, `None` when only configuration could decide.
    fn root_runs(&self, root_name: Option<&str>, b: &mut Budget) -> Checked<Option<bool>> {
        match self.runner {
            Runner::Npm => Ok(self.include_root),
            Runner::Bun => {
                // Bun matches name filters against every workspace package,
                // the root included; path filters name member directories.
                let mut runs = false;
                for selector in &self.include {
                    if let Selector::Name(name) = selector {
                        match root_name {
                            Some(own) => runs |= glob(name, own, false, b)?,
                            None => return Ok(None),
                        }
                    }
                }
                Ok(if runs { None } else { Some(false) })
            }
            // Only member paths provably leave the root out.
            Runner::Pnpm | Runner::Yarn => Ok((!self.all
                && !self.include.is_empty()
                && self.include.iter().all(
                    |selector| matches!(selector, Selector::Path(path) if !path.is_empty() && path != "."),
                ))
            .then_some(false)),
        }
    }
    fn selects(&self, member: &WorkspaceMember, rel: &str, b: &mut Budget) -> Checked<bool> {
        let mut selected = self.all;
        for s in &self.include {
            selected = selected || Self::matches(s, member, rel, b)?;
        }
        for s in &self.exclude {
            if selected && Self::matches(s, member, rel, b)? {
                selected = false;
            }
        }
        Ok(selected)
    }
}

fn metadata(e: crate::scripts::MetadataError) -> (Reason, ScriptIssue) {
    (Reason::UnresolvedScript, ScriptIssue::Metadata(e))
}

impl<M: PackageMetadata> Graph<'_, M> {
    /// Resolve `fan_out` from the workspace root at `cwd`.
    pub(super) fn workspace(&mut self, cwd: &Path, fan_out: &FanOut, depth: usize) -> Result<()> {
        if self.runner != Some(fan_out.runner) {
            return Err(unsupported());
        }
        self.edge(depth)?;
        let members = self
            .source
            .workspace_members(cwd, &mut self.result.metadata)
            .map_err(metadata)?
            .ok_or_else(unsupported)?;
        let root = self.retain(cwd)?;
        // An adapter may report the root itself, which no glob can name, to
        // supply its manifest name; it is never one of the members.
        let root_name = members
            .iter()
            .find(|m| m.dir == root.package_dir)
            .and_then(|m| m.name.clone());
        let members: Vec<_> = members
            .into_iter()
            .filter(|m| m.dir != root.package_dir)
            .collect();
        let root_runs = fan_out
            .root_runs(root_name.as_deref(), &mut self.result.analysis)
            .map_err(super::syntax)?;
        let mut ran = 0usize;
        if root.scripts.contains_key(&fan_out.script) {
            match root_runs {
                Some(false) => (),
                Some(true) => {
                    // npm with `include-workspace-root` runs the root first.
                    self.member(&root, Path::new("."), fan_out, depth)?;
                    ran += 1;
                }
                None => return Err(unsupported()),
            }
        } else if root_runs == Some(true) && fan_out.require_script {
            // npm fails in a root that lacks the script.
            return Err(unsupported());
        }
        let mut matched = vec![false; fan_out.include.len()];
        for member in &members {
            let b = &mut self.result.analysis;
            let rel = member
                .dir
                .strip_prefix(&root.package_dir)
                .ok()
                .and_then(Path::to_str)
                .filter(|r| !r.is_empty())
                .ok_or_else(unsupported)?
                .to_owned();
            for (at, s) in fan_out.include.iter().enumerate() {
                matched[at] |= FanOut::matches(s, member, &rel, b).map_err(super::syntax)?;
            }
            if !fan_out.selects(member, &rel, b).map_err(super::syntax)? {
                continue;
            }
            let package = self.retain(&member.dir)?;
            if package.package_dir != member.dir {
                return Err(unsupported());
            }
            if !package.scripts.contains_key(&fan_out.script) {
                if fan_out.require_script {
                    return Err(unsupported());
                }
                continue;
            }
            self.member(&package, Path::new(&rel), fan_out, depth)?;
            ran += 1;
        }
        // npm fails for a `-w` that names no member; never guess for others.
        if ran == 0 || fan_out.runner == Runner::Npm && matched.contains(&false) {
            return Err(unsupported());
        }
        Ok(())
    }
    /// Classify `fan_out`'s script in one participating package.
    fn member(
        &mut self,
        package: &crate::scripts::PackageSnapshot,
        rel: &Path,
        fan_out: &FanOut,
        depth: usize,
    ) -> Result<()> {
        let name = &fan_out.script;
        if matches!(fan_out.runner, Runner::Pnpm | Runner::Yarn)
            && (package.scripts.contains_key(&format!("pre{name}"))
                || package.scripts.contains_key(&format!("post{name}")))
        {
            return Err(unsupported());
        }
        let start = self.leaves.len();
        self.script(&package.package_dir, name, &fan_out.args, depth + 1)?;
        let classification = crate::aggregate(&self.leaves[start..], &mut self.result.analysis)
            .map_err(super::syntax)?;
        self.result.members.push(MemberClassification {
            member: rel.to_owned(),
            script: name.clone(),
            classification,
        });
        Ok(())
    }
    fn retain(&mut self, dir: &Path) -> Result<crate::scripts::PackageSnapshot> {
        let package = self
            .source
            .package(dir, &mut self.result.metadata)
            .map_err(metadata)?;
        if !self
            .result
            .packages
            .iter()
            .any(|p| p.package_dir == package.package_dir)
        {
            self.result.packages.push(package.clone());
        }
        Ok(package)
    }
}

#[cfg(test)]
#[path = "workspace/tests.rs"]
mod tests;
