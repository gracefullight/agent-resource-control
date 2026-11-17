use super::*;

// Validate only the explicit object/type/required/enum rules in the published
// schema projection; this is not Cursor's parser or approval implementation.
fn documented_response(schema: &Value, value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let Some(required) = schema["required"].as_array() else {
        return false;
    };
    if !required.iter().all(|field| {
        field
            .as_str()
            .is_some_and(|field| object.contains_key(field))
    }) {
        return false;
    }
    let Some(properties) = schema["properties"].as_object() else {
        return false;
    };
    properties.iter().all(|(name, rule)| {
        object.get(name).is_none_or(|actual| {
            let valid_type = match rule["type"].as_str() {
                Some("string") => actual.is_string(),
                Some("object") => actual.is_object(),
                _ => false,
            };
            valid_type
                && rule["enum"]
                    .as_array()
                    .is_none_or(|allowed| allowed.contains(actual))
        })
    })
}

pub(super) fn schema_candidates(root: &Path, bin: &Path, arcctl: &Path) {
    let context = HookContext {
        executable: Some(arcctl.to_path_buf()),
    };
    let surface = VerifiedSurface {
        command_path: bin.as_os_str(),
    };
    let schema: Value =
        serde_json::from_str(include_str!("pre-tool-use-response-schema.json")).unwrap();
    let candidate = cursor::handle_verified(&event(), &context, Some(&surface)).unwrap();
    assert!(candidate.get("permission").is_none());
    assert!(candidate["updated_input"].is_object());
    // The frozen fixture-only transformation does not satisfy this projection.
    // Production must not send it until the real runtime and approval are proven.
    assert!(!documented_response(&schema, &candidate));
    assert!(documented_response(
        &schema,
        &json!({"permission": "ask", "updated_input": candidate["updated_input"]})
    ));
    assert!(documented_response(
        &schema,
        &json!({"permission": "allow"})
    ));
    assert!(documented_response(&schema, &json!({"permission": "deny"})));
    assert!(!documented_response(
        &schema,
        &json!({"updated_input": {"command": "cargo build"}})
    ));
    assert!(!documented_response(
        &schema,
        &json!({"permission": "unknown"})
    ));
    assert!(!documented_response(
        &schema,
        &json!({"permission": "ask", "updated_input": "invalid"})
    ));
    // No fixture verdict is sent by the production entry: the real approval
    // and denial paths cannot be established by this schema check.
    assert!(cursor::handle(&event(), &context).is_none());
    assert!(!root.join("starts").exists());
}

#[test]
fn provisional_fixture_and_documented_schema_projection_remain_distinct() {
    Fixture::new().worker("schema-candidates", &[]);
}

// This simulated host accepts the frozen provisional transformation and has
// its own explicit approval decision. It is not Cursor's parser or policy.
fn fixture_host_decision(
    approved: bool,
    input: &Value,
    responses: &[&Value],
    root: &Path,
    bin: &Path,
) -> Option<std::process::Output> {
    let schema: Value =
        serde_json::from_str(include_str!("pre-tool-use-response-schema.json")).unwrap();
    if responses.iter().any(|response| {
        !(documented_response(&schema, response)
            || response.as_object().is_some_and(|object| object.len() == 1)
                && response.get("updated_input").is_some_and(Value::is_object))
    }) || !approved
        || responses
            .iter()
            .any(|response| response.get("permission").and_then(Value::as_str) == Some("deny"))
    {
        return None;
    }
    let selected = responses
        .iter()
        .rev()
        .find_map(|response| response.get("updated_input"))
        .unwrap_or(input);
    Some(shell(selected["command"].as_str().unwrap(), root, bin))
}

pub(super) fn approval_decisions(root: &Path, bin: &Path, arcctl: &Path) {
    let context = HookContext {
        executable: Some(arcctl.to_path_buf()),
    };
    let surface = VerifiedSurface {
        command_path: bin.as_os_str(),
    };
    let marker = root.join("starts");
    let mut starts = 0;
    for literal in ["cargo build", "bun run build"] {
        let mut input = event();
        input["tool_input"]["command"] = json!(literal);
        let candidate = cursor::handle_verified(&input, &context, Some(&surface)).unwrap();
        assert!(candidate.get("permission").is_none());
        assert!(cursor::handle(&input, &context).is_none());
        // Exercise both the provisional fixture and actual no-change output.
        for update in [Some(&candidate), None] {
            let updates: Vec<_> = update.into_iter().collect();
            let count = || {
                fs::read_to_string(&marker)
                    .map(|contents| contents.lines().count())
                    .unwrap_or(0)
            };
            assert_eq!(count(), starts, "hook must not start a command");
            assert!(
                fixture_host_decision(false, &input["tool_input"], &updates, root, bin).is_none()
            );
            let invalid = json!({"updated_input": "invalid"});
            assert!(
                fixture_host_decision(true, &input["tool_input"], &[&invalid], root, bin).is_none()
            );
            let deny = json!({"permission": "deny", "user_message": "fixture policy"});
            for responses in [
                [&[&deny][..], &updates[..]].concat(),
                [&updates[..], &[&deny][..]].concat(),
            ] {
                assert!(
                    fixture_host_decision(true, &input["tool_input"], &responses, root, bin)
                        .is_none()
                );
            }
            assert_eq!(count(), starts, "denial must not start a command");
            let output =
                fixture_host_decision(true, &input["tool_input"], &updates, root, bin).unwrap();
            starts += 1;
            assert_eq!(count(), starts, "approval starts one child");
            assert_eq!(output.status.code(), Some(17));
            assert_eq!(output.stderr, b"tool-stderr");
            let stdout = String::from_utf8(output.stdout).unwrap();
            let cwd = fs::canonicalize(root).unwrap();
            assert!(
                stdout.starts_with(&format!("{}\npreserved value\n", cwd.display())),
                "unexpected child stdout: {stdout:?}"
            );
        }
    }
}

#[test]
fn simulated_approval_starts_once_and_denial_never_starts() {
    Fixture::new().worker("approval-decisions", &[]);
}
