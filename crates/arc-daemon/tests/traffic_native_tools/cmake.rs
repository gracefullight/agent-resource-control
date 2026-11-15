//! Real offline CMake/CTest fixtures, shared by direct and registered entrypoints.
//! Implemented/verified with oma-backend; shared production wiring is integration-owned.
use super::*;

#[cfg(test)]
mod direct;
mod execution;
mod project;
mod tools;

use tools::Tools;

fn case(name: &str, read: Read) -> Case<Tools> {
    let tools = Tools::installed();
    let receipt = tools.receipt();
    let observers = tools.observers.clone();
    Case::with_tools(name, read, false, tools, receipt, observers)
}

pub fn semantics(read: Read) {
    execution::semantics(read);
}

pub fn pairs(read: Read) {
    for descriptor in ORDERED_PAIRS.into_iter().filter(|p| {
        matches!(p.first, Form::CmakeBuild | Form::Ctest)
            || matches!(p.second, Form::CmakeBuild | Form::Ctest)
    }) {
        let case = case(descriptor.id, read);
        let prepare = |form, name| {
            if matches!(form, Form::CmakeBuild | Form::Ctest) {
                project::prepare(&case.artifacts.join(name), form, &case.tools, true, false)
            } else {
                cargo_peer(&case, name, form, &case.tools.cargo)
            }
        };
        let first = prepare(descriptor.first, "first project");
        let second = prepare(descriptor.second, "second project");
        pair(case, descriptor, first, second);
    }
}
