# Runtime identity: value and scope

## Operator value

This mission funds a focused macOS runtime identity repair. The following benefits are expected from the required behavior; they are not measured user or commercial outcomes.

| Required behavior | Expected operator benefit | Evidence required before claiming delivery |
|---|---|---|
| Resolve the default endpoint through `confstr(_CS_DARWIN_USER_TEMP_DIR)`, independently of `TMPDIR` | Starting ARC and checking it from another terminal should address the same default daemon | Unset, changed and unwritable child `TMPDIR` values yield the same endpoint; `status` and `doctor` reach it |
| Use one stable per-user instance anchor, independent of `ARC_SOCKET_PATH` and `ARC_DB_PATH` | Competing launches should not create separate daemon writers or initialize unwanted storage | Concurrent starts with different overrides admit exactly one daemon; the loser creates no database, WAL/SHM or socket |
| Recover after graceful shutdown and forced death | An owned daemon should restart without manual lock deletion | Subprocess restart checks after both shutdown modes, with CLI reachability |
| Preserve directory and entry protections | A rejected startup should leave existing contents intact | Directories remain owner-only `0700`, sockets/locks `0600`; unsafe directories, symlinks and overlong socket paths reject without mutation |

Explicit socket overrides and the production persistent-database policy remain requirements. A stable default endpoint does not promise automatic discovery of an overridden endpoint. Supported writable fixture roots and serialization of complete daemon fixture lifecycles must make the evidence repeatable without weakening production uniqueness. `0001_phase0.sql` must remain unchanged.

## Why fund this scope

The inspected baseline contains two concrete identity gaps: the default socket follows the process temporary directory, and the instance lock follows the selected socket. Repairing those paths addresses the stated operator failure directly. The supplied Technical and Design Director consultations support reusing the existing Rust stack, filesystem protections and two-command CLI rather than adding a new interaction.

Endpoint discovery would introduce another behavior and validation contract. Packaging and a landing page address distribution rather than runtime identity. A paid diagnostic experiment would require buyer qualification, pricing, delivery capacity and separate authority for commercial activity. None is needed to prove this repair. No new market research, outreach, publication, spending or commercial infrastructure is included in this task.

## Evidence limits

Repository facts above come from direct reads of `crates/arc-core/src/lib.rs` and `crates/arc-daemon/src/instance.rs` at accepted committed baseline `073802555b639aa2fec183d93f748d9437cbbcad`, tree `ce009646036415990a815170955421e1ab8d5c2f`. This Actor's initial local HEAD is snapshot commit `6e5caae0b48417bc644894ce0062aba2037654b8`, with the same tree. Those observations describe the input baseline, not an integrated repair.

Cross-terminal developers and override-using integrators are plausible users, not researched participants. Fewer failed launches, retries or support requests are hypotheses. No adoption, customer reaction, time saving, support reduction or willingness to pay has been measured here. Supplied marketing opportunities are hypotheses, not a pipeline or authorization to contact anyone.

Revenue, pricing, labor hours and labor cost, acquisition cost, delivery/support cost and ROI are unknown and unmeasured. No quantified savings or profit improvement can be attributed to this mission. Commercial experiments remain outside its scope.

This report does not establish delivery. Independent acceptance must verify the runtime cases and the unchanged migration, and record actual macOS sandbox results for `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`. Historical passes cannot substitute for those results. This documentation task ran no runtime tests or commercial experiments.

## Report verification

Skill used: `oma-docs`. Manual inspection — `oma docs verify` did not run because its generated reference index falls outside this task's sole owned file. The document's required headings, evidence qualifiers, provenance, scope boundaries and file-change boundary were checked locally. There are no Markdown links or HTML resource references to resolve. Product acceptance remains with the implementation and independent review tasks.
