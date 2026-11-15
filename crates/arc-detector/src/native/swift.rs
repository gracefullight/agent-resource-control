//! Frozen SwiftPM argv only. No tool, SDK, filesystem or manifest inspection.
use super::scan;
use crate::{
    budget::{Budget, Checked},
    Classification, JobClass, Reason,
};

#[derive(Clone, Copy)]
enum Value {
    None,
    Configuration,
    Path,
    Count,
    Selector,
}
const PARALLEL: u16 = 1 << 6;
const WORKERS: u16 = 1 << 9;

pub(super) fn recognize(words: &[String], budget: &mut Budget) -> Checked<Classification> {
    let Some(verb) = words.get(1) else {
        return Err(Reason::UnresolvedCommand);
    };
    let test = if budget.eq(verb, "test")? {
        true
    } else if budget.eq(verb, "build")? {
        false
    } else {
        return Err(Reason::UnresolvedCommand);
    };
    let mut seen = 0_u16;
    let mut at = 2;
    while let Some(word) = words.get(at) {
        // Precharge splitting and finite option dispatch before scanning the word.
        scan(budget, word, 2, 32)?;
        let (flag, inline) = match word.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, Some(value)),
            Some(_) => return Err(Reason::UnresolvedCommand),
            None => (word.as_str(), None),
        };
        let (key, value) = match flag {
            "-v" | "--verbose" | "-q" | "--quiet" => (1, Value::None),
            "-c" | "--configuration" => (1 << 1, Value::Configuration),
            "--package-path" => (1 << 2, Value::Path),
            "--scratch-path" => (1 << 3, Value::Path),
            "-j" | "--jobs" => (1 << 4, Value::Count),
            "--build-tests" if !test => (1 << 5, Value::None),
            "--parallel" if test => (PARALLEL, Value::None),
            "--skip-build" if test => (1 << 7, Value::None),
            "--enable-code-coverage" if test => (1 << 8, Value::None),
            "--num-workers" if test => (WORKERS, Value::Count),
            // Selectors alone may repeat, including separate/equals mixtures.
            "--filter" | "--skip" if test => (0, Value::Selector),
            _ => return Err(Reason::UnresolvedCommand),
        };
        if seen & key != 0 {
            return Err(Reason::UnresolvedCommand);
        }
        seen |= key;
        at += 1;
        if matches!(value, Value::None) {
            if inline.is_some() {
                return Err(Reason::UnresolvedCommand);
            }
            continue;
        }
        let text = if let Some(text) = inline {
            text
        } else {
            let Some(text) = words.get(at) else {
                return Err(Reason::UnresolvedCommand);
            };
            at += 1;
            text
        };
        scan(budget, text, 1, 4)?;
        if text.is_empty() || text.starts_with('-') {
            return Err(Reason::UnresolvedCommand);
        }
        let valid = match value {
            Value::Configuration => budget.one_of(text, &["debug", "release"])?,
            Value::Count => count(text),
            Value::Path | Value::Selector => true,
            Value::None => unreachable!(),
        };
        if !valid {
            return Err(Reason::UnresolvedCommand);
        }
    }
    if seen & WORKERS != 0 && seen & PARALLEL == 0 {
        return Err(Reason::UnresolvedCommand);
    }
    Ok(Classification::new(
        if test {
            JobClass::Test
        } else {
            JobClass::Build
        },
        if test {
            Reason::SwiftTest
        } else {
            Reason::SwiftBuild
        },
    ))
}

fn count(text: &str) -> bool {
    let mut number = 0_u32;
    for byte in text.bytes() {
        if !byte.is_ascii_digit() {
            return false;
        }
        number = number * 10 + u32::from(byte - b'0');
        if number > 1024 {
            return false;
        }
    }
    number != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn swift_scans_charge_before_work_and_keep_checked_limits() {
        let words = ["swift", "test", "--filter=literal"].map(str::to_owned);
        let mut budget = Budget {
            work: crate::MAX_WORK - 5,
            ..Default::default()
        };
        assert_eq!(recognize(&words, &mut budget), Err(Reason::WorkLimit));
        assert!(budget.work <= crate::MAX_WORK);
    }
}
