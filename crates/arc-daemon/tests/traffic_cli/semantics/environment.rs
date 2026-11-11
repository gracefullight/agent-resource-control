#[test]
fn real_bun_preserves_caller_environment_and_receives_missing_lineage() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let project = f.root.join("environment_bun");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("environment.test.ts"), r#"
import {test} from 'bun:test';
import {writeFileSync} from 'node:fs';
test('environment', () => writeFileSync(process.env.RESULT!, JSON.stringify(Object.fromEntries(Object.entries(process.env).sort()))));
"#).unwrap();
    let result = project.join("environment.json");
    for lineage in [
        None,
        Some(""),
        Some("caller-owned-not-a-lineage-token"),
        Some("b0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde"),
    ] {
        let configure = |command: &mut Command| {
            command
                .current_dir(&project)
                .env_clear()
                .env("PATH", std::env::var_os("PATH").unwrap())
                .env("HOME", std::env::var_os("HOME").unwrap())
                .env(arc_core::ARC_SOCKET_PATH, &f.socket)
                .env("RESULT", &result)
                .env("CUSTOM_VALUE", "environment-secret-sentinel")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            if let Some(value) = lineage {
                command.env("ARC_TRAFFIC_LINEAGE", value);
            }
            // LLVM's instrumented executable adds this marker before main.
            // Give both commands the same caller environment during coverage.
            for key in ["__LLVM_PROFILE_RT_INIT_ONCE", "LLVM_PROFILE_FILE"] {
                if let Some(value) = std::env::var_os(key) {
                    command.env(key, value);
                }
            }
        };
        let mut direct = Command::new("bun");
        direct.args(["test", "environment.test.ts"]);
        configure(&mut direct);
        let output = direct.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let expected: serde_json::Value =
            serde_json::from_slice(&fs::read(&result).unwrap()).unwrap();
        fs::remove_file(&result).unwrap();
        let mut scheduled = Command::new(binary("arcctl"));
        scheduled.args(["exec", "--", "bun", "test", "environment.test.ts"]);
        configure(&mut scheduled);
        let output = scheduled.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut actual: serde_json::Value =
            serde_json::from_slice(&fs::read(&result).unwrap()).unwrap();
        if lineage.is_none() {
            let delivered = actual["ARC_TRAFFIC_LINEAGE"].as_str().unwrap();
            let context: String = database(&f)
                .query_row(
                    "SELECT context FROM traffic_entries ORDER BY sequence DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(delivered, context);
            actual
                .as_object_mut()
                .unwrap()
                .remove("ARC_TRAFFIC_LINEAGE");
        }
        assert_eq!(actual, expected);
        assert_eq!(active(&f).1, 0);
    }
    let completed: i64 = database(&f)
        .query_row(
            "SELECT count(*) FROM traffic_entries WHERE state='COMPLETED'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(completed, 4);
    daemon.stop(libc::SIGTERM);
    f.report();
}
