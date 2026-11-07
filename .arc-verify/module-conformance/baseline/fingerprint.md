# Source fingerprint contract

Algorithm: SHA-256 over UTF-8 JSON records sorted by project-relative path.
Serialize with sorted object keys, no spaces, no ASCII escaping and no trailing
newline. Each regular-file record has `path`, `mode` (Python octal permission
string), `sha256` (actual raw bytes), and `bytes`. A missing optional file has
`path` and `missing: true`. Hash bytes; never normalize source for this hash.

`fingerprint-files.json` records the baseline set. It includes all accepted
`crates/` files, dependency/toolchain configuration, both harness scripts,
`docs/architecture/CONSTRAINTS.md`, `docs/harness/` and OMA configuration.
Optional Cargo and local OMA configuration files have explicit missing markers.

For later snapshots, use the union of that seed path set and:

```
git ls-files --cached --others --exclude-standard -- crates scripts/harness .cargo
```

This is a Git path inventory, not a recursive source text/symbol search. It
captures newly created Rust modules before staging and preserves deleted seed
paths as missing markers. Relevant new configuration outside these scopes must
be added explicitly to the seed set and reported as a scope extension.
Use `capture.py source_fingerprint()` as the executable reference. The baseline
value is in `baseline.json`; moving tests necessarily changes the value.

Do not hash `.arc-verify/` into its own source identity. Hash evidence separately
and tie every final command receipt to the same final source fingerprint.
Command receipts record exact argv, working directory, environment overrides,
timestamps, return code, timeout, ordered lossless output parts and their hashes.
The initial preflight and IPC discovery receipts predate the recorder's
per-command fingerprint fields. Their source association is established by the
accepted-byte capture and final unchanged-source audit, not by retroactive
per-command timestamps. They remain baseline evidence only.

No dependency cache, build artifact, external filesystem state or running
process is covered by the source fingerprint. Document those prerequisites
and fixture effects separately. Global worktree cleanliness remains unverified
because Git status reports an access denial for `av.yaml`.
