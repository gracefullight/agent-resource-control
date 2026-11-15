//! The finite native-argv Python TEST grammar. No filesystem or runtime probes.
use crate::{
    budget::{Budget, Checked},
    Classification, JobClass, Reason,
};

fn scan(b: &mut Budget, word: &str, passes: usize, fixed: usize) -> Checked<()> {
    b.charge(
        word.len()
            .checked_mul(passes)
            .and_then(|n| n.checked_add(fixed))
            .ok_or(Reason::WorkLimit)?,
    )
}

/// `None` means another native recognizer may handle the executable. Once a
/// Python entry point is seen, an unresolved argument rejects the whole form.
pub(crate) fn recognize(words: &[String], b: &mut Budget) -> Checked<Option<Classification>> {
    let head = &words[0];
    scan(b, head, 2, 4)?; // basename search and trailing-slash check, before scanning
    let basename = head.rsplit('/').next().unwrap_or("");
    let (reason, args) = if b.eq(basename, "pytest")? {
        (Reason::PythonPytest, &words[1..])
    } else if b.one_of(
        basename,
        &[
            "python",
            "python3",
            "python3.8",
            "python3.9",
            "python3.10",
            "python3.11",
            "python3.12",
            "python3.13",
            "python3.14",
            "python3.15",
        ],
    )? {
        if words.len() < 3 || !b.eq(&words[1], "-m")? {
            return Ok(Some(Classification::unknown(Reason::UnresolvedCommand)));
        }
        if b.eq(&words[2], "pytest")? {
            (Reason::PythonPytest, &words[3..])
        } else if b.eq(&words[2], "unittest")? {
            (Reason::PythonUnittest, &words[3..])
        } else {
            return Ok(Some(Classification::unknown(Reason::UnresolvedCommand)));
        }
    } else {
        return Ok(None);
    };
    let valid = if reason == Reason::PythonPytest {
        pytest(args, b)?
    } else {
        unittest(args, b)?
    };
    Ok(Some(if valid {
        Classification::new(JobClass::Test, reason)
    } else {
        Classification::unknown(Reason::UnresolvedCommand)
    }))
}

#[derive(Clone, Copy)]
enum ValueKind {
    Opaque,
    Positive,
    Traceback,
    Capture,
    Color,
}

fn option_value<'a>(
    words: &'a [String],
    index: &mut usize,
    assigned: Option<&'a str>,
    b: &mut Budget,
) -> Checked<Option<&'a str>> {
    let value = match assigned {
        Some(value) => value,
        None => {
            *index += 1;
            let Some(value) = words.get(*index) else {
                return Ok(None);
            };
            value
        }
    };
    b.charge(2)?;
    Ok((!value.is_empty() && !value.starts_with('-')).then_some(value))
}

fn value_valid(value: &str, kind: ValueKind, b: &mut Budget) -> Checked<bool> {
    Ok(match kind {
        ValueKind::Opaque => true,
        ValueKind::Positive => {
            scan(b, value, 2, 1)?; // decimal validation and checked numeric parse
            value.bytes().all(|c| c.is_ascii_digit()) && value.parse::<u32>().is_ok_and(|n| n > 0)
        }
        ValueKind::Traceback => {
            b.one_of(value, &["auto", "long", "short", "line", "native", "no"])?
        }
        ValueKind::Capture => b.one_of(value, &["fd", "sys", "no", "tee-sys"])?,
        ValueKind::Color => b.one_of(value, &["yes", "no", "auto"])?,
    })
}

fn pytest(words: &[String], b: &mut Budget) -> Checked<bool> {
    let mut index = 0;
    while let Some(word) = words.get(index) {
        b.charge(3)?; // token/empty/leading-option inspections
        if word.is_empty() {
            return Ok(false);
        }
        if !word.starts_with('-') {
            // Brackets in the path are literal. Node parameters are opaque,
            // including embedded `::`; empty path/node components still fail.
            scan(b, word, 4, 1)?;
            if !pytest_selector(word) {
                return Ok(false);
            }
        } else if !b.one_of(
            word,
            &[
                "-q",
                "--quiet",
                "-qq",
                "-v",
                "--verbose",
                "-vv",
                "-x",
                "--exitfirst",
                "-s",
                "--disable-warnings",
                "--collect-only",
                "--co",
            ],
        )? {
            scan(b, word, 1, 2)?;
            let (option, assigned) = word
                .split_once('=')
                .map_or((word.as_str(), None), |(k, v)| (k, Some(v)));
            let mut kind = None;
            for (name, candidate) in [
                ("-k", ValueKind::Opaque),
                ("-m", ValueKind::Opaque),
                ("--maxfail", ValueKind::Positive),
                ("--tb", ValueKind::Traceback),
                ("--capture", ValueKind::Capture),
                ("--color", ValueKind::Color),
            ] {
                if b.eq(option, name)? {
                    kind = Some(candidate);
                    break;
                }
            }
            let Some(kind) = kind else { return Ok(false) };
            if assigned.is_some() && !option.starts_with("--") {
                return Ok(false);
            }
            let Some(value) = option_value(words, &mut index, assigned, b)? else {
                return Ok(false);
            };
            if !value_valid(value, kind, b)? {
                return Ok(false);
            }
        }
        index += 1;
    }
    Ok(true)
}

/// Called only after the selector's full scan allowance has been charged.
fn pytest_selector(word: &str) -> bool {
    let Some((path, nodes)) = word.split_once("::") else {
        return true;
    };
    if path.is_empty() {
        return false;
    }
    let mut bytes = nodes.bytes().peekable();
    let mut named = false;
    let mut parameter = false;
    while let Some(byte) = bytes.next() {
        if parameter {
            if byte == b']' {
                parameter = false;
            }
        } else if byte == b'[' {
            if !named {
                return false;
            }
            parameter = true;
        } else if byte == b':' && bytes.peek() == Some(&b':') {
            if !named {
                return false;
            }
            bytes.next();
            named = false;
        } else {
            named = true;
        }
    }
    named && !parameter
}

fn unittest(words: &[String], b: &mut Budget) -> Checked<bool> {
    let discovery = match words.first() {
        Some(word) => b.eq(word, "discover")?,
        None => false,
    };
    let mut index = usize::from(discovery);
    while let Some(word) = words.get(index) {
        b.charge(3)?;
        if word.is_empty() {
            return Ok(false);
        }
        if !word.starts_with('-') {
            if discovery || b.eq(word, "discover")? || !unittest_selector(word, b)? {
                return Ok(false);
            }
        } else if !b.one_of(
            word,
            &[
                "-v",
                "--verbose",
                "-q",
                "--quiet",
                "-f",
                "--failfast",
                "-b",
                "--buffer",
                "-c",
                "--catch",
            ],
        )? {
            scan(b, word, 1, 2)?;
            let (option, assigned) = word
                .split_once('=')
                .map_or((word.as_str(), None), |(k, v)| (k, Some(v)));
            let allowed = b.eq(option, "-k")?
                || (discovery
                    && b.one_of(
                        option,
                        &[
                            "-s",
                            "--start-directory",
                            "-p",
                            "--pattern",
                            "-t",
                            "--top-level-directory",
                        ],
                    )?);
            if !allowed
                || (assigned.is_some() && !option.starts_with("--"))
                || option_value(words, &mut index, assigned, b)?.is_none()
            {
                return Ok(false);
            }
        }
        index += 1;
    }
    Ok(true)
}

fn unittest_selector(word: &str, b: &mut Budget) -> Checked<bool> {
    scan(b, word, 3, 4)?; // suffix, component splitting, ID byte inspections
    if word.ends_with(".py") {
        return Ok(true);
    }
    Ok(word.split('.').all(|id| {
        let mut bytes = id.bytes();
        bytes
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
            && bytes.all(|c| c.is_ascii_alphanumeric() || c == b'_')
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MAX_WORK;

    #[test]
    fn scans_refuse_before_exhaustion_and_checked_arithmetic_never_wraps() {
        for words in [
            vec![
                ".venv/bin/python",
                "-m",
                "pytest",
                "a.py::test[x::y]",
                "--maxfail=12",
            ],
            vec!["python", "-m", "unittest", "discover", "--pattern=test*.py"],
            vec!["python", "-m", "unittest", "pkg.Case.test"],
            vec!["pytest", "--tb=unknown"],
            vec!["python", "-m", "missing"],
            vec!["python", "-c"],
            vec!["other"],
            vec!["pytest", "a.py::test_one[param]::"],
            vec!["pytest", "a.py::test_one[param]::::other"],
            vec!["pytest", "a.py::test_one[a::b]::other[c::d]"],
        ] {
            let words: Vec<_> = words.into_iter().map(String::from).collect();
            let mut measured = Budget::default();
            let expected = recognize(&words, &mut measured).unwrap();
            let mut exact = Budget {
                work: MAX_WORK - measured.work,
                ..Default::default()
            };
            assert_eq!(recognize(&words, &mut exact), Ok(expected));
            assert_eq!(exact.work, MAX_WORK);
            let mut short = Budget {
                work: MAX_WORK - measured.work + 1,
                ..Default::default()
            };
            assert_eq!(recognize(&words, &mut short), Err(Reason::WorkLimit));
            assert!(short.work <= MAX_WORK);
        }
        let mut b = Budget::default();
        assert_eq!(scan(&mut b, "xx", usize::MAX, 1), Err(Reason::WorkLimit));
        assert_eq!(b.work, 0);
    }
}
