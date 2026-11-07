//! Literal SSH boundary data shared by detector, typed IPC and real daemon/CLI tests.
//! The first 17 cases retain the independent SSH-POST-DESTINATION probe inputs.
use serde_json::{json, Value};

pub fn fixtures() -> Vec<Value> {
    let mut cases = Vec::new();
    for (id, command, class, locality, reason) in [
        (
            "post-n",
            "ssh build01 -N",
            "UNKNOWN",
            "UNKNOWN",
            "SSH_AMBIGUOUS",
        ),
        (
            "post-g",
            "ssh build01 -G",
            "UNKNOWN",
            "UNKNOWN",
            "SSH_AMBIGUOUS",
        ),
        (
            "post-n-with-command",
            "ssh build01 -N 'make test'",
            "UNKNOWN",
            "UNKNOWN",
            "SSH_AMBIGUOUS",
        ),
        (
            "post-g-with-command",
            "ssh build01 -G 'make test'",
            "UNKNOWN",
            "UNKNOWN",
            "SSH_AMBIGUOUS",
        ),
        (
            "post-l",
            "ssh build01 -L8080:localhost:80",
            "UNKNOWN",
            "UNKNOWN",
            "SSH_AMBIGUOUS",
        ),
        (
            "post-r",
            "ssh build01 -R8080:localhost:80",
            "UNKNOWN",
            "UNKNOWN",
            "SSH_AMBIGUOUS",
        ),
        (
            "post-d",
            "ssh build01 -D8080",
            "UNKNOWN",
            "UNKNOWN",
            "SSH_AMBIGUOUS",
        ),
        (
            "post-w",
            "ssh build01 -W target:80",
            "UNKNOWN",
            "UNKNOWN",
            "SSH_AMBIGUOUS",
        ),
        (
            "post-quoted-n",
            "ssh build01 '-N'",
            "UNKNOWN",
            "UNKNOWN",
            "SSH_AMBIGUOUS",
        ),
        (
            "compound-n",
            "cargo test && ssh build01 -N",
            "UNKNOWN",
            "UNKNOWN",
            "SSH_AMBIGUOUS",
        ),
        (
            "compound-g",
            "cargo test && ssh build01 -G",
            "UNKNOWN",
            "UNKNOWN",
            "SSH_AMBIGUOUS",
        ),
        (
            "pre-n-control",
            "ssh -N build01",
            "UNKNOWN",
            "UNKNOWN",
            "SSH_FORWARDING",
        ),
        (
            "opaque-word-control",
            "ssh build01 make -N",
            "REMOTE_EXEC",
            "REMOTE",
            "SSH_REMOTE_COMMAND",
        ),
        (
            "opaque-quoted-command-control",
            "ssh build01 'make -N'",
            "REMOTE_EXEC",
            "REMOTE",
            "SSH_REMOTE_COMMAND",
        ),
        (
            "terminated-before-destination-control",
            "ssh -- build01 -N",
            "REMOTE_EXEC",
            "REMOTE",
            "SSH_REMOTE_COMMAND",
        ),
        (
            "terminated-after-destination-control",
            "ssh build01 -- -N",
            "REMOTE_EXEC",
            "REMOTE",
            "SSH_REMOTE_COMMAND",
        ),
        (
            "quoted-local-argument-control",
            "cargo test 'ssh build01 -N'",
            "TEST",
            "HOST_LOCAL",
            "CARGO_TEST",
        ),
    ] {
        cases.push(case(id, command, class, locality, reason));
    }
    for (index, command) in [
        "ssh",
        "ssh --",
        "ssh build01",
        "ssh -- build01",
        "ssh build01 ''",
        "ssh -- build01 ''",
        "ssh build01 --",
        "ssh build01 '--'",
        "ssh build01 -- ''",
        "ssh -- build01 --",
        "ssh -- build01 -- ''",
        "ssh build01 '' -N",
        "ssh build01 '' --",
        "ssh build01 '' -- ''",
        "cargo test && ssh build01 --",
        "ssh build01 -",
        "ssh build01 -z",
        "ssh build01 -p 22 make",
        "ssh build01 -oLogLevel=INFO make",
        "ssh -lDora build01 make",
        "ssh -o '--N' build01 make",
        "ssh localhost make",
        "ssh user@127.0.0.2 make",
        "ssh '[::1]' make",
        "ssh -- -host make",
        "ssh bad@@host make",
    ]
    .into_iter()
    .enumerate()
    {
        cases.push(case(
            &format!("empty-or-ambiguous-{index}"),
            command,
            "UNKNOWN",
            "UNKNOWN",
            "SSH_AMBIGUOUS",
        ));
    }
    for (index, command) in [
        "ssh -L8080:host:80 build01 make",
        "ssh -R 8080:host:80 build01 make",
        "ssh -D8080 build01 make",
        "ssh -W host:80 build01 make",
        "ssh -nNT build01",
        "ssh -lDora -N build01",
    ]
    .into_iter()
    .enumerate()
    {
        cases.push(case(
            &format!("forwarding-control-{index}"),
            command,
            "UNKNOWN",
            "UNKNOWN",
            "SSH_FORWARDING",
        ));
    }
    for flag in ["-N", "-G", "-L", "-R", "-D", "-W"] {
        for (form, command) in [
            ("quoted", format!("ssh build01 '{flag}'")),
            ("compound", format!("cargo test && ssh build01 {flag}")),
            (
                "quoted-compound",
                format!("cargo test && ssh build01 \"{flag}\""),
            ),
        ] {
            cases.push(case(
                &format!("{form}-{flag}"),
                &command,
                "UNKNOWN",
                "UNKNOWN",
                "SSH_AMBIGUOUS",
            ));
        }
        for (form, command) in [
            ("before", format!("ssh -- build01 {flag}")),
            ("after", format!("ssh build01 -- {flag}")),
            ("opaque", format!("ssh build01 make {flag}")),
            ("quoted-opaque", format!("ssh build01 'make {flag}'")),
        ] {
            cases.push(case(
                &format!("terminated-or-opaque-{form}-{flag}"),
                &command,
                "REMOTE_EXEC",
                "REMOTE",
                "SSH_REMOTE_COMMAND",
            ));
        }
    }
    for (index, command) in [
        "ssh build01 '--' make -N",
        "ssh -- build01 -- make -N",
        "ssh build01 make --",
        "ssh build01 make '' -G",
        "ssh build01 'make -L8080:host:80'",
        "ssh build01 '' -- -N",
        "ssh -- build01 '' -N",
    ]
    .into_iter()
    .enumerate()
    {
        cases.push(case(
            &format!("opaque-control-{index}"),
            command,
            "REMOTE_EXEC",
            "REMOTE",
            "SSH_REMOTE_COMMAND",
        ));
    }
    cases.push(case(
        "terminated-compound",
        "cargo test && ssh build01 -- -N",
        "MIXED",
        "MIXED",
        "COMPOUND_MIXED",
    ));
    cases
}

fn case(id: &str, command: &str, class: &str, locality: &str, reason: &str) -> Value {
    json!({"id":id, "payload":{"command":command},
        "result":{"classification":class, "locality":locality, "reason":reason,
            "uncertainty":if class == "UNKNOWN" { "OBSERVE" } else { "NONE" }}})
}
