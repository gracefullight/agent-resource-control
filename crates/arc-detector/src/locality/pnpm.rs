use crate::{
    budget::{Budget, Checked},
    Classification, EndpointContext, EndpointKind, Reason,
};

pub(super) fn recognize_pnpm(
    words: &[String],
    context: &EndpointContext,
    budget: &mut Budget,
) -> Checked<Classification> {
    if let [_, run, name] = words {
        if budget.eq(run, "run")? {
            for fact in &context.facts {
                budget.charge(1)?;
                if fact.kind == EndpointKind::PnpmScript && budget.eq(&fact.selector, name)? {
                    if let [case] | [case, _] = fact.endpoints.as_slice() {
                        if budget.eq(case, "cargo-build-gh-workflow-ci-v1")?
                            && fact.endpoints.get(1).is_none_or(|digest| {
                                digest.len() == 64
                                    && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                            })
                        {
                            let gh = ["gh", "workflow", "run", "ci.yml"].map(str::to_owned);
                            let remote = crate::remote_ci::recognize(&gh, context, budget)?;
                            if remote.is_some_and(|item| {
                                item.classification == crate::JobClass::RemoteCi
                            }) {
                                return Ok(Classification::new(
                                    crate::JobClass::Mixed,
                                    Reason::CompoundMixed,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(Classification::unknown(Reason::UnresolvedScript))
}
