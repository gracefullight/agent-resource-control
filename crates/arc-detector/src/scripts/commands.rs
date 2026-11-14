//! Closed command grammar for the supported one-shot package-script slice.
use crate::{
    budget::{Budget, Checked},
    rules, Classification, JobClass as C, Reason as R,
};
use std::path::{Path, PathBuf};

pub(super) fn is_bun(program: &str) -> bool {
    Path::new(program).file_name().is_some_and(|s| s == "bun")
}
pub(super) struct ScriptCall {
    pub cwd: PathBuf,
    pub name: String,
    pub args: Vec<String>,
}
pub(super) fn script_call(
    words: &[String],
    cwd: &Path,
    b: &mut Budget,
) -> Checked<Option<ScriptCall>> {
    b.charge(1 + words.first().map_or(0, String::len))?;
    if !words.first().is_some_and(|w| is_bun(w)) {
        return Ok(None);
    }
    let (mut i, mut run, mut explicit_cwd) = (1, false, None);
    while let Some(word) = words.get(i) {
        b.charge(20 + word.len())?;
        match word.as_str() {
            "run" if !run => run = true,
            "--silent" | "-s" => (),
            // Bun 1.4.4 consumes a separated --cwd value before `run` as
            // the entry point, printing usage instead of running that script.
            "--cwd" if run && explicit_cwd.is_none() => {
                i += 1;
                let value = words
                    .get(i)
                    .filter(|v| !v.is_empty() && !v.starts_with('-'))
                    .ok_or(R::UnresolvedScript)?;
                b.charge(value.len() + 1)?;
                explicit_cwd = Some(cwd.join(value));
            }
            value if value.starts_with("--cwd=") && explicit_cwd.is_none() => {
                let value = &value[6..];
                if value.is_empty() {
                    return Err(R::UnresolvedScript);
                }
                explicit_cwd = Some(cwd.join(value));
            }
            "--" if run => {
                i += 1;
                let Some(name) = words.get(i) else {
                    return Err(R::UnresolvedScript);
                };
                return call(name, &words[i + 1..], explicit_cwd, cwd);
            }
            value if !value.starts_with('-') && run => {
                // File/binary fallbacks, patterns and workspace fan-out are not
                // script resolution. Missing script maps never guess a binary.
                return call(value, &words[i + 1..], explicit_cwd, cwd);
            }
            _ if !run && i == 1 && matches!(word.as_str(), "build" | "test") => return Ok(None),
            _ => return Err(R::UnresolvedScript),
        }
        i += 1;
    }
    Err(R::UnresolvedScript)
}
fn call(
    name: &str,
    args: &[String],
    explicit_cwd: Option<PathBuf>,
    cwd: &Path,
) -> Checked<Option<ScriptCall>> {
    if name.is_empty()
        || name.contains('/')
        || name.starts_with(['.', '-'])
        || name.contains(['*', '?', '['])
    {
        return Err(R::UnresolvedScript);
    }
    // Bun strips exactly one delimiter immediately following the script name.
    let args = if args.first().is_some_and(|arg| arg == "--") {
        &args[1..]
    } else {
        args
    };
    Ok(Some(ScriptCall {
        cwd: explicit_cwd.unwrap_or_else(|| cwd.to_owned()),
        name: name.to_owned(),
        args: args.to_vec(),
    }))
}

pub(super) fn leaf(words: &[String], b: &mut Budget) -> Checked<Classification> {
    b.charge(words.iter().map(|w| 20 + 4 * w.len()).sum())?;
    let unknown = || Classification::unknown(R::UnresolvedCommand);
    let Some(program) = words.first() else {
        return Ok(unknown());
    };
    let class = if is_bun(program) && words.get(1).is_some_and(|w| w == "test") {
        return rules::recognize_bun_test(&words[2..], b);
    } else if is_bun(program) && words.get(1).is_some_and(|w| w == "build") {
        options(
            &words[2..],
            &[
                "--minify",
                "--minify-syntax",
                "--minify-whitespace",
                "--minify-identifiers",
                "--splitting",
                "--compile",
                "--bytecode",
                "--production",
            ],
            &[
                "--outdir",
                "--outfile",
                "--target",
                "--format",
                "--external",
                "--define",
                "--naming",
                "--public-path",
                "--sourcemap",
                "--banner",
                "--footer",
            ],
            false,
        )
        .then_some(C::Build)
    } else if program == "vite" && words.get(1).is_some_and(|w| w == "build") {
        options(
            &words[2..],
            &[
                "--emptyOutDir",
                "--ssrManifest",
                "--manifest",
                "--clearScreen",
            ],
            &[
                "--mode",
                "--config",
                "-c",
                "--base",
                "--outDir",
                "--target",
                "--logLevel",
                "-l",
                "--minify",
                "--sourcemap",
            ],
            false,
        )
        .then_some(C::Build)
    } else if program == "biome" && words.get(1).is_some_and(|w| w == "check") {
        options(
            &words[2..],
            &[
                "--write",
                "--unsafe",
                "--error-on-warnings",
                "--no-errors-on-unmatched",
                "--verbose",
                "--skip-errors",
            ],
            &[
                "--config-path",
                "--reporter",
                "--max-diagnostics",
                "--files-ignore-unknown",
                "--formatter-enabled",
                "--linter-enabled",
                "--assist-enabled",
                "--colors",
            ],
            false,
        )
        .then_some(C::Lint)
    } else if program == "tsc" {
        typecheck(&words[1..]).then_some(C::Typecheck)
    } else if matches!(program.as_str(), "gh" | "ssh") {
        let c = rules::recognize(words, b)?;
        return Ok(if c.locality == crate::Locality::Remote {
            c
        } else {
            unknown()
        });
    } else {
        None
    };
    Ok(match class {
        Some(C::Build) => Classification::new(C::Build, R::BunScriptBuild),
        Some(C::Lint) => Classification::new(C::Lint, R::BunScriptLint),
        Some(C::Typecheck) => Classification::new(C::Typecheck, R::BunScriptTypecheck),
        _ => unknown(),
    })
}

fn options(words: &[String], flags: &[&str], values: &[&str], boolean: bool) -> bool {
    let mut i = 0;
    while let Some(word) = words.get(i) {
        if word.is_empty() || word == "--" {
            return false;
        }
        if !word.starts_with('-') {
            i += 1;
            continue;
        }
        let (key, assigned) = word
            .split_once('=')
            .map_or((word.as_str(), None), |(k, v)| (k, Some(v)));
        if flags.contains(&key)
            && (assigned.is_none() || boolean && matches!(assigned, Some("true" | "false")))
        {
            i += 1;
            continue;
        }
        if !values.contains(&key) {
            return false;
        }
        let value = match assigned {
            Some(v) => v,
            None => {
                i += 1;
                let Some(v) = words.get(i) else { return false };
                v
            }
        };
        if value.is_empty() || value.starts_with('-') {
            return false;
        }
        i += 1;
    }
    true
}
fn typecheck(words: &[String]) -> bool {
    let (mut found, mut i) = (false, 0);
    while let Some(word) = words.get(i) {
        // TypeScript expands response files before parsing options. Their
        // contents could enable watch or override noEmit, and are outside the
        // bounded package snapshot. Never infer a leaf from an uninspected file.
        if word.starts_with('@') {
            return false;
        }
        match word.as_str() {
            "--noEmit"
            | "--strict"
            | "--skipLibCheck"
            | "--pretty"
            | "--incremental"
            | "--allowJs"
            | "--checkJs"
            | "--isolatedModules"
            | "--esModuleInterop"
            | "--resolveJsonModule"
            | "--strictNullChecks"
            | "--noUnusedLocals"
            | "--noUnusedParameters"
            | "--forceConsistentCasingInFileNames" => {
                let no_emit = word == "--noEmit";
                found |= no_emit;
                if let Some(value) = words
                    .get(i + 1)
                    .filter(|v| matches!(v.as_str(), "true" | "false"))
                {
                    if no_emit && value == "false" {
                        return false;
                    }
                    i += 1;
                }
            }
            "-p" | "--project" | "--target" | "-t" | "--module" | "-m" | "--moduleResolution"
            | "--lib" | "--types" | "--jsx" | "--tsBuildInfoFile" => {
                i += 1;
                let Some(value) = words.get(i) else {
                    return false;
                };
                if value.is_empty() || value.starts_with(['-', '@']) {
                    return false;
                }
            }
            value if !value.is_empty() && !value.starts_with('-') => (),
            // Unlike the other supported tools, tsc accepts these options only
            // with separated values, not --key=value (including --noEmit=true).
            _ => return false,
        }
        i += 1;
    }
    found
}
