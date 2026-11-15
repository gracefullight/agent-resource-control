//! Frozen CMake/CTest argv only; never inspect a build tree or discover targets.
use crate::{
    budget::{Budget, Checked},
    Classification, JobClass, Reason,
};

#[derive(Clone, Copy)]
enum Value {
    Flag,
    Opaque,
    Configuration,
    Parallel,
    Aggregate,
}
struct OptionSpec {
    aliases: &'static [&'static str],
    key: u16,
    value: Value,
}
const fn option(aliases: &'static [&'static str], key: u16, value: Value) -> OptionSpec {
    OptionSpec {
        aliases,
        key,
        value,
    }
}
const CMAKE: &[OptionSpec] = &[
    option(&["--clean-first"], 1, Value::Flag),
    option(&["-v", "--verbose"], 2, Value::Flag),
    option(&["--config"], 4, Value::Configuration),
    option(&["-j", "--parallel"], 8, Value::Parallel),
    option(&["-t", "--target"], 16, Value::Aggregate),
];
const CTEST: &[OptionSpec] = &[
    option(&["--output-on-failure"], 1, Value::Flag),
    option(&["--stop-on-failure"], 2, Value::Flag),
    // All verbosity choices, including aliases, occupy the same slot.
    option(
        &["-V", "--verbose", "-VV", "--extra-verbose", "-Q", "--quiet"],
        4,
        Value::Flag,
    ),
    option(&["--test-dir"], 8, Value::Opaque),
    option(&["-C", "--build-config"], 16, Value::Configuration),
    option(&["-j", "--parallel"], 32, Value::Parallel),
    option(&["-R", "--tests-regex"], 64, Value::Opaque),
    option(&["-E", "--exclude-regex"], 128, Value::Opaque),
    // Label selectors alone may repeat. No regex evaluation occurs here.
    option(
        &["-L", "--label-regex", "-LE", "--label-exclude"],
        0,
        Value::Opaque,
    ),
];

pub(super) fn recognize(words: &[String], b: &mut Budget) -> Checked<Classification> {
    super::scan(b, &words[0], 1, 1)?;
    let cmake = b.eq(words[0].rsplit('/').next().unwrap_or(""), "cmake")?;
    let (args, specs, class, reason) = if cmake {
        if words.len() < 3 || !b.eq(&words[1], "--build")? || !opaque(&words[2], b)? {
            return Ok(Classification::unknown(Reason::UnresolvedCommand));
        }
        (&words[3..], CMAKE, JobClass::Build, Reason::CmakeBuild)
    } else {
        (&words[1..], CTEST, JobClass::Test, Reason::Ctest)
    };
    Ok(if options(args, specs, b)? {
        Classification::new(class, reason)
    } else {
        Classification::unknown(Reason::UnresolvedCommand)
    })
}

fn opaque(word: &str, b: &mut Budget) -> Checked<bool> {
    b.charge(2)?;
    Ok(!word.is_empty() && !word.starts_with('-'))
}

fn valid(word: &str, kind: Value, b: &mut Budget) -> Checked<bool> {
    if !opaque(word, b)? {
        return Ok(false);
    }
    Ok(match kind {
        Value::Flag => unreachable!("flags consume no value"),
        Value::Opaque => true,
        Value::Configuration => {
            b.one_of(word, &["Debug", "Release", "RelWithDebInfo", "MinSizeRel"])?
        }
        Value::Aggregate => b.eq(word, "all")?,
        Value::Parallel => {
            super::scan(b, word, 2, 1)?;
            word.bytes().all(|c| c.is_ascii_digit())
                && word.parse::<u32>().is_ok_and(|n| (1..=1024).contains(&n))
        }
    })
}

fn options(words: &[String], specs: &[OptionSpec], b: &mut Budget) -> Checked<bool> {
    let (mut seen, mut index) = (0_u16, 0);
    while let Some(word) = words.get(index) {
        b.charge(3)?;
        // Only -jN supports attachment. Equals and native build-tool forwarding
        // are deliberately absent, as are custom CMake targets and CTest modes.
        let attached = word.strip_prefix("-j").filter(|n| !n.is_empty());
        let key = if attached.is_some() { "-j" } else { word };
        let mut found = None;
        for spec in specs {
            if b.one_of(key, spec.aliases)? {
                found = Some(spec);
                break;
            }
        }
        let Some(spec) = found else { return Ok(false) };
        if seen & spec.key != 0 {
            return Ok(false);
        }
        seen |= spec.key;
        if !matches!(spec.value, Value::Flag) {
            let value = if let Some(value) = attached {
                value
            } else {
                index += 1;
                let Some(value) = words.get(index) else {
                    return Ok(false);
                };
                value
            };
            if !valid(value, spec.value, b)? {
                return Ok(false);
            }
        }
        index += 1;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_and_option_matching_refuse_before_budget_exhaustion() {
        for argv in [
            vec![
                "cmake",
                "--build",
                "build path",
                "--target",
                "all",
                "-j1024",
            ],
            vec![
                "ctest",
                "--test-dir",
                "build",
                "-C",
                "Release",
                "-LE",
                "slow",
            ],
            vec!["ctest", "-R", "opaque [ regex", "--output-on-failure"],
            vec!["ctest", "--parallel", "999999999999999999999999999999"],
            vec!["cmake", "-P", "script.cmake"],
            vec!["ctest", "-D", "Experimental"],
        ] {
            let words: Vec<_> = argv.into_iter().map(String::from).collect();
            let mut measured = Budget::default();
            let expected = recognize(&words, &mut measured).unwrap();
            let mut exact = Budget {
                work: crate::MAX_WORK - measured.work,
                ..Default::default()
            };
            assert_eq!(recognize(&words, &mut exact), Ok(expected));
            assert_eq!(exact.work, crate::MAX_WORK);
            let mut short = Budget {
                work: crate::MAX_WORK - measured.work + 1,
                ..Default::default()
            };
            assert_eq!(recognize(&words, &mut short), Err(Reason::WorkLimit));
            assert!(short.work <= crate::MAX_WORK);
        }
    }
}
