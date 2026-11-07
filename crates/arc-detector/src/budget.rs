use crate::Reason;

pub const MAX_INPUT_BYTES: usize = 16384;
pub const MAX_TOKENS: usize = 1024;
pub const MAX_INVOCATIONS: usize = 128;
pub const MAX_DEPTH: usize = 8;
pub const MAX_WORK: usize = 65536;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalysisStats {
    pub work: usize,
    pub tokens: usize,
    pub invocations: usize,
    pub max_depth: usize,
}

pub(crate) type Checked<T> = Result<T, Reason>;
pub(crate) type Budget = AnalysisStats;
impl Budget {
    /// Refuse a charge before its operation; never wrap or commit an overrun.
    pub(crate) fn charge(&mut self, amount: usize) -> Checked<()> {
        let next = self.work.checked_add(amount).ok_or(Reason::WorkLimit)?;
        if next > MAX_WORK {
            return Err(Reason::WorkLimit);
        }
        self.work = next;
        Ok(())
    }
    pub(crate) fn token(&mut self) -> Checked<()> {
        if self.tokens >= MAX_TOKENS {
            return Err(Reason::TokenLimit);
        }
        self.tokens += 1;
        Ok(())
    }
    pub(crate) fn invocation(&mut self) -> Checked<()> {
        if self.invocations >= MAX_INVOCATIONS {
            return Err(Reason::InvocationLimit);
        }
        self.charge(1)?;
        self.invocations += 1;
        Ok(())
    }
    pub(crate) fn depth(&mut self, depth: usize) -> Checked<()> {
        if depth > MAX_DEPTH {
            return Err(Reason::NestingLimit);
        }
        self.max_depth = self.max_depth.max(depth);
        Ok(())
    }
    pub(crate) fn eq(&mut self, a: &str, b: &str) -> Checked<bool> {
        self.charge(1)?;
        if a.len() != b.len() {
            return Ok(false);
        }
        for at in 0..a.len() {
            self.charge(1)?;
            if a.as_bytes()[at] != b.as_bytes()[at] {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub(crate) fn one_of(&mut self, a: &str, choices: &[&str]) -> Checked<bool> {
        for choice in choices {
            if self.eq(a, choice)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
