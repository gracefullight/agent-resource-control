//! Finite native argv only. The literal-command parser deliberately does not use this module.
//! Recognition never inspects paths, projects, environment or installed tools.
use crate::{
    budget::{Budget, Checked},
    Classification, JobClass, Locality, Reason, Uncertainty,
};

mod cmake;
mod go;
mod swift;

const fn local(classification: JobClass, reason: Reason) -> Classification {
    Classification {
        classification,
        locality: Locality::HostLocal,
        reason,
        uncertainty: Uncertainty::None,
    }
}

/// The entire additive vocabulary; also consumed by the Rust-derived wire exporter.
pub const CLASSIFICATIONS: [Classification; 7] = [
    local(JobClass::Build, Reason::GoBuild),
    local(JobClass::Test, Reason::GoTest),
    local(JobClass::Lint, Reason::GoVet),
    local(JobClass::Build, Reason::SwiftBuild),
    local(JobClass::Test, Reason::SwiftTest),
    local(JobClass::Build, Reason::CmakeBuild),
    local(JobClass::Test, Reason::Ctest),
];

/// Charge a bounded lexical scan before it runs; arithmetic overflow is refusal.
pub(super) fn scan(b: &mut Budget, text: &str, passes: usize, fixed: usize) -> Checked<()> {
    b.charge(
        text.len()
            .checked_mul(passes)
            .and_then(|n| n.checked_add(fixed))
            .ok_or(Reason::WorkLimit)?,
    )
}

pub(crate) fn recognize(words: &[String], b: &mut Budget) -> Checked<Option<Classification>> {
    let Some(head) = words.first() else {
        return Ok(None);
    };
    scan(b, head, 1, 1)?;
    let basename = head.rsplit('/').next().unwrap_or("");
    let result = if b.eq(basename, "go")? {
        go::recognize(words, b)?
    } else if b.eq(basename, "swift")? {
        swift::recognize(words, b)?
    } else if b.one_of(basename, &["cmake", "ctest"])? {
        cmake::recognize(words, b)?
    } else {
        return Ok(None);
    };
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexical_scans_refuse_before_work_or_arithmetic_overflow() {
        let mut budget = Budget {
            work: crate::MAX_WORK - 3,
            ..Default::default()
        };
        assert_eq!(scan(&mut budget, "abc", 1, 1), Err(Reason::WorkLimit));
        assert_eq!(budget.work, crate::MAX_WORK - 3);
        assert_eq!(
            scan(&mut budget, "xx", usize::MAX, 0),
            Err(Reason::WorkLimit)
        );
        assert_eq!(scan(&mut budget, "ab", 1, 1), Ok(()));
        assert_eq!(budget.work, crate::MAX_WORK);
    }
}
