// Lifecycle responses are the old protocol surface. Do not change the stored
// classification that admission, duplicate detection and recovery consume.
pub(super) fn project_legacy_snapshot(value: &mut serde_json::Value) {
    if let Some(classification) = value
        .get_mut("job")
        .and_then(|job| job.get_mut("classification"))
    {
        if classification
            .get("reason")
            .and_then(serde_json::Value::as_str)
            == Some("BUN_TEST")
        {
            *classification = serde_json::json!({
                "classification": "UNKNOWN", "locality": "UNKNOWN",
                "reason": "UNRESOLVED_COMMAND", "uncertainty": "OBSERVE"
            });
        }
    }
}
