use crate::{
    budget::Budget, rules, AnalysisStats, Classification, Reason, MAX_INPUT_BYTES, MAX_TOKENS,
};
use std::ffi::OsString;

/// Analyze native arguments directly; no shell interpretation or executable lookup.
pub fn classify_argv(argv: &[OsString]) -> Classification {
    analyze_argv(argv).0
}
pub fn analyze_argv(argv: &[OsString]) -> (Classification, AnalysisStats) {
    let mut budget = Budget::default();
    let result = (|| {
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
            let word = arg.to_str().ok_or(Reason::UnsupportedSyntax)?;
            if word.contains('\0') {
                return Err(Reason::UnsupportedSyntax);
            }
            words.push(word.to_owned());
        }
        if words.is_empty() || words[0].is_empty() {
            return Err(Reason::EmptyCommand);
        }
        budget.invocation()?;
        if Budget::eq(&mut budget, &words[0], "bun")?
            && match words.get(1) {
                Some(word) => Budget::eq(&mut budget, word, "test")?,
                None => false,
            }
        {
            rules::recognize_bun_test(&words[2..], &mut budget)
        } else {
            rules::recognize(&words, &mut budget)
        }
    })();
    (result.unwrap_or_else(Classification::unknown), budget)
}
