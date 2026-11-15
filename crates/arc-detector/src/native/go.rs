//! The frozen native Go argv grammar. No tool, environment or project inspection.
use super::scan;
use crate::{
    budget::{Budget, Checked},
    Classification, JobClass, Reason,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Verb {
    Build,
    Test,
    Vet,
}
#[derive(Clone, Copy)]
enum Value {
    None,
    Opaque,
    Number,
    Module,
    Duration,
}

pub(super) fn recognize(words: &[String], budget: &mut Budget) -> Checked<Classification> {
    let reject = Reason::UnresolvedCommand;
    let mut at = 1;
    let first = words.get(at).ok_or(reject)?;
    scan(budget, first, 1, 1)?;
    if first == "-C" || first.starts_with("-C=") {
        let path = value(words, &mut at, Value::Opaque, budget)?;
        debug_assert!(!path.is_empty());
        at += 1;
    }
    let verb = words.get(at).ok_or(reject)?;
    scan(budget, verb, 1, 1)?;
    let (verb, class, reason) = match verb.as_str() {
        "build" => (Verb::Build, JobClass::Build, Reason::GoBuild),
        "test" => (Verb::Test, JobClass::Test, Reason::GoTest),
        "vet" => (Verb::Vet, JobClass::Lint, Reason::GoVet),
        _ => return Err(reject),
    };
    at += 1;
    let mut seen = 0_u32;
    let mut source_selectors = None;
    while let Some(word) = words.get(at) {
        scan(budget, word, 2, 16)?;
        if word.starts_with('-') {
            if source_selectors.is_some() {
                return Err(reject);
            }
            let name = word.split_once('=').map_or(word.as_str(), |(k, _)| k);
            let (key, kind) = flag(verb, name).ok_or(reject)?;
            let bit = 1_u32 << key;
            if seen & bit != 0 {
                return Err(reject);
            }
            seen |= bit;
            value(words, &mut at, kind, budget)?;
        } else {
            if word.is_empty() {
                return Err(reject);
            }
            let source = word.ends_with(".go");
            if source_selectors.is_some_and(|previous| previous != source) {
                return Err(reject);
            }
            if !source
                && !word.bytes().all(|c| {
                    c.is_ascii_alphanumeric()
                        || matches!(c, b'_' | b'.' | b'/' | b'~' | b'+' | b'-')
                })
            {
                return Err(reject);
            }
            source_selectors = Some(source);
        }
        at += 1;
    }
    Ok(Classification::new(class, reason))
}

fn flag(verb: Verb, name: &str) -> Option<(u8, Value)> {
    use Value::*;
    Some(match name {
        "-a" => (0, None),
        "-v" => (1, None),
        "-x" => (2, None),
        "-race" => (3, None),
        "-trimpath" => (4, None),
        "-p" => (5, Number),
        "-tags" => (6, Opaque),
        "-mod" => (7, Module),
        "-modfile" => (8, Opaque),
        "-o" if verb == Verb::Build => (9, Opaque),
        "-short" if verb == Verb::Test => (9, None),
        "-failfast" if verb == Verb::Test => (10, None),
        "-json" if verb == Verb::Test => (11, None),
        "-count" if verb == Verb::Test => (12, Number),
        "-parallel" if verb == Verb::Test => (13, Number),
        "-run" if verb == Verb::Test => (14, Opaque),
        "-skip" if verb == Verb::Test => (15, Opaque),
        "-bench" if verb == Verb::Test => (16, Opaque),
        "-timeout" if verb == Verb::Test => (17, Duration),
        "-benchtime" if verb == Verb::Test => (18, Duration),
        "-all" if verb == Verb::Vet => (9, None),
        "-composites" if verb == Verb::Vet => (10, None),
        "-printf" if verb == Verb::Vet => (11, None),
        "-structtag" if verb == Verb::Vet => (12, None),
        "-unusedresult" if verb == Verb::Vet => (13, None),
        _ => return Option::None,
    })
}

fn value<'a>(
    words: &'a [String],
    at: &mut usize,
    kind: Value,
    budget: &mut Budget,
) -> Checked<&'a str> {
    let reject = Reason::UnresolvedCommand;
    let attached = words[*at].split_once('=').map(|(_, v)| v);
    if matches!(kind, Value::None) {
        return if attached.is_none() {
            Ok("")
        } else {
            Err(reject)
        };
    }
    let text = match attached {
        Some(text) => text,
        None => {
            *at += 1;
            words.get(*at).ok_or(reject)?.as_str()
        }
    };
    scan(budget, text, 2, 1)?;
    if text.is_empty() || text.starts_with('-') {
        return Err(reject);
    }
    let valid = match kind {
        Value::None => unreachable!(),
        Value::Opaque => true,
        Value::Number => decimal(text).is_some_and(|n| n <= 1024),
        Value::Module => matches!(text, "readonly" | "vendor" | "mod"),
        Value::Duration => {
            let digits = text.bytes().take_while(u8::is_ascii_digit).count();
            decimal(&text[..digits]).is_some()
                && matches!(&text[digits..], "ns" | "us" | "ms" | "s" | "m" | "h")
        }
    };
    if valid {
        Ok(text)
    } else {
        Err(reject)
    }
}

fn decimal(text: &str) -> Option<u32> {
    if !text.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    text.parse().ok().filter(|n| *n > 0)
}
