//! Real offline SwiftPM, shared class-aware barriers and the registered read callback.
use super::{cargo_peer, pair, Case, Form, Read, ORDERED_PAIRS};
mod evidence;
mod forwarding;
mod process;
mod projects;
mod semantics;
mod streams;
mod tools;

pub fn semantics(read: Read) {
    semantics::managed(read);
}
pub fn pairs(read: Read) {
    for descriptor in ORDERED_PAIRS.into_iter().filter(|pair| {
        matches!(pair.first, Form::SwiftBuild | Form::SwiftTest)
            || matches!(pair.second, Form::SwiftBuild | Form::SwiftTest)
    }) {
        let tools = tools::Tools::installed();
        let receipt = tools.receipt();
        let observers = tools.observers.clone();
        let case = Case::with_tools(descriptor.id, read, false, tools, receipt, observers);
        let prepare = |name, form| {
            if matches!(form, Form::SwiftBuild | Form::SwiftTest) {
                projects::prepare(&case.artifacts, &case.tools, name, form, true, false)
            } else {
                cargo_peer(&case, name, form, &case.tools.cargo)
            }
        };
        let first = prepare("first", descriptor.first);
        let second = prepare("second", descriptor.second);
        pair(case, descriptor, first, second);
    }
}

pub(super) fn assert_completed(project: &std::path::Path) {
    // Check the complete native body before the case removes its own tree.
    assert_eq!(evidence::body_count(project), 1);
    projects::record(project);
}

/// Separate synthetic byte-forwarding proof; real SwiftPM scenarios remain above.
pub fn stream_forwarding(read: Read) {
    forwarding::check(read);
}

/// Lane readiness is independent of product admission/diagnostic wiring and binaries.
#[test]
fn swift_offline_native_readiness() {
    semantics::offline();
}
