use super::*;
use crate::budget::Budget;

#[test]
fn bun_native_argv_is_conservative_and_shell_protocol_is_unchanged() {
    use std::ffi::OsString;
    for (args, eligible) in [
        (vec!["bun", "test"], true),
        (vec!["bun", "test", "watcher.test.ts"], true),
        (vec!["bun", "test", "--test-name-pattern=watch"], true),
        (vec!["bun", "test", "-t", "hot", "--timeout", "2000"], true),
        (
            vec!["bun", "test", "--coverage", "--coverage-dir=out"],
            true,
        ),
        (vec!["bun", "test", "--watch"], false),
        (vec!["bun", "test", "--hot"], false),
        (vec!["bun", "test", "--watch=false"], false),
        (vec!["bun", "test", "--hot=true"], false),
        (vec!["bun", "test", "-w"], false),
        (vec!["bun", "test", "--test-name-pattern", "--watch"], false),
        (vec!["bun", "test", "--test-name-pattern"], false),
        (vec!["bun", "test", "--timeout=invalid"], false),
        (vec!["bun", "test", "--coverage-reporter=bogus"], false),
        (vec!["bun", "test", "--reporter=dots"], true),
        (vec!["bun", "test", "--reporter=invalid"], false),
        (vec!["bun", "test", "--reporter=junit"], false),
        (
            vec![
                "bun",
                "test",
                "--reporter=junit",
                "--reporter-outfile=x.xml",
            ],
            true,
        ),
        (vec!["bun", "test", "-t=watch"], false),
        (vec!["bun", "test", "--shard=1/2"], true),
        (vec!["bun", "test", "--shard=2/1"], false),
        (vec!["bun", "test", "--", "--watch"], false),
        (vec!["bun", "test", "--mystery", "x"], false),
        (vec!["bun", "run", "test"], false),
        (vec!["/usr/bin/bun", "test"], false),
        (vec!["ssh", "build.example", "bun test"], false),
    ] {
        let native: Vec<_> = args.iter().map(OsString::from).collect();
        let (actual, stats) = analyze_argv(&native);
        assert_eq!(
            actual.reason == Reason::BunTest,
            eligible,
            "{args:?}: {actual:?}"
        );
        assert!(actual.is_consistent() && stats.work <= MAX_WORK);
    }
    assert_eq!(
        classify(&request("bun test")).reason,
        Reason::UnresolvedCommand
    );
    assert_eq!(
        classify(&request("bun test && cargo test")).reason,
        Reason::UnresolvedCommand
    );
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let input = vec![
            OsString::from("bun"),
            OsString::from("test"),
            OsString::from_vec(vec![0xff]),
        ];
        assert_eq!(classify_argv(&input).reason, Reason::UnsupportedSyntax);
    }
}

#[path = "../tests/support/contract.rs"]
mod contract_fixtures;

fn request(command: &str) -> ClassifyRequest {
    ClassifyRequest {
        command: command.into(),
        shell_kind: "sh".into(),
    }
}

#[test]
fn delivered_contract_matrix() {
    for case in contract_fixtures::fixtures() {
        let input = serde_json::from_value(case["payload"].clone()).unwrap();
        let expected: Classification = serde_json::from_value(case["result"].clone()).unwrap();
        let (actual, stats) = analyze(&input);
        assert_eq!(actual, expected, "case {} stats {stats:?}", case["id"]);
        assert_eq!(classify(&input), actual, "determinism {}", case["id"]);
        assert!(actual.is_consistent());
        if actual.reason == Reason::InputLimit {
            assert_eq!(stats, AnalysisStats::default(), "{}", case["id"]);
        }
    }
}

#[test]
fn ssh_post_destination_matrix() {
    let mut mismatches = Vec::new();
    for case in contract_fixtures::ssh_fixtures() {
        let input = serde_json::from_value(case["payload"].clone()).unwrap();
        let expected: Classification = serde_json::from_value(case["result"].clone()).unwrap();
        let (actual, stats) = analyze(&input);
        assert_eq!(analyze(&input), (actual, stats), "{}", case["id"]);
        assert!(actual.is_consistent() && stats.work <= MAX_WORK);
        println!(
            "ssh-boundary: {}",
            serde_json::json!({"case":case,"actual":actual,"work":stats.work})
        );
        if actual != expected {
            mismatches.push(format!(
                "{}: expected {expected:?}, got {actual:?}",
                case["id"]
            ));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn ssh_post_destination_work_checks_refuse_before_overrun() {
    for words in [
        vec!["ssh", "build01", "-N"],
        vec!["ssh", "build01", "", "-G"],
        vec!["ssh", "build01", "--"],
        vec!["ssh", "build01", "--", "-N"],
        vec!["ssh", "--", "build01", "-N"],
        vec!["ssh", "build01", "make", "-N"],
    ] {
        let words: Vec<String> = words.into_iter().map(String::from).collect();
        let mut measured = Budget::default();
        let expected = crate::rules::recognize(&words, &mut measured).unwrap();
        let mut exact = Budget {
            work: MAX_WORK - measured.work,
            ..Budget::default()
        };
        assert_eq!(crate::rules::recognize(&words, &mut exact), Ok(expected));
        assert_eq!(exact.work, MAX_WORK);
        let mut short = Budget {
            work: MAX_WORK - measured.work + 1,
            ..Budget::default()
        };
        assert_eq!(
            crate::rules::recognize(&words, &mut short),
            Err(Reason::WorkLimit)
        );
        assert!(short.work <= MAX_WORK);
    }
}

#[test]
fn malformed_payloads_are_not_analysis_requests() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../../.arc-verify/command-classification/contract.json"
    ))
    .unwrap();
    for case in contract["malformedPayloadFixtures"].as_array().unwrap() {
        assert!(
            serde_json::from_value::<ClassifyRequest>(case["payload"].clone()).is_err(),
            "{}",
            case["id"]
        );
    }
}

#[test]
fn input_unicode_tokens_invocations_and_depth_boundaries() {
    let at = format!("cargo test {}é", "x".repeat(MAX_INPUT_BYTES - 13));
    assert_eq!(at.len(), MAX_INPUT_BYTES);
    assert_eq!(classify(&request(&at)).classification, JobClass::Test);
    let (beyond, stats) = analyze(&request(&(at + "x")));
    assert_eq!(beyond.reason, Reason::InputLimit);
    assert_eq!(stats, AnalysisStats::default());
    for (count, expected) in [(1024, Reason::CargoTest), (1025, Reason::TokenLimit)] {
        let text = format!("cargo test{}", " a".repeat(count - 2));
        assert_eq!(classify(&request(&text)).reason, expected);
    }
    for (count, expected) in [(128, Reason::CompoundLocal), (129, Reason::InvocationLimit)] {
        let text = vec!["cargo test"; count].join(";");
        assert_eq!(classify(&request(&text)).reason, expected);
    }
    for (depth, expected) in [(8, Reason::CargoTest), (9, Reason::NestingLimit)] {
        let text = format!("{}cargo test{}", "(".repeat(depth), ")".repeat(depth));
        assert_eq!(classify(&request(&text)).reason, expected);
        let mut shell = "cargo test".to_owned();
        for _ in 0..depth {
            shell = format!(
                "sh -c \"{}\"",
                shell.replace('\\', "\\\\").replace('"', "\\\"")
            );
        }
        assert_eq!(classify(&request(&shell)).reason, expected);
        let mut combined = "(cargo test)".to_owned();
        for _ in 1..depth {
            combined = format!(
                "sh -c \"{}\"",
                combined.replace('\\', "\\\\").replace('"', "\\\"")
            );
        }
        assert_eq!(classify(&request(&combined)).reason, expected);
    }
}

#[test]
fn work_counter_refuses_before_operations_and_never_wraps() {
    let mut b = Budget::default();
    b.charge(MAX_WORK).unwrap();
    let mut performed = false;
    if b.charge(1).is_ok() {
        performed = true;
    }
    assert!(!performed);
    assert_eq!(b.work, MAX_WORK);
    assert_eq!(b.charge(usize::MAX), Err(Reason::WorkLimit));
    assert_eq!(b.work, MAX_WORK);
}

#[test]
fn production_work_exhaustion_trace() {
    let mut text = format!("cargo test {}", "x".repeat(12000));
    // Re-tokenization shares the root budget; no cap is lowered for this fixture.
    for _ in 0..2 {
        text = format!("sh -c '{}'", text.replace('\'', "'\\''"));
    }
    let (result, stats) = analyze(&request(&text));
    assert_eq!(result.reason, Reason::WorkLimit, "{stats:?}");
    assert!(text.len() < MAX_INPUT_BYTES);
    assert!(
        stats.tokens < MAX_TOKENS
            && stats.invocations < MAX_INVOCATIONS
            && stats.max_depth < MAX_DEPTH
    );
    assert!(stats.work <= MAX_WORK);
    assert_eq!(analyze(&request(&text)), (result, stats));
    println!("work-exhaustion: bytes={} stats={stats:?}", text.len());
}

#[test]
fn syntax_quotes_aggregation_and_ssh_negative_matrix() {
    use Reason::*;
    let cases = [
        ("(cargo build && cargo test) && gh run watch", CompoundMixed),
        (
            "sh -c 'cargo build; cargo test'; gh run view",
            CompoundMixed,
        ),
        ("cargo test; gh run view; echo x", UnresolvedCommand),
        ("gh run view; ssh host make", UnresolvedCompound),
        ("cargo test \\; \\| \\$TOKEN \\` \\( \\)", CargoTest),
        ("cargo test \"a\\\"b\" 'a'\"b\"c", CargoTest),
        ("cargo test \\\nabc", CargoTest),
        ("cargo test\r\ngh run view", CompoundMixed),
        ("cargo test\rarg", UnsupportedSyntax),
        ("cargo test;;", UnsupportedSyntax),
        ("cargo test |", UnsupportedSyntax),
        ("cargo test && | gh run view", UnsupportedSyntax),
        ("()", UnsupportedSyntax),
        ("cargo test # gh run view", UnsupportedSyntax),
        ("cargo test & gh run view", UnsupportedSyntax),
        ("if cargo test; then gh run view; fi", UnsupportedSyntax),
        ("cargo test *.rs", UnresolvedExpansion),
        ("cargo test \\\t", CargoTest),
        ("i\\\nf cargo test", UnsupportedSyntax),
        ("\\\ncargo test", CargoTest),
        ("\\\n cargo test", CargoTest),
        ("sh -c '' ; cargo test", EmptyCommand),
        ("cargo test '\r'", UnsupportedSyntax),
        ("'if' cargo test", UnresolvedCommand),
        ("cargo test \"$VAR\"", UnresolvedExpansion),
        ("cargo test '$VAR `thing` *.rs'", CargoTest),
        ("cargo test \"\\$VAR \\`thing\\`\"", CargoTest),
        ("/usr/bin/cargo build", UnresolvedCommand),
        ("env cargo test", UnresolvedCommand),
        ("VAR=x cargo test", UnresolvedCommand),
        ("bash -c 'cargo test' extra", UnresolvedScript),
        (
            "ssh -- user@build01 'cargo test $(remote)'",
            SshRemoteCommand,
        ),
        ("ssh -p 22 host make", SshAmbiguous),
        ("ssh -lDora build01 make", SshAmbiguous),
        ("ssh -oLogLevel=INFO build01 make", SshAmbiguous),
        ("ssh -vlDora build01 make", SshAmbiguous),
        ("ssh -l Dora build01 make", SshAmbiguous),
        ("ssh -o LogLevel=INFO build01 make", SshAmbiguous),
        ("ssh -l N build01 make", SshAmbiguous),
        ("ssh -o '--N' build01 make", SshAmbiguous),
        ("ssh -zN build01 make", SshAmbiguous),
        ("ssh -nNT host", SshForwarding),
        ("ssh -vNL8080:host:80 build01 make", SshForwarding),
        ("ssh -lDora -N build01", SshForwarding),
        ("ssh -R 8080:host:80 host make", SshForwarding),
        ("ssh -D8080 host make", SshForwarding),
        ("ssh -W host:80 jump", SshForwarding),
        ("ssh -- host ''", SshAmbiguous),
        ("ssh user@127.0.0.2 make", SshAmbiguous),
        ("ssh LOCALHOST make", SshAmbiguous),
        ("ssh x.localhost make", SshAmbiguous),
        ("ssh '[::1]' make", SshAmbiguous),
        ("ssh '[2001:db8::1]' make", SshRemoteCommand),
        ("ssh '[::::]' make", SshAmbiguous),
        ("ssh bad@@host make", SshAmbiguous),
        (
            "docker -H tcp://127.0.0.1:1234 build .",
            DockerEndpointUnknown,
        ),
    ];
    for (index, (text, reason)) in cases.iter().enumerate() {
        assert_eq!(classify(&request(text)).reason, *reason, "negative-{index}");
    }
}

#[test]
fn ipv6_destination_numeric_validation_and_loopback_conservatism() {
    for (host, remote) in [
        ("[::]", false),
        ("[::1]", false),
        ("[0:0:0:0:0:0:0:1]", false),
        ("[0:0:0:0:0:0:0:0]", false),
        ("[::0001]", false),
        ("[::2]", true),
        ("[1::]", true),
        ("[2001:db8::1234]", true),
        ("[1:2:3:4:5:6:7:8]", true),
        ("[1:2:3:4:5:6:7:8:9]", false),
        ("[1:2:3:4]", false),
        ("[1::2::3]", false),
        ("[1:::]", false),
        ("[:1]", false),
        ("[1:]", false),
        ("[12345::]", false),
        ("[1.2.3.4]", false),
        ("[::g]", false),
        ("[]", false),
    ] {
        let result = classify(&request(&format!("ssh '{host}' make")));
        assert_eq!(
            result.classification == JobClass::RemoteExec,
            remote,
            "IPv6 fixture"
        );
    }
}

#[test]
fn bounded_deterministic_arbitrary_unicode_inputs_never_panic() {
    // Fixed-seed generated syntax fragments; no shell, paths, endpoints or RNG I/O.
    let alphabet = [
        "cargo", "test", " ", "\t", "\n", "\r", "'", "\"", "\\", "&&", "|", "(", ")", "$", "`",
        "*", "é", "λ", "검", "\0", "#", ";", "gh", "run", "view",
    ];
    let mut state = 0x6a09e667f3bcc909u64;
    for _ in 0..4096 {
        let mut text = String::new();
        for _ in 0..32 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            text.push_str(alphabet[(state as usize) % alphabet.len()]);
        }
        let input = request(&text);
        let (result, stats) = analyze(&input);
        assert!(result.is_consistent());
        assert!(
            stats.work <= MAX_WORK
                && stats.tokens <= MAX_TOKENS
                && stats.invocations <= MAX_INVOCATIONS
                && stats.max_depth <= MAX_DEPTH
        );
        assert_eq!(analyze(&input), (result, stats));
    }
}
