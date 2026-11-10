//! Reuse the unchanged native OS pipeline controller with an absent daemon.
use super::{cli::binary, runtime::Fixture, support};
#[test]
#[cfg(target_os = "macos")]
fn fallback_foreground_pipeline_input_signals_stop_continue_and_exit17() {
    let _serial = support::serial();
    let f = Fixture::new();
    for mode in ["--normal", "--continuation"] {
        let mut c = std::process::Command::new("python3");
        c.arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/managed_exec/pipeline.py"
        ));
        if mode == "--continuation" {
            c.arg(mode);
        }
        let out = c
            .arg(&f.root)
            .arg(binary("arc-exec-fixture"))
            .arg(binary("arcctl"))
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        for line in String::from_utf8(out.stdout).unwrap().lines() {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(row["pass"], true);
            assert_eq!(row["allRecordedIdentitiesAbsent"], true);
        }
    }
    super::fail_open::receipt(
        "pipeline",
        serde_json::json!({"scenario":"pipeline","passed":true,"invocations":1}),
    );
}
