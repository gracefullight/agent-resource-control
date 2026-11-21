//! One bounded capture contract shared by execution and shell inspection.
pub(crate) fn capture_argv(argv: &[std::ffi::OsString]) -> Option<arc_detector::EndpointContext> {
    if let [program, option, source] = argv {
        if program == "sh" && option == "-c" {
            return capture_shell(&arc_detector::ClassifyRequest {
                command: source.to_str()?.to_owned(),
                shell_kind: "sh".into(),
            });
        }
        if matches!(program.to_str(), Some("bash" | "zsh")) && option == "-c" {
            let shell = program.to_str()?;
            let fact = super::proof::bounded_shell_startup(shell)?;
            let mut context = capture_shell(&arc_detector::ClassifyRequest {
                command: source.to_str()?.to_owned(),
                shell_kind: shell.to_owned(),
            })?;
            add_shell_fact(&mut context, fact);
            return context
                .is_valid_at(arc_ipc::locality::now_ms()?)
                .then_some(context);
        }
    }
    let mut context = super::endpoint_context::capture(argv)?;
    super::ci_context::extend(argv, &mut context)?;
    super::endpoint_context::finish(argv, &mut context)?;
    if matches!(argv, [program, target] if program == "make" && target == "test") {
        let remaining = arc_detector::MAX_CONTEXT_AGE_MS
            .checked_sub(arc_ipc::locality::now_ms()?.checked_sub(context.captured_at_ms)?)?;
        if let Some(Some(fact)) = super::endpoint_context::bounded(
            std::time::Duration::from_millis(remaining),
            super::proof::makefile_fact,
        ) {
            context.facts.push(fact);
        }
    }
    // Capture independently verified facts for the finite literal pnpm case.
    if matches!(argv, [program, run, _] if program == "pnpm" && run == "run") {
        let age = arc_ipc::locality::now_ms()?.checked_sub(context.captured_at_ms)?;
        let remaining = arc_detector::MAX_CONTEXT_AGE_MS.checked_sub(age)?;
        let words = argv.to_vec();
        let base = context.clone();
        if let Some(Some(script_context)) = super::endpoint_context::bounded(
            std::time::Duration::from_millis(remaining),
            move || pnpm_literal_context(&words, base),
        ) {
            context = script_context;
        }
    }
    let context = context
        .is_valid_at(arc_ipc::locality::now_ms()?)
        .then_some(context)?;
    observe_fixture("argv", Some(&context));
    Some(context)
}

fn pnpm_literal_context(
    argv: &[std::ffi::OsString],
    mut merged: arc_detector::EndpointContext,
) -> Option<arc_detector::EndpointContext> {
    use arc_detector::scripts::{MetadataStats, PackageMetadata};

    let name = argv.get(2)?.to_str()?;
    if name.is_empty() || name.starts_with('-') {
        return None;
    }
    let cwd = std::env::current_dir().ok()?.canonicalize().ok()?;
    let mut metadata = super::metadata::FreshMetadata::default();
    let mut stats = MetadataStats::default();
    let package = metadata.package(&cwd, &mut stats).ok()?;
    // The leaves run in the package directory; caller-directory facts must
    // never be applied to a package discovered in a parent directory.
    if package.package_dir != cwd
        || package.scripts.contains_key(&format!("pre{name}"))
        || package.scripts.contains_key(&format!("post{name}"))
    {
        return None;
    }
    let request = arc_detector::ClassifyRequest {
        command: package.scripts.get(name)?.clone(),
        shell_kind: "sh".into(),
    };
    let leaves = arc_detector::capture_shell_argv(&request)?;
    if leaves.is_empty() {
        return None;
    }
    for leaf in leaves {
        // Do not recurse into nested package scripts. Every leaf has to prove
        // its own endpoint facts before anything is combined for the caller.
        if leaf.first().is_some_and(|word| word == "pnpm") {
            return None;
        }
        let context = capture_argv(&leaf)?;
        merge_invocation(&mut merged, &leaf, context, arc_ipc::locality::now_ms()?)?;
    }
    metadata.revalidate(&[package], &mut stats).ok()?;
    if let Some(fact) = super::proof::pnpm_fact(name) {
        merged.facts.push(fact);
    }
    merged
        .is_valid_at(arc_ipc::locality::now_ms()?)
        .then_some(merged)
}

pub(crate) fn capture_shell(
    request: &arc_detector::ClassifyRequest,
) -> Option<arc_detector::EndpointContext> {
    let shell_fact = if matches!(request.shell_kind.as_str(), "bash" | "zsh") {
        Some(super::proof::bounded_shell_startup(&request.shell_kind)?)
    } else {
        None
    };
    let commands = arc_detector::capture_shell_argv(request)?;
    let single_leaf = commands.len() == 1;
    let mut merged = arc_detector::EndpointContext {
        captured_at_ms: arc_ipc::locality::now_ms()?,
        ..Default::default()
    };
    for argv in commands {
        let mut context = capture_argv(&argv)?;
        // A single leaf cannot borrow facts from another invocation. Preserve
        // its valid context even when the result is UNKNOWN so shell and argv
        // classification have the same evidence and uncertainty reason.
        if single_leaf {
            if let Some(fact) = shell_fact {
                add_shell_fact(&mut context, fact);
            }
            observe_fixture("shell", Some(&context));
            return Some(context);
        }
        if merge_invocation(&mut merged, &argv, context, arc_ipc::locality::now_ms()?).is_none() {
            observe_fixture("shell", None);
            return None;
        }
    }
    if let Some(fact) = shell_fact {
        add_shell_fact(&mut merged, fact);
    }
    let merged = merged
        .is_valid_at(arc_ipc::locality::now_ms()?)
        .then_some(merged)?;
    observe_fixture("shell", Some(&merged));
    Some(merged)
}

fn add_shell_fact(context: &mut arc_detector::EndpointContext, fact: arc_detector::EndpointFact) {
    if !context
        .facts
        .iter()
        .any(|existing| existing.kind == fact.kind && existing.selector == fact.selector)
    {
        context.facts.push(fact);
    }
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
