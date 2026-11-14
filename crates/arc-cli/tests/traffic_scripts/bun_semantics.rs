//! Pinned Bun argument/lifecycle semantics. These are tiny real interpreter
//! fixtures, not the downstream build/tool/production-daemon acceptance gates.
use crate::support::*;
use serde_json::{json, Value};
use std::{fs, process::Command};

#[test]
fn installed_bun_and_the_cli_agree_on_package_cwd_hooks_and_literal_forwarding() {
    let socket = Socket::new();
    // No coordinator: compare the native Bun invocation to the original argv
    // executed by the CLI. The bodies are intentionally unsupported scripts.
    fs::remove_file(socket.root.join("s")).unwrap();
    let package = socket.root.join("package");
    fs::create_dir(&package).unwrap();
    let subdir = package.join("subdir");
    fs::create_dir(&subdir).unwrap();
    fs::write(
        package.join("package.json"),
        json!({"scripts":{
            "prex":"bun print.ts", "x":"bun print.ts && bun print.ts", "postx":"bun print.ts"
        }})
        .to_string(),
    )
    .unwrap();
    fs::write(package.join("print.ts"), r#"
console.log(JSON.stringify({event: process.env.npm_lifecycle_event, args: process.argv.slice(2), cwd: process.cwd(), value: process.env.ARC_SCRIPT_TEST_VALUE}));
if (process.env.npm_lifecycle_event === "postx") process.exit(17);
"#).unwrap();
    let argv = ["run", "x", "two words", "", "a'b", "$HOME", ";touch poison"];
    let mut baseline = Command::new("bun");
    baseline
        .args(argv)
        .current_dir(&subdir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .env("ARC_SCRIPT_TEST_VALUE", "preserved");
    let baseline = output(baseline);
    assert_eq!(baseline.status.code(), Some(17));
    let mut cli = socket.cli();
    cli.args(["exec", "--", "bun"])
        .args(argv)
        .current_dir(&subdir)
        .env("ARC_SCRIPT_TEST_VALUE", "preserved");
    let result = output(cli);
    assert_eq!(result.status.code(), Some(17));
    assert_eq!(result.stdout, baseline.stdout);
    let records: Vec<Value> = String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        records.iter().map(|r| &r["event"]).collect::<Vec<_>>(),
        vec![&json!("prex"), &json!("x"), &json!("x"), &json!("postx")]
    );
    for record in &records {
        assert_eq!(record["cwd"], json!(package.canonicalize().unwrap()));
        assert_eq!(record["value"], "preserved");
    }
    assert_eq!(records[0]["args"], json!([]));
    assert_eq!(records[1]["args"], json!([]));
    assert_eq!(records[2]["args"], json!(&argv[2..]));
    assert_eq!(records[3]["args"], json!([]));
    assert!(!package.join("poison").exists());

    for (prefix, forwarded) in [
        (
            vec!["run", "--cwd", "package", "x", "--", "--watch"],
            vec!["--watch"],
        ),
        (
            vec!["--cwd=package", "run", "x", "literal", "--", "tail"],
            vec!["literal", "--", "tail"],
        ),
    ] {
        let mut cli = socket.cli();
        cli.args(["exec", "--", "bun"]).args(prefix);
        let result = output(cli);
        assert_eq!(result.status.code(), Some(17));
        let records: Vec<Value> = String::from_utf8(result.stdout)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(records.len(), 4);
        assert_eq!(records[2]["args"], json!(forwarded));
    }
}
