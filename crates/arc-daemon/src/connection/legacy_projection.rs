// Lifecycle responses are the old protocol surface. Do not change the stored
// classification that admission, duplicate detection and recovery consume.
pub(super) fn project_legacy_snapshot(value: &mut serde_json::Value) {
    if let Some(classification) = value
        .get_mut("job")
        .and_then(|job| job.get_mut("classification"))
    {
        if let Ok(typed) = serde_json::from_value::<arc_ipc::Classification>(classification.clone())
        {
            *classification = serde_json::json!(typed.legacy());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_script_class_projects_without_mutating_the_source_snapshot() {
        for (class, reason) in [
            ("BUILD", "BUN_SCRIPT_BUILD"),
            ("TEST", "BUN_SCRIPT_TEST"),
            ("LINT", "BUN_SCRIPT_LINT"),
            ("TYPECHECK", "BUN_SCRIPT_TYPECHECK"),
            ("TEST", "BUN_TEST"),
        ] {
            let stored = serde_json::json!({"job":{"classification":{"classification":class,
                "locality":"HOST_LOCAL","reason":reason,"uncertainty":"NONE"}}});
            let mut reply = stored.clone();
            project_legacy_snapshot(&mut reply);
            assert_eq!(stored["job"]["classification"]["reason"], reason);
            assert_eq!(
                reply["job"]["classification"],
                serde_json::json!({"classification":"UNKNOWN",
                "locality":"UNKNOWN","reason":"UNRESOLVED_COMMAND","uncertainty":"OBSERVE"})
            );
        }
        let old = serde_json::json!({"job":{"classification":{"classification":"BUILD",
            "locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"}}});
        let mut reply = old.clone();
        project_legacy_snapshot(&mut reply);
        assert_eq!(reply, old);
    }
}
