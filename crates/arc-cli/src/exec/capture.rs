//! One bounded capture contract shared by execution and shell inspection.
pub(crate) fn capture_argv(argv: &[std::ffi::OsString]) -> Option<arc_detector::EndpointContext> {
    let mut context = super::endpoint_context::capture(argv)?;
    super::ci_context::extend(argv, &mut context)?;
    super::endpoint_context::finish(argv, &mut context)?;
    let context = context
        .is_valid_at(arc_ipc::locality::now_ms()?)
        .then_some(context)?;
    observe_fixture("argv", Some(&context));
    Some(context)
}

#[allow(dead_code)] // Consumed by the next-wave classify route.
pub(crate) fn capture_shell(
    request: &arc_detector::ClassifyRequest,
) -> Option<arc_detector::EndpointContext> {
    let commands = arc_detector::capture_shell_argv(request)?;
    let single_leaf = commands.len() == 1;
    let mut merged = arc_detector::EndpointContext {
        captured_at_ms: arc_ipc::locality::now_ms()?,
        ..Default::default()
    };
    for argv in commands {
        let context = capture_argv(&argv)?;
        // A single leaf cannot borrow facts from another invocation. Preserve
        // its valid context even when the result is UNKNOWN so shell and argv
        // classification have the same evidence and uncertainty reason.
        if single_leaf {
            observe_fixture("shell", Some(&context));
            return Some(context);
        }
        if merge_invocation(&mut merged, &argv, context, arc_ipc::locality::now_ms()?).is_none() {
            observe_fixture("shell", None);
            return None;
        }
    }
    let merged = merged
        .is_valid_at(arc_ipc::locality::now_ms()?)
        .then_some(merged)?;
    observe_fixture("shell", Some(&merged));
    Some(merged)
}

// The integration fixture can supply an in-process observer by interposing a
// symbol in debug builds. No endpoint facts are written or exposed by ARC;
// release builds have no observer, and an ordinary debug build has no symbol.
#[cfg(debug_assertions)]
fn observe_fixture(route: &str, context: Option<&arc_detector::EndpointContext>) {
    type Observer = unsafe extern "C" fn(*const u8, usize, *const u8, usize);
    let symbol = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"arc_test_capture_observe".as_ptr()) };
    if !symbol.is_null() {
        if let Ok(bytes) = serde_json::to_vec(&context) {
            let observer: Observer = unsafe { std::mem::transmute(symbol) };
            unsafe { observer(route.as_ptr(), route.len(), bytes.as_ptr(), bytes.len()) };
        }
    }
}

#[cfg(not(debug_assertions))]
fn observe_fixture(_: &str, _: Option<&arc_detector::EndpointContext>) {}

fn merge_invocation(
    merged: &mut arc_detector::EndpointContext,
    argv: &[std::ffi::OsString],
    context: arc_detector::EndpointContext,
    now: u64,
) -> Option<()> {
    // A fact belonging to another leaf must not fill a missing fact for this
    // invocation when the detector later evaluates the combined shell.
    if arc_detector::classify_argv_with_context(argv, &context, now).classification
        == arc_detector::JobClass::Unknown
    {
        return None;
    }
    merged.captured_at_ms = merged.captured_at_ms.min(context.captured_at_ms);
    for (key, value) in context.environment {
        if merged
            .environment
            .get(&key)
            .is_some_and(|old| old != &value)
        {
            return None;
        }
        merged.environment.insert(key, value);
    }
    for fact in context.facts {
        if let Some(existing) = merged
            .facts
            .iter()
            .find(|old| old.kind == fact.kind && old.selector == fact.selector)
        {
            if existing.endpoints != fact.endpoints {
                return None;
            }
        } else {
            merged.facts.push(fact);
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shell_merge_never_lends_one_invocation_facts_to_an_unproved_one() {
        let argv = ["gh", "workflow", "run", "build.yml"].map(std::ffi::OsString::from);
        let proven: arc_detector::EndpointContext = serde_json::from_value(serde_json::json!({
            "captured_at_ms":10000,
            "facts":[{"kind":"CI_HOST","selector":"gh","endpoints":["https://github.com"]}]
        }))
        .unwrap();
        let missing = arc_detector::EndpointContext {
            captured_at_ms: 10000,
            ..Default::default()
        };
        let mut merged = missing.clone();
        assert!(merge_invocation(&mut merged, &argv, proven, 10000).is_some());
        assert!(merge_invocation(&mut merged, &argv, missing, 10000).is_none());
    }

    #[test]
    fn unknown_kubernetes_leaf_never_contributes_to_a_multi_leaf_merge() {
        let argv = ["kubectl", "exec", "pod", "--", "make"].map(std::ffi::OsString::from);
        let context: arc_detector::EndpointContext = serde_json::from_value(serde_json::json!({
            "captured_at_ms":10000,
            "facts":[{"kind":"KUBERNETES_CONTEXT","selector":"@current","endpoints":["https://192.0.2.41:6443"]}]
        }))
        .unwrap();
        let mut merged = arc_detector::EndpointContext {
            captured_at_ms: 10000,
            ..Default::default()
        };
        assert!(merge_invocation(&mut merged, &argv, context, 10000).is_none());
        assert!(merged.facts.is_empty());
    }
}
