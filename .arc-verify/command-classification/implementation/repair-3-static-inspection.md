# SSH repair static inspection

Self-inspection using oma-backend; not independent oma-qa.

`crates/arc-detector/src/rules.rs` now isolates SSH option-position inspection in `ssh_forwarding`. Known no-value switches allow a later forwarding flag in the same option cluster; L/R/D/W/N at option positions select SSH_FORWARDING. Known valued switches stop scanning that word, consuming an attached opaque suffix or one separate value token. Unsupported options do not produce resolved remote execution: the original first option is still rejected by destination validation as SSH_AMBIGUOUS. Unknown option arity stops the forwarding scan conservatively. A literal -- terminates option scanning. Destination and remote-command words remain opaque to this helper. No option value is reinterpreted as another invocation or option.

The helper charges each visited option/value token, equality comparison, prefix inspection and possible option byte scan before doing the work. It shares the existing root budget. Parsing is linear in bounded input and contains no recursion or allocation. No subprocess, tool lookup, filesystem access, environment lookup, endpoint call, resolver, persistence or dependency was introduced. The existing static-inspection.md describes the preserved detector/IPC/daemon/CLI call path; its earlier source hashes are historical. repair-3-source-inventory.json and final command receipt source maps identify the current inputs.

Regression mapping:

| Requirement | Current tests |
| --- | --- |
| Option values with forwarding-like letters stay ambiguous | detector syntax_quotes_aggregation_and_ssh_negative_matrix; IPC ssh_option_values_are_not_forwarding_flags; actual daemon/CLI ssh_option_values_preserve_typed_daemon_and_cli_observations |
| Attached/combined forwarding remains recognized | Same tests cover -D8080, -nNT, -vNL8080:host:80 and a genuine -N after an attached username |
| Separate values are opaque, including flag-looking values | Same tests cover a separate username N and a separate -o value --N |
| Unknown option arity stays conservative | Same tests cover -zN returning SSH_AMBIGUOUS |
| Human and JSON output retain the shared schema | Actual daemon/CLI regression checks complete human output and parsed JSON from real arcctl subprocesses |
| No database dependency | IPC callback panics if invoked; actual daemon regression asserts database high-water usage stays zero |

TDD_EVIDENCE: repair-3-ssh-red (exit 101) fails the added -lDora case before the product repair, observing SshForwarding versus SshAmbiguous. repair-3-ssh-green (exit 0) runs the identical command after the repair. Receipts retain actual argv, complete logs and pre/post source hashes. No test was removed, ignored or weakened. Broader matrix remains in test-matrix.md and is rerun by current workspace checks.
