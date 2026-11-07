use super::*;

#[test]
fn detector_budgets_are_preserved_through_daemon_and_cli() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = fixture.spawn("");
    let mut stream = fixture.connect();
    let max = format!("cargo test {}é", "x".repeat(16384 - 13));
    let mut cases = vec![
        (max.clone(), "CARGO_TEST"),
        (max + "x", "INPUT_LIMIT"),
        (format!("cargo test{}", " a".repeat(1022)), "CARGO_TEST"),
        (format!("cargo test{}", " a".repeat(1023)), "TOKEN_LIMIT"),
        (vec!["cargo test"; 128].join(";"), "COMPOUND_LOCAL"),
        (vec!["cargo test"; 129].join(";"), "INVOCATION_LIMIT"),
        (
            format!("sh -c 'cargo test {}'", "x".repeat(12000)),
            "WORK_LIMIT",
        ),
    ];
    for depth in [8, 9] {
        let reason = if depth == 8 {
            "CARGO_TEST"
        } else {
            "NESTING_LIMIT"
        };
        cases.push((
            format!("{}cargo test{}", "(".repeat(depth), ")".repeat(depth)),
            reason,
        ));
        let mut nested = "cargo test".to_owned();
        for _ in 0..depth {
            nested = format!(
                "sh -c \"{}\"",
                nested.replace('\\', "\\\\").replace('"', "\\\"")
            );
        }
        cases.push((nested, reason));
    }
    for (index, (text, reason)) in cases.iter().enumerate() {
        let response = exchange(&mut stream, json!({"command":text}));
        assert_eq!(response.status, "ok", "boundary-{index}");
        let expected = response.result.unwrap();
        assert_eq!(expected["reason"], *reason, "boundary-{index}");
        for json in [false, true] {
            let args = if json {
                vec!["classify", "--json", "--", text]
            } else {
                vec!["classify", "--", text]
            };
            let output = cli(&fixture, &args);
            assert!(
                output.status.success() && output.stderr.is_empty(),
                "boundary-cli-{index}"
            );
            if json {
                assert_eq!(
                    serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                    expected
                );
            } else {
                assert_eq!(output.stdout, human(&expected).as_bytes());
            }
        }
    }
    drop(stream);
    child.stop(libc::SIGTERM);
    fixture.report();
}
