//! The closed fan-out grammar of each runner (lane C, DET-009 item 2).
use super::{FanOut, Runner, Selector, R};
use crate::budget::{Budget, Checked};
use std::path::Path;

fn pattern(value: &str, b: &mut Budget) -> Checked<String> {
    b.charge(1 + value.len())?;
    if value.is_empty() || value.contains(['?', '[', ']', '{', '}', '^', '#', '!', '\0']) {
        return Err(R);
    }
    Ok(value.to_owned())
}
/// A name or `./path` selector of Bun, pnpm and Yarn.
fn selector(value: &str, b: &mut Budget) -> Checked<Selector> {
    let value = pattern(value, b)?;
    if value.contains("...") || value.contains("**") || value.starts_with("..") {
        return Err(R);
    }
    Ok(match value.strip_prefix("./") {
        Some("") => return Err(R),
        Some(path) => Selector::Path(path.trim_end_matches('/').to_owned()),
        None if value.starts_with('/') || value.starts_with('.') => return Err(R),
        None => Selector::Name(value),
    })
}
fn script_name(name: &str) -> Checked<String> {
    if name.is_empty()
        || name.contains('/')
        || name.starts_with(['.', '-'])
        || name.contains(['*', '?', '[', '\0'])
    {
        return Err(R);
    }
    Ok(name.to_owned())
}
/// The value of `--flag value` or `--flag=value`; `None` when `word` is not it.
fn value<'a>(
    words: &'a [String],
    i: &mut usize,
    names: &[&str],
    b: &mut Budget,
) -> Checked<Option<&'a str>> {
    let word = &words[*i];
    for name in names {
        if word == name {
            *i += 1;
            let value = words.get(*i).filter(|v| !v.starts_with('-')).ok_or(R)?;
            b.charge(value.len())?;
            return Ok(Some(value));
        }
        if let Some(value) = word.strip_prefix(name).and_then(|v| v.strip_prefix('=')) {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

/// `Ok(None)`: `words` is not a workspace fan-out, and nothing was decided.
pub(crate) fn parse(words: &[String], _cwd: &Path, b: &mut Budget) -> Checked<Option<FanOut>> {
    let Some(runner) = words.first().and_then(|w| Runner::of(w)) else {
        return Ok(None);
    };
    b.charge(words.iter().map(|w| 1 + w.len()).sum())?;
    match runner {
        Runner::Bun => bun(words, b),
        Runner::Npm => npm(words, b),
        Runner::Pnpm => pnpm(words, b),
        Runner::Yarn => yarn(words, b),
    }
}

fn fan_out(runner: Runner, script: &str, rest: &[String]) -> Checked<FanOut> {
    let args = match rest.first() {
        Some(first) if first == "--" => rest[1..].to_vec(),
        _ => rest.to_vec(),
    };
    Ok(FanOut {
        runner,
        all: false,
        include: Vec::new(),
        exclude: Vec::new(),
        script: script_name(script)?,
        args,
        require_script: false,
        include_root: None,
    })
}

fn bun(words: &[String], b: &mut Budget) -> Checked<Option<FanOut>> {
    // Only a filter before the script name makes a fan-out; later words are
    // the script's own arguments and the single-package grammar applies.
    let (mut i, mut run, mut filters) = (1, false, Vec::new());
    while let Some(word) = words.get(i) {
        if word == "run" && !run {
            run = true;
        } else if let Some(v) = value(words, &mut i, &["--filter", "-F"], b)? {
            filters.push(selector(v, b)?);
        } else if word.starts_with('-') || filters.is_empty() {
            // Not a fan-out unless a filter was already seen.
            return if filters.is_empty() { Ok(None) } else { Err(R) };
        } else {
            let mut out = fan_out(Runner::Bun, word, &words[i + 1..])?;
            out.include = filters;
            return Ok(Some(out));
        }
        i += 1;
    }
    if filters.is_empty() {
        Ok(None)
    } else {
        Err(R)
    }
}

fn npm(words: &[String], b: &mut Budget) -> Checked<Option<FanOut>> {
    if !words
        .get(1)
        .is_some_and(|w| matches!(w.as_str(), "run" | "run-script"))
    {
        return Ok(None);
    }
    let end = words.iter().position(|w| w == "--").unwrap_or(words.len());
    let fan = words[2..end].iter().any(|w| {
        matches!(
            w.as_str(),
            "--workspaces" | "--ws" | "-ws" | "-w" | "--workspace"
        ) || w.starts_with("--workspace=")
            || w.starts_with("--workspaces=")
    });
    if !fan {
        return Ok(None);
    }
    let (mut i, mut script, mut args) = (2, None, Vec::new());
    let (mut all, mut include, mut if_present, mut include_root) = (false, Vec::new(), false, None);
    while i < end {
        let word = words[i].as_str();
        match word {
            "--workspaces" | "--ws" | "-ws" | "--workspaces=true" => all = true,
            "--if-present" => if_present = true,
            "--no-include-workspace-root" | "--include-workspace-root=false" => {
                include_root = Some(false)
            }
            "--include-workspace-root" | "--include-workspace-root=true" => {
                include_root = Some(true)
            }
            "--silent" | "-s" => (),
            _ if word.starts_with('-') => match value(words, &mut i, &["--workspace", "-w"], b)? {
                Some(v) if !v.is_empty() && !v.contains(['*', '?', '[']) => {
                    include.push(Selector::Exact(
                        v.trim_start_matches("./").trim_end_matches('/').to_owned(),
                    ));
                }
                // An unknown npm option may change npm's own configuration.
                _ => return Err(R),
            },
            _ if script.is_none() => script = Some(word),
            _ => args.push(words[i].clone()),
        }
        i += 1;
    }
    args.extend(words.get(end + 1..).unwrap_or_default().iter().cloned());
    let mut out = fan_out(Runner::Npm, script.ok_or(R)?, &[])?;
    out.args = args;
    out.all = all;
    out.include = include;
    out.require_script = !if_present;
    out.include_root = include_root;
    Ok(Some(out))
}

fn pnpm(words: &[String], b: &mut Budget) -> Checked<Option<FanOut>> {
    let (mut i, mut run, mut recursive) = (1, false, false);
    let (mut include, mut exclude, mut seen) = (Vec::new(), Vec::new(), false);
    while let Some(word) = words.get(i) {
        match word.as_str() {
            "-r" | "--recursive" => (recursive, seen) = (true, true),
            "run" | "run-script" if !run => run = true,
            "--parallel" | "--stream" | "--no-bail" | "--if-present" | "--sequential" => (),
            _ if word.starts_with('-') => match value(words, &mut i, &["--filter", "-F"], b)? {
                Some(v) => {
                    seen = true;
                    match v.strip_prefix('!') {
                        Some(v) => exclude.push(selector(v, b)?),
                        None => include.push(selector(v, b)?),
                    }
                }
                // `-w` selects the root; other options may change selection.
                None => return if seen { Err(R) } else { Ok(None) },
            },
            _ if !seen => return Ok(None),
            _ => {
                // pnpm's built-in commands take precedence over scripts;
                // `test` and `start` run the script of that name. Any other
                // name is script shorthand, which pnpm may otherwise run as
                // a binary: every selected member must define it.
                let (script, require) = match word.as_str() {
                    _ if run => (word.as_str(), false),
                    "test" | "t" | "tst" => ("test", false),
                    "start" => ("start", true),
                    name if PNPM_BUILTINS.contains(&name) => return Err(R),
                    name => (name, true),
                };
                let mut out = fan_out(Runner::Pnpm, script, &words[i + 1..])?;
                out.require_script = require;
                out.all = include.is_empty() && (recursive || !exclude.is_empty());
                (out.include, out.exclude) = (include, exclude);
                return Ok(Some(out));
            }
        }
        i += 1;
    }
    if seen {
        Err(R)
    } else {
        Ok(None)
    }
}

/// pnpm commands, aliases and npm pass-through commands that are not scripts.
#[rustfmt::skip]
const PNPM_BUILTINS: &[&str] = &[
    "add", "adduser", "approve-builds", "audit", "bin", "bugs", "c", "cache", "cat-file",
    "cat-index", "catalog", "ci", "cit", "clean-install", "clean-install-test", "completion",
    "config", "create", "dedupe", "deploy", "deprecate", "dist-tag", "dlx", "docs", "doctor",
    "edit", "env", "exec", "fetch", "find-hash", "get", "help", "i", "ignored-builds", "import",
    "info", "init", "install", "install-test", "issues", "it", "la", "licenses", "link", "list",
    "ll", "ln", "login", "logout", "ls", "m", "multi", "node", "outdated", "owner", "pack",
    "patch", "patch-commit", "patch-remove", "ping", "prefix", "profile", "prune", "publish", "rb",
    "rebuild", "recursive", "remove", "repo", "restart", "rm", "root", "run", "run-script",
    "runtime", "s", "sbom", "se", "search", "self-update", "server", "set", "set-script", "setup",
    "show", "star", "stop", "store", "team", "token", "un", "undeprecate", "uninstall", "unlink",
    "unpublish", "unstar", "up", "update", "upgrade", "v", "version", "view", "whoami", "why",
    "with", "xmas",
];

fn yarn(words: &[String], b: &mut Budget) -> Checked<Option<FanOut>> {
    if words.get(1).is_none_or(|w| w != "workspaces") || words.get(2).is_none_or(|w| w != "foreach")
    {
        return Ok(None);
    }
    let (mut i, mut all) = (3, false);
    let (mut include, mut exclude) = (Vec::new(), Vec::new());
    while let Some(word) = words.get(i) {
        match word.as_str() {
            "-A" | "--all" | "-W" | "--worktree" => all = true,
            "-p" | "--parallel" | "-t" | "--topological" | "--topological-dev" | "-v"
            | "--verbose" | "-i" | "--interlaced" => (),
            "run" if all => {
                let script = words.get(i + 1).ok_or(R)?;
                let mut out = fan_out(Runner::Yarn, script, &words[i + 2..])?;
                out.all = include.is_empty();
                (out.include, out.exclude) = (include, exclude);
                return Ok(Some(out));
            }
            _ if value(words, &mut i, &["-j", "--jobs"], b)?.is_some() => (),
            _ => {
                if let Some(v) = value(words, &mut i, &["--include"], b)? {
                    include.push(Selector::Name(pattern(v, b)?));
                } else if let Some(v) = value(words, &mut i, &["--exclude"], b)? {
                    exclude.push(Selector::Name(pattern(v, b)?));
                } else {
                    // `-R`, `--since`, `--from` and the rest are dynamic.
                    return Err(R);
                }
            }
        }
        i += 1;
    }
    Err(R)
}
