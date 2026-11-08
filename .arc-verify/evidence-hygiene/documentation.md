# Task-3 documentation inspection

Status: completed for the assigned documentation work. Final integrated mission acceptance remains downstream.

## Scope and source

Used `oma-docs` and the installed docs-curator definition. This is a documentation-only change; no new visual system or supplier was needed. No user study, simulation, browser interaction, external delivery, dependency installation or coverage/Cargo execution occurred.

Accepted source baseline: `aacb3b4359f13c31c0705c0cffc5bf1ac00f387e`, tree `8131af56cf26cbf3d332fbcf1af00858e5a4bc3a`. Inspected Actor-local HEAD/snapshot commit: `97e6c8234a1be7e3d46c82eb6149684fa449cbcb`, tree `c9fd6c00741218a5acd90eb6a065a3b345595cd3`. These identities were read with `git rev-parse`; the documentation edits are uncommitted task-3 changes, not bytes asserted to exist at either commit. Source evidence identifies snapshot ref `refs/agent-valley/task-evidence/8753207314f4ee46-1724980aa084bbaa-1/snapshot` and worker ref `refs/agent-valley/task-evidence/8753207314f4ee46-1724980aa084bbaa-1/worker`; no substituted source revision is used.

Consumed task-1's `retention.json`, its selected historical source hashes and handoff, and task-2's implemented `.gitignore`, acceptance receipt path, implementation report and preliminary clean-checkout receipt. Read the supplied paths for collector, command capture, configuration and live evidence verification to check the documented behavior. Serena was not available in callable tools; no repository code search or substitute search provider was used.

Ownership: changed only [coverage guide](../../docs/rust-coverage.md), [evidence index](index.md) and this report. README already links the guide directly, so no README change was necessary. Scripts, manifests, raw artifacts, frozen historical files and other Actors' reports were not edited. The CLI regenerated ignored `docs/generated/doc-refs.json` as its documented verification side effect; it is not a deliverable.

## Evidence navigation

| Journey inspected | Result of text/path inspection |
| --- | --- |
| First use | README leads to the guide, pinned tool versions, explicit provisioning, root coverage command, six labeled crate results and printed report directory's `uncovered.md`. |
| Returning contributor | Guide explains stable crate order, covered/total counts, exact independent 80% decisions and per-file gaps. `PASS`, `FAIL` and `UNAVAILABLE` remain plain text labels; no prompts or color dependence were introduced. |
| Current native evidence | Guide/index require collection before `bun test scripts/coverage/evidence.test.ts` in the collecting checkout. They identify ignored latest/run/target paths, unchanged native exports, source matching, profile hashes, complete merge membership and child PID proof. |
| Failure recovery | Missing data requires collection; failed latest attempts require diagnostic recovery and recollection; changed inputs require a fresh run. Output capture and three stage labels are explained. An older passing pointer is never a substitute. |
| Maintainer acceptance | Index separates Historical receipts, current local output and Fresh amendment verification. Task-2's clean-checkout result is labeled preliminary and source-scoped; final integrated results remain downstream. |
| Historical lookup | All 24 selected destinations are linked directly. Original storage commit, unknown execution HEAD, accounting baseline, source hash, host, collection times and run ID are explicit. Removed raw/inventory/log references in frozen reports are explained as historical Git paths with executable `git show` examples. |

These are inspection observations, not observed contributor sessions or measured task success. The hypothesis is that one index reduces navigation and mistaken acceptance of historical passes. No improvement in task time, usability, retention, conversion or churn was measured. No simulation was performed. No experiment or retention prompt was added.

## Checks and outcomes

Initial diff intake: `git diff --stat HEAD` on the three owned paths was empty. The task began without existing task-3 edits. Unrestricted `git status --short` printed `av.yaml: Operation not permitted` even though it returned exit 0; global cleanliness is not established. No bypass was attempted.

`oma docs verify docs/rust-coverage.md --json --no-urls` before edits exited 0: one document, 10 references, zero skipped and zero broken. This physical-path baseline alone could not detect compaction drift: the Actor snapshot still contains old raw files. `oma docs sync aacb3b4359f13c31c0705c0cffc5bf1ac00f387e..97e6c8234a1be7e3d46c82eb6149684fa449cbcb --json` exited 0 with no proposals. Explicit task requirements and inspected implementation behavior supplied the semantic corrections.

During editing, the first index verification reported its not-yet-created `documentation.md` link missing. This report supplies that owned target; final verification follows below. External URLs were excluded with `--no-urls`; no network-link or assistive-technology check is claimed.

### Reproduce the retained-link and provenance inspection

Run from the repository root after receiving these three documents. This checks maintained documents against the selected retained destinations even if removed raw paths happen to remain locally. It also checks all selected originals against the accepted storage commit, verifies the 22 exact retained/copied files, and verifies provenance on both compact derived receipts. It does not test the final Git index for prohibited artifacts or original-local raw preservation.

```sh
python3 - <<'PY'
from pathlib import Path
from urllib.parse import unquote
import hashlib, json, re, subprocess
root = Path.cwd()
contract = json.loads((root / '.arc-verify/evidence-hygiene/retention.json').read_text())
selected = {x['destination'] for x in contract['retained']}
docs = ['README.md', 'docs/rust-coverage.md',
        '.arc-verify/evidence-hygiene/index.md',
        '.arc-verify/evidence-hygiene/documentation.md']
coverage_links = set()
for name in docs:
    text = re.sub(r'```.*?```', '', (root / name).read_text(), flags=re.S)
    links = re.findall(r'!?\[[^\]]*\]\(([^)]+)\)', text)
    links += re.findall(r'(?:href|src)=[\"\']([^\"\']+)', text)
    local = 0
    for target in links:
        target = unquote(target.split('#', 1)[0])
        if not target or re.match(r'^[a-zA-Z][\w+.-]*:', target):
            continue
        path = ((root / name).parent / target).resolve()
        assert path.exists(), (name, target, 'missing')
        relative = path.relative_to(root).as_posix()
        if relative.startswith('.arc-verify/rust-coverage/'):
            assert relative in selected, (name, target, 'not retained')
            coverage_links.add(relative)
        local += 1
    print(name, 'resolved local links:', local)
assert coverage_links == selected, selected - coverage_links
sha = lambda data: hashlib.sha256(data).hexdigest()
exact = derived = 0
for entry in contract['retained']:
    source = entry['source']
    original = subprocess.check_output(['git', 'show',
        contract['acceptedHead'] + ':' + source['path']])
    assert sha(original) == source['sha256'], source['path']
    data = (root / entry['destination']).read_bytes()
    if entry['action'] == 'derive-compact':
        receipt = json.loads(data)
        assert receipt['classification'] == 'Historical'
        provenance = receipt['provenance']
        assert provenance['originalSha256'] == source['sha256']
        assert provenance['originalPath'] == source['path']
        assert provenance['storageCommit'] == contract['acceptedHead']
        derived += 1
    else:
        assert sha(data) == source['sha256'], entry['destination']
        exact += 1
print('retained destinations linked:', len(coverage_links))
print('exact historical files:', exact, 'compact provenance receipts:', derived)
PY
```

## Integration handoff

Measured Actor-local index at intake: **932 rust-coverage paths**, including task-2's five new historical files and the original paths. Task-2's frozen retention receipt instead records its own staged checkpoint as **24 files / 475,720 bytes**, zero prohibited artifacts and seven evidence-hygiene files. These are different scopes, not a current success claim. This Actor snapshot does not reproduce the dependency's index-only removals. Documentation paths and source hashes are present, but the Chief/implementation owner must preserve/reapply the reviewed retention boundary during integration and rerun final hygiene accounting. This task does not own index cleanup and made no index changes.

The task-2 receipt's raw preservation claim concerns the original raw-bearing workspace and its ignored manifest. This snapshot cannot establish that claim and was not used to relabel it. Keep that original workspace intact and rehash there after integration. Final Cargo/harness commands, amendment receipts and independent QA remain with their assigned owners. No documentation change alters root command behavior, interfaces, thresholds or verification commands.

## Acceptance checklist

- [x] Guide/index distinguish historical receipts, current local output and fresh preliminary amendment verification; no old pass is relabeled fresh.
- [x] Maintained links cover retained contracts, summaries, uncovered reports, receipts and verdicts. Removed raw references are explained as local/historical provenance.
- [x] Inspection records journeys, reproducible link/provenance checks, source scope and unmeasured usability assumptions.
- [x] Unattended terminal workflow and all six independent 80% floors remain intact in documentation; implementation files were not changed.

Final checks executed in this Actor worktree:

| Command/check | Actual outcome |
| --- | --- |
| `oma docs verify docs/rust-coverage.md --json --no-urls` | Exit 0; 1 document, 15 references, zero skipped/broken. Before: 10 references, zero broken; after: 15 references, zero broken. |
| `oma docs verify .arc-verify/evidence-hygiene/index.md --json --no-urls` | Exit 0; 1 document, 40 references, zero skipped/broken. The intermediate missing report link is resolved. |
| `oma docs verify .arc-verify/evidence-hygiene/documentation.md --json --no-urls` | Exit 0; 1 document, 16 references, zero skipped/broken after adding the command-results table. |
| `oma docs verify README.md --json --no-urls` | Exit 0; 1 document, 1 reference, zero skipped/broken. README unchanged. |
| Reproduction script above, extracted and executed using Python | Exit 0; 45 local Markdown/HTML links resolved across four documents: README 1, guide 9, index 33, report 2. All 24 retained destinations linked; 22 exact historical files and two derived provenance receipts verified. All 24 original source hashes matched bytes in accepted storage commit. |
| `git diff --check -- docs/rust-coverage.md README.md .arc-verify/evidence-hygiene/index.md .arc-verify/evidence-hygiene/documentation.md` | Exit 0; no tracked whitespace errors. New files were also checked directly for trailing whitespace and final newline. |
| `git diff --exit-code HEAD -- scripts .gitignore Cargo.toml Cargo.lock package.json crates .arc-verify/rust-coverage .arc-verify/evidence-hygiene/retention.json .arc-verify/evidence-hygiene/implementation.md` | Exit 0, empty; inspected implementation/dependency files unchanged from this Actor snapshot. This is not a comparison of amended scripts to the accepted baseline. |

The CLI counts include its own file-reference extraction rules; the explicit reproduction script counts Markdown/HTML links only. Neither scan proves user comprehension. There were no skipped doc candidates requiring edits: sync returned none, README was already correct, and frozen historical reports were preserved as required. Their archived links are explained through the maintained index rather than rewritten. No outstanding in-scope broken maintained link remains. Remaining mission risks are the snapshot/index discrepancy, original-local preservation after integration and final required verification, as described above.
