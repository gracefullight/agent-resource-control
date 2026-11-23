//! Bounded workspace glob membership. Only directories the declared globs
//! name are examined; nothing else under the root is listed or read.
use arc_detector::scripts::{MetadataError as E, MetadataStats, MAX_PACKAGE_LOOKUPS};
use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};

/// Declared globs accepted per workspace.
pub(crate) const MAX_GLOBS: usize = 64;
const MAX_GLOB_BYTES: usize = 256;
/// Directory entries examined by one expansion, across all wildcards.
pub(crate) const MAX_DIRECTORY_ENTRIES: usize = 1024;

fn bounded(globs: Vec<String>) -> Result<Vec<String>, E> {
    if globs.len() > MAX_GLOBS || globs.iter().any(|g| g.len() > MAX_GLOB_BYTES) {
        return Err(E::Invalid);
    }
    Ok(globs)
}
fn strings(value: &serde_json::Value) -> Result<Vec<String>, E> {
    value
        .as_array()
        .ok_or(E::Invalid)?
        .iter()
        .map(|g| g.as_str().map(str::to_owned).ok_or(E::Invalid))
        .collect::<Result<_, _>>()
        .and_then(bounded)
}

/// `workspaces` as an array, or as `{ "packages": [...] }`. Absent: none.
pub(crate) fn package_json_globs(manifest: &serde_json::Value) -> Result<Vec<String>, E> {
    match manifest.get("workspaces") {
        None => Err(E::Missing),
        Some(list @ serde_json::Value::Array(_)) => strings(list),
        Some(serde_json::Value::Object(object)) => {
            strings(object.get("packages").ok_or(E::Invalid)?)
        }
        Some(_) => Err(E::Invalid),
    }
}

/// `packages` of `pnpm-workspace.yaml`; other keys are not membership.
pub(crate) fn pnpm_globs(bytes: &[u8]) -> Result<Vec<String>, E> {
    let text = std::str::from_utf8(bytes).map_err(|_| E::Invalid)?;
    let value: serde_norway::Value = serde_norway::from_str(text).map_err(|_| E::Invalid)?;
    let Some(packages) = value.get("packages") else {
        return Ok(Vec::new());
    };
    packages
        .as_sequence()
        .ok_or(E::Invalid)?
        .iter()
        .map(|g| g.as_str().map(str::to_owned).ok_or(E::Invalid))
        .collect::<Result<_, _>>()
        .and_then(bounded)
}

/// Relative segments of one static glob. `**` is a whole path segment.
fn segments(pattern: &str) -> Result<Vec<&str>, E> {
    let pattern = pattern.strip_prefix("./").unwrap_or(pattern);
    let pattern = pattern.trim_end_matches('/');
    if pattern.is_empty() || pattern.starts_with('/') || pattern.contains(['{', '}', '\\', '\0']) {
        return Err(E::Invalid);
    }
    let parts: Vec<&str> = pattern.split('/').collect();
    if parts.iter().any(|p| {
        p.is_empty()
            || *p == "."
            || *p == ".."
            || (p.contains("**") && *p != "**")
            || !valid_segment(p)
    }) {
        return Err(E::Invalid);
    }
    Ok(parts)
}

fn valid_segment(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'[' => {
                i += 1;
                if bytes.get(i) == Some(&b'!') || bytes.get(i) == Some(&b'^') {
                    i += 1;
                }
                let start = i;
                while i < bytes.len() && bytes[i] != b']' {
                    i += 1;
                }
                if i == start || i == bytes.len() {
                    return false;
                }
            }
            b']' | b'!' => return false,
            _ => (),
        }
        i += 1;
    }
    true
}

/// Expand a bounded static brace choice before walking the filesystem.
fn alternatives(pattern: &str) -> Result<Vec<String>, E> {
    let mut pending = vec![pattern.to_owned()];
    let mut output = Vec::new();
    while let Some(value) = pending.pop() {
        if let Some(start) = value.find('{') {
            let end = value[start + 1..]
                .find('}')
                .map(|n| n + start + 1)
                .ok_or(E::Invalid)?;
            let choices = &value[start + 1..end];
            if choices.contains('{') || !choices.contains(',') {
                return Err(E::Invalid);
            }
            for choice in choices.split(',') {
                if choice.is_empty() || pending.len() + output.len() >= MAX_GLOBS {
                    return Err(E::Invalid);
                }
                pending.push(format!("{}{choice}{}", &value[..start], &value[end + 1..]));
            }
        } else {
            if value.contains('}') {
                return Err(E::Invalid);
            }
            output.push(value);
        }
    }
    Ok(output)
}

/// Match `*`, `?` and character classes in one directory name, by Unicode
/// scalar value as Bun's and Node's glob matchers do, never by UTF-8 byte.
pub(crate) fn matches(pattern: &str, name: &str) -> bool {
    fn go(
        p: &[char],
        t: &[char],
        memo: &mut std::collections::BTreeMap<(usize, usize), bool>,
        i: usize,
        j: usize,
    ) -> bool {
        if let Some(value) = memo.get(&(i, j)) {
            return *value;
        }
        let value = if i == p.len() {
            j == t.len()
        } else if p[i] == '*' {
            go(p, t, memo, i + 1, j) || (j < t.len() && go(p, t, memo, i, j + 1))
        } else if j == t.len() {
            false
        } else if p[i] == '?' {
            go(p, t, memo, i + 1, j + 1)
        } else if p[i] == '[' {
            let end = p[i + 1..]
                .iter()
                .position(|c| *c == ']')
                .map(|n| n + i + 1)
                .unwrap_or(p.len());
            let mut k = i + 1;
            let negate = p.get(k).is_some_and(|c| *c == '!' || *c == '^');
            if negate {
                k += 1;
            }
            let mut found = false;
            while k < end {
                if k + 2 < end && p[k + 1] == '-' {
                    found |= p[k] <= t[j] && t[j] <= p[k + 2];
                    k += 3;
                } else {
                    found |= p[k] == t[j];
                    k += 1;
                }
            }
            end < p.len() && (found != negate) && go(p, t, memo, end + 1, j + 1)
        } else {
            p[i] == t[j] && go(p, t, memo, i + 1, j + 1)
        };
        memo.insert((i, j), value);
        value
    }
    if !valid_segment(pattern) {
        return false;
    }
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    go(
        &pattern,
        &name,
        &mut std::collections::BTreeMap::new(),
        0,
        0,
    )
}

fn walk(
    root: &Path,
    pattern: &str,
    budget: &mut MetadataStats,
    entries: &mut usize,
) -> Result<BTreeSet<PathBuf>, E> {
    let parts = segments(pattern)?;
    let mut pending = vec![(root.to_owned(), 0usize)];
    let mut found = BTreeSet::new();
    while let Some((dir, at)) = pending.pop() {
        if at == parts.len() {
            found.insert(dir);
            if found.len() > MAX_PACKAGE_LOOKUPS {
                return Err(E::LookupLimit);
            }
            continue;
        }
        budget.lookup()?;
        let segment = parts[at];
        if segment == "**" {
            pending.push((dir.clone(), at + 1));
        }
        if segment != "**" && !segment.contains(['*', '?', '[']) {
            match fs::symlink_metadata(dir.join(segment)) {
                Ok(meta) if meta.is_dir() => pending.push((dir.join(segment), at + 1)),
                Ok(_) => (),
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(_) => return Err(E::Unreadable),
            }
            continue;
        }
        let listing = match fs::read_dir(&dir) {
            Ok(listing) => listing,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => return Err(E::Unreadable),
        };
        for entry in listing {
            *entries += 1;
            if *entries > MAX_DIRECTORY_ENTRIES {
                return Err(E::LookupLimit);
            }
            let entry = entry.map_err(|_| E::Unreadable)?;
            let name = entry.file_name();
            // A name the matcher cannot read could be a member: fail closed.
            let name = name.to_str().ok_or(E::Invalid)?;
            if name.starts_with('.')
                || name == "node_modules"
                || (segment != "**" && !matches(segment, name))
            {
                continue;
            }
            if entry.file_type().map_err(|_| E::Unreadable)?.is_dir() {
                pending.push((dir.join(name), if segment == "**" { at } else { at + 1 }));
                if pending.len() > MAX_PACKAGE_LOOKUPS {
                    return Err(E::LookupLimit);
                }
            }
        }
    }
    Ok(found)
}

/// Candidate member directories, sorted: those the positive globs name minus
/// those a `!` glob names. Hidden entries, `node_modules` and symlinks are
/// never members. Whether a candidate holds a manifest is the caller's read.
pub(crate) fn expand(
    root: &Path,
    globs: &[String],
    budget: &mut MetadataStats,
) -> Result<Vec<PathBuf>, E> {
    let (mut include, mut exclude) = (BTreeSet::new(), BTreeSet::new());
    let mut entries = 0usize;
    for glob in globs {
        let (negated, pattern) = match glob.strip_prefix('!') {
            Some(pattern) => (true, pattern),
            None => (false, glob.as_str()),
        };
        for alternative in alternatives(pattern)? {
            let dirs = walk(root, &alternative, budget, &mut entries)?;
            if negated { &mut exclude } else { &mut include }.extend(dirs);
        }
    }
    Ok(include.difference(&exclude).cloned().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations_and_patterns_are_closed_and_bounded() {
        let json = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap();
        assert_eq!(
            package_json_globs(&json(r#"{"workspaces":["packages/*","!packages/x"]}"#)),
            Ok(vec!["packages/*".into(), "!packages/x".into()])
        );
        assert_eq!(
            package_json_globs(&json(r#"{"workspaces":{"packages":["apps/*"]}}"#)),
            Ok(vec!["apps/*".into()])
        );
        assert_eq!(package_json_globs(&json("{}")), Err(E::Missing));
        for invalid in [
            r#"{"workspaces":"packages/*"}"#,
            r#"{"workspaces":[1]}"#,
            r#"{"workspaces":{"nohoist":[]}}"#,
        ] {
            assert_eq!(
                package_json_globs(&json(invalid)),
                Err(E::Invalid),
                "{invalid}"
            );
        }
        let many = format!(r#"{{"workspaces":{:?}}}"#, vec!["a"; MAX_GLOBS + 1]);
        assert_eq!(package_json_globs(&json(&many)), Err(E::Invalid));
        assert_eq!(
            pnpm_globs(b"packages:\n  - 'packages/*'\ncatalog:\n  x: 1\n"),
            Ok(vec!["packages/*".into()])
        );
        assert_eq!(pnpm_globs(b"onlyBuiltDependencies: []\n"), Ok(vec![]));
        assert_eq!(pnpm_globs(b"packages: packages/*\n"), Err(E::Invalid));
        assert_eq!(pnpm_globs(b"\xff"), Err(E::Invalid));
        for pattern in [
            "", "/abs", "a/../b", "a/[ab", "a/{b}", "a//b", "./", "a/x**y",
        ] {
            assert_eq!(segments(pattern), Err(E::Invalid), "{pattern:?}");
        }
        assert_eq!(segments("./packages/*/"), Ok(vec!["packages", "*"]));
        assert!(matches("*", "a") && matches("p*-x", "pkg-x") && !matches("a*", "ba"));
        assert_eq!(segments("packages/**"), Ok(vec!["packages", "**"]));
        assert!(matches("[a-c]?", "b2") && !matches("[!a-c]*", "b2"));
        // One `?` is one character, whatever its UTF-8 length.
        for name in ["a", "é", "한", "😀"] {
            assert!(matches("?", name), "{name}");
            assert!(!matches("??", name), "{name}");
        }
        assert!(matches("[가-힣]", "한") && !matches("[!가-힣]", "한") && matches("é?", "éx"));
        assert_eq!(
            alternatives("{apps,packages}/[ab]"),
            Ok(vec!["packages/[ab]".into(), "apps/[ab]".into()])
        );
    }
}
