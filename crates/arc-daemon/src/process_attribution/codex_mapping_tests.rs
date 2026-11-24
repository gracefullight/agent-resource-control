use super::*;

#[test]
fn codex_executable_mapping_is_exact_and_keeps_legacy_agents() {
    assert_eq!(vendor_for_executable("codex"), Some(Vendor::Codex));
    assert_eq!(vendor_for_executable("claude"), Some(Vendor::ClaudeCode));
    assert_eq!(vendor_for_executable("cursor-agent"), Some(Vendor::Cursor));
    for name in ["Codex", "codex-helper", "node", ""] {
        assert_eq!(vendor_for_executable(name), None);
    }
}
