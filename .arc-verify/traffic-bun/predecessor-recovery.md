# Task 9: predecessor input recovery

## Attempt 9 — Actor-local recovery and handoff

Status: **task 9 complete**. The authenticated recovery tool, source table and restored files were verified in this Actor workspace. The recursive validator and both complete archive fixture files exited successfully, and an independent audit rehashed all 47 destinations after the fixtures. Task 8 owns restoration and verification in the final workspace. This result does not establish the mission-wide gates or final-workspace recovery. Earlier sections retain their original attempt-specific status and provenance.

### Source and workspace identity

- Accepted committed baseline: `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`.
- This Actor's inspected HEAD and supplied snapshot are `f709cf18152da86fb0303a2512e25f384e6bf816`, tree `2474bca41de15b7b6643c1ee546bd2aa86842df4`. The snapshot ref resolved to that commit in the mission source repository. The supplied worker ref did not resolve before collection; no worker commit is inferred. The report and tool are tracked in this snapshot, but neither exists in the accepted baseline. This section is an uncommitted Actor-local report change.
- The Actor root is `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/59981caa-53a9-40cb-b3a1-3c3dfbb22b09/workspace/.agent-valley/parallel/workspaces/1f5006e3792c6ab1-e34d3152baeb112c-8`. At the read-only inspection, the mission source workspace's index contained the earlier report and tool as staged additions. That observation does not establish delivery of this report update. No final-workspace write was attempted.
- The retained attempt-2 script, delivered restoration tool, historical table and selected collection matched SHA-256 `81f1d4572c8c26ad55b5d01e0037f74501034490518b61ad2c42f52d378e3c31`, `7bf1aec7a4c87ef52d7f85d7b10960585c9bf63bdee4263f5754740cf2af29b4`, `78f8a822b4fb1296eb264fcdd6618582a9018de900d94552399ce7b7d20c02ea` and `3bad816a4f054f1f52fbbb7479a278c89a119f35fb8f0c074dc8166a9cb246ac`, respectively. The tool is unchanged from this Actor's snapshot.
- The 24 Git-backed archives come from the historical object database at `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace`; their object IDs, sizes and SHA-256 values are in the unchanged historical table. The remaining two archive entries come from the 152,867,036-byte authenticated original at `/Users/gracefullight/workspace/arc/.agent-valley/maintenance-logs/task19-shadow-workspace/target/exec-fail-open/task19/frozen/target/exec-fail-open/task13-preserved/collection-next-original.json`, SHA-256 `4456ce62e9949a3a8e1b208983ec67318cf0ff73e596c76640f7a9149f6e9366`. The 13 frozen inputs and eight original-location copies are selected and verified using that same historical table. The tool and the attempt-4 section below give the exact paths and independent audit commands. The deleted task-16 manifest was not reconstructed.
- Before and after the checks, the scoped `scripts/harness/exec-fail-open*`, historical table and selected collection matched the accepted baseline. This is not a whole-worktree clean claim. Full `git status` reported `av.yaml: Operation not permitted`; Serena tools were unavailable and named files were read directly.

### Checks in this Actor checkout

The checks ran serially on Darwin arm64, Bun 1.4.4 and Python 3.14.10. The host reported 16 GiB RAM, eight logical CPUs and approximately 56 GiB free before the run. Each subprocess had a 7,200-second timeout. Both fixture commands were unfiltered.

| Command | Result |
| --- | --- |
| `python3 -u target/traffic-bun/task9-attempt9-safety.py` | Exit 0; verified exclusive creation, identical and differing existing files, source size and digest mismatches, symlinked parents, destinations and workspace root, relative root and traversal rejection. |
| `python3 -u .arc-verify/traffic-bun/restore-predecessor.py --workspace-root "$PWD"` | Exit 0; all 47 existing destinations were individually checked as identical, with a 26/13/8 passing finish event. The earlier attempt-8 restoration in this same checkout created all 47. |
| `bun -e 'import {reference} from "./scripts/harness/exec-fail-open-inputs"; import {canonicalArchive} from "./scripts/harness/exec-fail-open-compact"; const root=process.cwd(); const p=".arc-verify/exec-fail-open/implementation/collection-next.json"; const entries=canonicalArchive(root,reference(root,p)); console.log("validated top-level archived selectors:",Object.keys(entries).length);'` | Exit 0; the recursive validator reported 12 top-level selectors and authenticated their nested dependencies. |
| `bun test scripts/harness/exec-fail-open-compact.test.ts` | Exit 0; 16 passed, zero failed, 724 assertions. |
| `bun test scripts/harness/exec-fail-open-compact-flow.test.ts` | Exit 0; two passed, zero failed, 190 assertions. |
| `python3 -u target/traffic-bun/task9-attempt9-audit.py` | Exit 0 after both fixtures; independently checked regular-file type, byte count and SHA-256 for 26 archives, 13 frozen files and eight original-location copies. |

The following ignored files are under this Actor root's `target/traffic-bun/`. Each receipt records exact argv, cwd, PID, timestamps, exit, timeout and stream hashes; all receipt-referenced stream hashes were rechecked. They do not transfer automatically with the tracked report.

| Check and basename | Receipt SHA-256 | stdout SHA-256 | stderr SHA-256 |
| --- | --- | --- | --- |
| `task9-attempt9-safety` | `4782683d51032a2588db17f121028bb74e6c825937c765f2734770fe0dbf8884` | `77d29e51c4d7deba39a16e693598e47d8986ac552b9a2990b720e944d94218ae` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `task9-attempt9-restore` | `974274257af1ceaf4b728504c812616b20a8de115b63a8742ab6fec1265afc5a` | `8e010255a5a70a9f3937ee6d6a8f8af914e9023787964d611ef0022893fa6f62` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `task9-attempt9-recursive` | `9999b4b55cf662cc7e15417100f249eec4530d3bd2a8dac42d6232bed2577222` | `491fd1e0bb69df263520f8b5d8fd6155e88b8c929a024b4c4a8929730eacc492` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `task9-attempt9-compact` | `910a90a01eb9b010142877d4872d5c0f65add183e73a9213b8466ec991455abf` | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `ec3ce47c6579e5494e2a00b78d79aa272ac75194b9c2217a67bd16dee142a901` |
| `task9-attempt9-compact-flow` | `3efefa79693f616502f5dfb9319dbb820bfbf6b80c96211b53c22c65c9939bd7` | `cd449c856d12f7f9c1d8f20399f3531cef1c48aba92566fcfe4aaaece10aca3d` | `a2c144a5c6c2211193bda0017f771b7801e944ae2fb1e15ca286a1a2226d88f7` |
| `task9-attempt9-postaudit` | `01a700ed7631530ac4eeb319ff7d693f2475edb1c43ec31e19fd185722ba9b64` | `5c31de04dadf50e280da7e46440b2029d2b403e6e4c9bc773ea7075d23022c32` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

The attempt-8 creation receipt is `target/traffic-bun/task9-attempt8-restore.json`, SHA-256 `08c65c79b93341daf62f1b6f93e1012878f6cba8ad1788bd5252129286a6e153`; its stdout is SHA-256 `7c1accdf3b45c408e1e12a523d4851cf0fedef2827889e4aa7dfa8d6b05b9e36`. The attempt-9 runner, safety helper and audit helper have SHA-256 `fb8bbfc04d52caaf8d85c4f76f1b8a1c733cd43c604a901c2cbe554c4920215b`, `e1afaa6c09088eb402af747fcab772753e3ea79e08172f01469b3fc09d6d23cc` and `d23e1ccd42e7f0ef520ddee74e44c6a4f11a35aa884a4f6ac3feea9f5d51bf99`. `oma-qa` guided this scoped verification; it is separate from task 7's independent product review.

### Task 8 handoff

Task 8 should verify the delivered tool hash, run `python3 -u .arc-verify/traffic-bun/restore-predecessor.py --workspace-root "$PWD"` from its authorized final workspace, and run the independent 47-file audit and recursive validator. The exact copyable commands, source paths and expected hashes are retained in the attempt-4 handoff below. The runtime needs read access to the historical Git object database and shadow source, and write access to its own `target/exec-fail-open`. Missing or altered inputs must be reported, not fabricated. Actor-local restored files and raw receipts remain ignored and are not evidence of transfer to the final workspace.

## Attempt 7 — current Actor-local verification

Status: **task 9 complete**. The recovery tool and prior Actor-local restoration are present in this checkout. The unchanged recursive validator and both complete archive fixture files ran here, with successful recorded exits; an independent audit after the fixtures verified all 47 destinations. This task result does not establish completion of the mission-wide gates or final-workspace restoration.

### Source and workspace identity

- Accepted committed baseline `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`. This Actor's supplied snapshot and inspected HEAD are `6889d01dcf678cccaa81bbaf716dffb8988d9716`, tree `0adad5515e09f3deeded164ca2eddefcc8fcf3e4`. The snapshot reference resolves in the mission source repository; the supplied worker reference was not present when checked, so no worker commit is asserted.
- The Actor root is `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/59981caa-53a9-40cb-b3a1-3c3dfbb22b09/workspace/.agent-valley/parallel/workspaces/1f5006e3792c6ab1-e34d3152baeb112c-6`. The tool and report are tracked in this Actor snapshot but are absent from the accepted committed baseline. This attempt modifies the report in the Actor worktree; the tool is unchanged, SHA-256 `7bf1aec7a4c87ef52d7f85d7b10960585c9bf63bdee4263f5754740cf2af29b4`. Before this report edit, a read-only status of the mission source workspace showed the report as `AM` and the tool as `A`; that does not establish delivery of this new Actor-local edit. No write to that workspace was attempted.
- The retained attempt-2 source script, historical report, selected collection, and shadow original matched SHA-256 values `81f1d4572c8c26ad55b5d01e0037f74501034490518b61ad2c42f52d378e3c31`, `78f8a822b4fb1296eb264fcdd6618582a9018de900d94552399ce7b7d20c02ea`, `3bad816a4f054f1f52fbbb7479a278c89a119f35fb8f0c074dc8166a9cb246ac`, and `4456ce62e9949a3a8e1b208983ec67318cf0ff73e596c76640f7a9149f6e9366`, respectively. The original contains 53 entries and is 152,867,036 bytes. The tool's source paths, 24 historical Git blobs, two authenticated inline selections, 13 frozen inputs, and exact expected sizes and hashes are identified in the attempt-5 and attempt-4 sections below and in the unchanged historical report.
- The checked `scripts/harness/exec-fail-open*`, `.arc-verify/exec-fail-open`, and `.arc-verify/traffic/predecessor-recovery.md` paths match the accepted baseline after the fixtures. Before the report edit the Actor snapshot's `sourceContext` reported source hash `02506ddc0356dbddfe90de84a76e2df2fffd922fc5c0ef4f9e56462f69b433fe`. This is an Actor-snapshot observation, not a substitute for the mission workspace's final source identity.

### Checks in this Actor checkout

The continuation ran on Darwin arm64 with Bun 1.4.4 and Python 3.14.10. Before the batch the host reported 16 GiB RAM, eight logical CPUs and about 58 GiB available. The receipt runner used a 7,200-second limit for each check. Attempt-6 results below belong to this same Actor workspace and were verified against their retained receipts and stream hashes; the full-flow rerun and post-fixture audit are from this continuation.

| Command | Result |
| --- | --- |
| `python3 -u target/traffic-bun/task9-attempt6-safety.py` | Exit 0. Synthetic checks covered exclusive creation, identical and different existing bytes, bad source size and digest, symlinked parent, destination and workspace root, relative root and parent traversal. |
| `python3 -u .arc-verify/traffic-bun/restore-predecessor.py --workspace-root "$PWD"` | Exit 0. The finish event reported 26 archive, 13 frozen and eight original-location destinations. |
| `bun -e 'import {reference} from "./scripts/harness/exec-fail-open-inputs"; import {canonicalArchive} from "./scripts/harness/exec-fail-open-compact"; const root=process.cwd(); const p=".arc-verify/exec-fail-open/implementation/collection-next.json"; const entries=canonicalArchive(root,reference(root,p)); console.log("validated top-level archived selectors:",Object.keys(entries).length);'` | Exit 0; 12 top-level selectors and their nested dependencies validated. |
| `bun test scripts/harness/exec-fail-open-compact.test.ts` | Exit 0; 16 passed, zero failed, 724 assertions. |
| `bun test scripts/harness/exec-fail-open-compact-flow.test.ts` | Exit 0; two passed, zero failed, 190 assertions. No filter or skip. |
| `python3 -u target/traffic-bun/task9-attempt6-audit.py` | Exit 0 after both fixtures; a separate read-only pass verified the size, SHA-256 and regular-file type of all 47 destinations against the historical table. |

Receipts and raw streams below are in this Actor workspace's ignored `target/traffic-bun/`. Each receipt records exact argv, cwd, process ID, timestamps, exit, timeout and stream hashes; all listed hashes were checked. These files do not transfer automatically with the tracked report.

| Check | Receipt and log basename | Receipt SHA-256 | stdout SHA-256 | stderr SHA-256 |
| --- | --- | --- | --- | --- |
| Safety | `task9-attempt6-safety` | `3130496a277719e3d60d2b3cf1dbe6fb0929dd548ce9fa569d28c481dc989afa` | `ca97d473bee2c409e63651b56092de0d9a451442ada3779ded8de09af2e107fb` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| Restore | `task9-attempt6-restore` | `bf011e7d7ef96ae50798de460bf14e95f709ba8377be59de323fccca30f05b16` | `91e950406b12ad07b8d838b5d8b77a420eed3c2a0420c3dc317bf3e25e9207f6` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| Recursive validator | `task9-attempt6-recursive` | `094448c04f02fdf484e0b7e1c05cfccecfcf0b48a5e12d42f53d1a6f3f265301` | `491fd1e0bb69df263520f8b5d8fd6155e88b8c929a024b4c4a8929730eacc492` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| Compact fixture | `task9-attempt6-compact` | `b62253687c39bbbcac929dfb0215a9ea685a31924ca10a7a67f049fa15c02041` | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `46c55eb4d7573c45928251e2c379794567182d0359d9ee7ae97031cb847cb74b` |
| Complete flow fixture | `task9-attempt7-compact-flow` | `13b39745fc777e0f680e3dcb832f54b0e5427154e2cd879ef35a1774b75aa4ca` | `cd449c856d12f7f9c1d8f20399f3531cef1c48aba92566fcfe4aaaece10aca3d` | `b75056da9dd91b287a58de0833b4cf1eff90aff353a556e5b86c017374d88fab` |
| Post-fixture audit | `task9-attempt7-postaudit` | `f3309c628f21e091008a24e22b16409feb946e8beb36805b93bac2df8cb74ded` | `3d1a7295c944e5abf2b2c9f869627b02dc1b320568ba8141c01615181bd8f0ed` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| Actor-snapshot final evidence | `task9-attempt7-evidence` | `36c2e4dbfe285a4c3a5afa60cc3f128c8947239c3b157b721e8c36df37ff61ad` | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `34db8103d34b394dc57542c77605198989aba56addeaba43511038648df484f1` |

The prior attempt's flow runner was interrupted while its child remained active. Its eventual stderr reported two passes and zero failures, but its parent did not record the exit; it is not used as a successful exit receipt. The fresh full-flow run above provides that receipt. The attempt-5 sibling workspace named in the historical section below was absent when checked in this attempt; its earlier results remain historical and are not represented as revalidated here.

### Final-gate handoff and outstanding mission work

Task 8 must run the authenticated restoration, independent audit and recursive validator in its own authorized final workspace. The exact copyable commands, expected tool hash, source paths, historical size/digest table references and 47-file audit are in the attempt-4 handoff section below. Its operator needs read access to the historical Git object database and shadow source, and write access to its own `target/exec-fail-open`; the tool rejects missing or different bytes and symlinked parents. Neither the historical task-16 manifest nor any predecessor evidence was invented or modified. The restored Actor-local files and raw logs are ignored files; do not assume they arrived in the final workspace.

The operator's later verification note was checked locally by running `bun test scripts/coverage/traffic-bun-evidence.test.ts` in this Actor snapshot before editing this report. It exited 1: 13 passed, one failed, with `TRAFFIC_EVIDENCE: gate summary is incomplete or stale`. In this snapshot the evidence budget advanced past the indexed-file check; the checked summary records `coveragePassed: false` and `fullHarnessPassed: false`. This is not a test of the mission source workspace, where the supplied failure reported an indexed/worktree difference. The note assigns repair of the orphan `crates/arc-cli/src/exec/transport/tests.rs` and worktree-based evidence/harness inventories in the `scripts/coverage/traffic-receipts.ts` and `scripts/coverage/traffic-bun-evidence.ts` paths, followed by final evidence binding, to the backend engineer. Those source files are outside task 9 ownership. The independent product `oma-qa` review is also a separate assignment.

`oma-qa` guided this scoped verification and reporting. Serena tools were not exposed, so named paths were read directly. Full `git status` reported `av.yaml: Operation not permitted`, and `ps` was also denied; no whole-worktree clean or unrestricted process-inventory claim is made.

## Attempt 5 — current Actor-local verification

Status: **completed for task 9**. This attempt restored and independently rehashed 26 archive entries, 13 frozen inputs and eight original-location copies in the Actor workspace, then ran the unchanged recursive archive validator and both complete archive fixture files. All six recorded checks exited zero. This is not a claim that the mission-wide gates passed, or that these ignored files were transferred to the final verification workspace. The previous attempt sections remain below as historical records.

### Source and workspace identity

- Accepted committed baseline: `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, product tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`.
- This Actor's initial snapshot and inspected HEAD: `ca683db4214a54ce9e605e8c7785d270df3ea4c0`, tree `e8a7ef5e6c4537a7ff6f5ca87f12fa39d31a8ac0`; its supplied snapshot ref resolved to that commit in the mission source repository. The supplied worker ref had not been created there when checked, so no collected worker revision is asserted. The tool and report are tracked in this snapshot but absent from the accepted committed baseline. This attempt's report changes are uncommitted in the Actor workspace; the tool itself is unchanged.
- The Actor root is `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/59981caa-53a9-40cb-b3a1-3c3dfbb22b09/workspace/.agent-valley/parallel/workspaces/1f5006e3792c6ab1-e34d3152baeb112c-5`. No write to the mission source workspace was attempted. At a separate read-only inspection, the mission source workspace was at accepted HEAD `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, and its index showed both this report and the tool as staged additions. Before this attempt's report edit, their working-tree hashes there matched this Actor snapshot: report `eab6fc3bc4b917e9cab482a0f8fbd501e060a31dc37508e7894f51fa680a3710`, tool `7bf1aec7a4c87ef52d7f85d7b10960585c9bf63bdee4263f5754740cf2af29b4`. The new Actor-local report edit still needs the normal delivery handoff.
- The retained attempt-2 script was read from the sibling Actor workspace `../1f5006e3792c6ab1-e34d3152baeb112c-2/target/traffic-bun/task9-attempt2-restore.py`, SHA-256 `81f1d4572c8c26ad55b5d01e0037f74501034490518b61ad2c42f52d378e3c31`. The handoff tool's diff from it requires an explicit workspace root while preserving the authenticated restoration logic; its SHA-256 is `7bf1aec7a4c87ef52d7f85d7b10960585c9bf63bdee4263f5754740cf2af29b4`.
- The authoritative historical size, SHA-256 and source table is `.arc-verify/traffic/predecessor-recovery.md`, SHA-256 `78f8a822b4fb1296eb264fcdd6618582a9018de900d94552399ce7b7d20c02ea`. The selected collection `.arc-verify/exec-fail-open/implementation/collection-next.json` is SHA-256 `3bad816a4f054f1f52fbbb7479a278c89a119f35fb8f0c074dc8166a9cb246ac`. Both and the fixture files under `scripts/harness` match the accepted commit in a scoped `git diff --exit-code` before and after the checks.
- The 24 blob-backed archive bytes came from the historical Git object database at `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace`; the historical table identifies each blob and its expected byte size and SHA-256. Object presence alone is not a claim of reachability from that repository's HEAD. The other two archive entries were decoded only from the authenticated 53-entry original at `/Users/gracefullight/workspace/arc/.agent-valley/maintenance-logs/task19-shadow-workspace/target/exec-fail-open/task19/frozen/target/exec-fail-open/task13-preserved/collection-next-original.json`, 152,867,036 bytes, SHA-256 `4456ce62e9949a3a8e1b208983ec67318cf0ff73e596c76640f7a9149f6e9366`. All 13 frozen inputs came from that shadow's `task19/frozen` root and were checked against the historical table; eight were additionally copied to their recorded original `target/exec-fail-open` locations. The separate deleted task-16 manifest was not reconstructed, and no historical evidence was changed.

### Checks performed

Commands ran serially from the Actor root on Darwin arm64, Bun 1.4.4 and Python 3.14.10, with 16 GiB RAM, eight logical CPUs and about 60 GiB free before the run. Each subprocess had a 7,200-second timeout, matching the mission's per-check allowance. No fixture filter or skip was used.

| Command | Result |
| --- | --- |
| `python3 -u target/traffic-bun/task9-attempt5-safety.py` | Exit 0. Tested exclusive creation, acceptance of identical existing bytes, rejection of differing destination bytes and source size mismatch, and rejection of symlinked destination parents, destination files and workspace roots, plus relative roots. |
| `python3 -u .arc-verify/traffic-bun/restore-predecessor.py --workspace-root "$PWD"` | Exit 0; 47 destinations created and individually verified, with a passing 26/13/8 finish record. |
| `bun -e 'import {reference} from "./scripts/harness/exec-fail-open-inputs"; import {canonicalArchive} from "./scripts/harness/exec-fail-open-compact"; const root=process.cwd(); const p=".arc-verify/exec-fail-open/implementation/collection-next.json"; const entries=canonicalArchive(root,reference(root,p)); console.log("validated top-level archived selectors:",Object.keys(entries).length);'` | Exit 0; 12 top-level selectors and their nested dependencies validated. |
| `bun test scripts/harness/exec-fail-open-compact.test.ts` | Exit 0; 16 passed, zero failed, 724 assertions, 340.472 seconds. |
| `bun test scripts/harness/exec-fail-open-compact-flow.test.ts` | Exit 0; two passed, zero failed, 190 assertions, 379.334 seconds. |
| `python3 -u target/traffic-bun/task9-attempt5-audit.py` | Exit 0 after both suites; a separate read-only audit rehashed all 47 expected regular files against the historical table. |

The Actor-local receipt `target/traffic-bun/task9-attempt5-checks.jsonl` is SHA-256 `6f7e1732e0ef616c55f0eaba2397bfe4f26e06d2c52114c6fd9200bba96402d7`. It records exact argv, cwd, process IDs, start and finish times, exit statuses, timeout status and SHA-256 for each stdout and stderr. All 12 stream hashes were independently rechecked. The files remain under this Actor's ignored `target/traffic-bun` and are not implied to transfer with tracked files.

| Log basename under `target/traffic-bun/` | stdout SHA-256 | stderr SHA-256 |
| --- | --- | --- |
| `task9-attempt5-safety` | `db70d1a243ebca6be182282b49d99cac9a20cd5cdb38412f408d14da1a7915e3` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `task9-attempt5-restore` | `ce930cc0f493c3e7a4661616bbbe5e9f708b7b598261e8efa8c3eb0ec3731a57` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `task9-attempt5-recursive` | `491fd1e0bb69df263520f8b5d8fd6155e88b8c929a024b4c4a8929730eacc492` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `task9-attempt5-compact` | `bbe6a94c29cc8d3ff5a1792a5abae1f1122aed0642111b52036ab7e0b2b2ca47` | `77931b88abe9e20e3381c799a0b9f77bb49f871422e7fa88bbb11a014255ec74` |
| `task9-attempt5-compact-flow` | `cd449c856d12f7f9c1d8f20399f3531cef1c48aba92566fcfe4aaaece10aca3d` | `031187a31c670d34c370fd46ef33f47c8ea94aed89ee51310d67ea6235aad485` |
| `task9-attempt5-audit` | `8b5a81f41783fb4eb75810e108d70fdae5b429171a1431332e6ba3debb7f64d3` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

The Actor-local collector, safety helper and audit helper have SHA-256 `4b8bce9225168b3d33e4c1faf714562283dccc762c8d1ea988078fd43a5182ce`, `53c424d55094dd1dea2674d3422163ac37c1ff519ed916338fde843ae5d727a4` and `7bfb60c2218dc7c9216b6de879e30005e924041341b59ffeaf54b71ef8d7b80e`, respectively. `oma-qa` guided this scoped verification and reporting; it is not the independent product review assigned to task 7.

### Final-gate owner handoff and remaining limits

Task 8 owns installation and verification in the final workspace. Its runtime needs read access to the historical Git object database and shadow source above, and write access to its selected workspace's `target/exec-fail-open`. First verify the delivered tool's SHA-256, then run the following from that authorized workspace using fresh log names; the tool refuses differing existing bytes and symlinked destination parents. The full independent 47-file audit snippet in the following attempt-4 handoff section remains applicable, as do its exact source and digest table references.

```sh
set -eu
set -C
mkdir -p target/traffic-bun
test "$(shasum -a 256 .arc-verify/traffic-bun/restore-predecessor.py | cut -d ' ' -f 1)" = 7bf1aec7a4c87ef52d7f85d7b10960585c9bf63bdee4263f5754740cf2af29b4
python3 -u .arc-verify/traffic-bun/restore-predecessor.py --workspace-root "$PWD" > target/traffic-bun/task9-handoff-restore.stdout 2> target/traffic-bun/task9-handoff-restore.stderr
bun -e 'import {reference} from "./scripts/harness/exec-fail-open-inputs"; import {canonicalArchive} from "./scripts/harness/exec-fail-open-compact"; const root=process.cwd(); const p=".arc-verify/exec-fail-open/implementation/collection-next.json"; const entries=canonicalArchive(root,reference(root,p)); console.log("validated top-level archived selectors:",Object.keys(entries).length);' > target/traffic-bun/task9-handoff-recursive.stdout 2> target/traffic-bun/task9-handoff-recursive.stderr
shasum -a 256 target/traffic-bun/task9-handoff-restore.stdout target/traffic-bun/task9-handoff-restore.stderr target/traffic-bun/task9-handoff-recursive.stdout target/traffic-bun/task9-handoff-recursive.stderr
```

Check for the tool's `status: passed` finish event and counts 26/13/8, run the independent 47-file audit, and require 12 top-level selectors from the recursive validator. The applicable complete fixture commands are listed in the check table above. Record fresh raw-log hashes in the final workspace. Do not invent the missing historical manifest if an input is absent or differs.

Serena tools were not exposed, so the work used named-file reads rather than code search. Full `git status` reported `av.yaml: Operation not permitted`; scoped status excluding that path was clean before this report edit, so no whole-worktree clean claim is made. An optional `ps` resource probe was also denied; process completion and exit status were observed through the owned subprocess runner. The assignment's supplied mission-level verification failure reports a stale or incomplete gate summary and a failed source-bound coverage baseline; the prior task-8 record also says the installed independent QA predates final source. Those are final-gate and review work outside task 9 and were not retested or repaired here.

## Attempt 4 — completed Actor-local handoff

Status: **completed for task 9**. The 26 archive entries, 13 frozen files and eight original-location copies were restored and rehashed in this Actor checkout. The recursive validator and both complete fixture files passed there. Task 8 must run the handoff in the final verification workspace; this attempt did not write there. The older attempt sections below retain their original wording, including their then-current failure status.

### Scope and provenance

- Accepted committed baseline: `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`.
- Inspected Actor HEAD/snapshot: `34a56833935321ff9ea2a42b32323e507d9865ba`, tree `d90fc31a234dd9c72399e2c60a43e3465139de37`. The supplied snapshot ref resolves to this commit in the mission source repository. The supplied worker ref did not resolve there during this running attempt; no worker commit is inferred. The current report modification and new restoration tool are Actor-local uncommitted changes. The report is tracked in this Actor snapshot but absent from the accepted baseline; this does not establish that it is tracked in the final review checkout.
- The mission source workspace is `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/59981caa-53a9-40cb-b3a1-3c3dfbb22b09/workspace`. This Actor root is its `.agent-valley/parallel/workspaces/1f5006e3792c6ab1-e34d3152baeb112c-3` child. No write into the mission source workspace was attempted in this run.
- The historical table `.arc-verify/traffic/predecessor-recovery.md` is SHA-256 `78f8a822b4fb1296eb264fcdd6618582a9018de900d94552399ce7b7d20c02ea`; the selected collection `.arc-verify/exec-fail-open/implementation/collection-next.json` is `3bad816a4f054f1f52fbbb7479a278c89a119f35fb8f0c074dc8166a9cb246ac`. The table gives the exact size, SHA-256 and source blob for all 26 archive entries and the size and SHA-256 for all 13 frozen entries. `git diff --exit-code 16772fe2aa350413225f3fb2bb9227d14ae5a6c4 -- scripts/harness .arc-verify/traffic/predecessor-recovery.md .arc-verify/exec-fail-open/implementation/collection-next.json` passed before and after the fixture runs. This is a scoped comparison, not a clean-tree claim.
- The 24 blob-backed archives were read from the object database of `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace`. Object presence does not establish reachability from that repository's HEAD. The two inline selections were decoded only after authenticating the 152,867,036-byte original at `/Users/gracefullight/workspace/arc/.agent-valley/maintenance-logs/task19-shadow-workspace/target/exec-fail-open/task19/frozen/target/exec-fail-open/task13-preserved/collection-next-original.json`, SHA-256 `4456ce62e9949a3a8e1b208983ec67318cf0ff73e596c76640f7a9149f6e9366`. All 13 frozen source files came from the same task-19 `frozen` root. No historical manifest or evidence was reconstructed or changed.
- The handoff tool `.arc-verify/traffic-bun/restore-predecessor.py` is SHA-256 `7bf1aec7a4c87ef52d7f85d7b10960585c9bf63bdee4263f5754740cf2af29b4`. It adapts the retained attempt-2 script, SHA-256 `81f1d4572c8c26ad55b5d01e0037f74501034490518b61ad2c42f52d378e3c31`, to require an explicit absolute `--workspace-root`. It authenticates source and destination bytes, uses exclusive creation, accepts only identical existing files, rejects symlinked destination parents and workspace path components, and rehashes installed destinations. It does not change the Git index or the deleted task-16 manifest.

### Checks in this Actor checkout

Commands ran serially on Darwin arm64 with Bun 1.4.4 and Python 3.14.10. The runner imposed no subprocess timeout, so no check was stopped before the mission's 7,200,000 ms allowance. The checkout had approximately 79 GiB free, 16 GiB RAM and eight logical CPUs before the run.

| Command | Result |
| --- | --- |
| `python3 -u target/traffic-bun/task9-attempt4-safety.py` | Exit 0. Synthetic cases checked exclusive creation, identical and differing existing bytes, source size mismatch, symlinked destination parent, symlinked root and relative root rejection. |
| `python3 -u .arc-verify/traffic-bun/restore-predecessor.py --workspace-root "$PWD"` | Exit 0. The tool recorded 47 verified destinations and a passing finish event: 26 archives, 13 frozen inputs and eight original-location copies. |
| `bun -e 'import {reference} from "./scripts/harness/exec-fail-open-inputs"; import {canonicalArchive} from "./scripts/harness/exec-fail-open-compact"; const root=process.cwd(); const p=".arc-verify/exec-fail-open/implementation/collection-next.json"; const entries=canonicalArchive(root,reference(root,p)); console.log("validated top-level archived selectors:",Object.keys(entries).length);'` | Exit 0. The existing recursive validator authenticated the archive chain and reported 12 top-level selectors. |
| `bun test scripts/harness/exec-fail-open-compact.test.ts` | Exit 0; 16 passed, zero failed, 724 assertions, 365.367 seconds. No filters. |
| `bun test scripts/harness/exec-fail-open-compact-flow.test.ts` | Exit 0; two passed, zero failed, 190 assertions, 357.985 seconds. No filters. |
| `python3 -u target/traffic-bun/task9-attempt4-audit.py` | Exit 0 after the fixtures. A separate read-only pass checked the recorded sizes and SHA-256 of all 47 Actor-local destinations; its output lists each path and digest. |

The receipt stream and every recorded stdout/stderr were rehashed against their receipt entries. Raw logs are ignored Actor-local files under `target/traffic-bun/`; they do not transfer through a tracked-file handoff. Key hashes:

| Actor-local file | SHA-256 |
| --- | --- |
| `task9-attempt4-checks.jsonl` | `d860724b8c59f1e80ebd503c583b844ea54b32316af59c0ec70f2503414702c3` |
| `task9-attempt4-audit-receipt.json` | `cdd7d390db878509680643fc485b4d3d7eecdadaa7c8de0a9675ac693c2295e1` |
| `task9-attempt4-restoration.stdout` | `7f9e40ae421fe6341da962b284bbb300501ef14dfd985ca5b7e37654c95e672f` |
| `task9-attempt4-recursive.stdout` | `491fd1e0bb69df263520f8b5d8fd6155e88b8c929a024b4c4a8929730eacc492` |
| `task9-attempt4-compact.stderr` | `d5af6effcb9f5c4909ec9ecf5dc97476a6930b2c56701afd04c95483610bbae2` |
| `task9-attempt4-compact-flow.stderr` | `e6d96f25cbc6f13f12119f6854e677eb599c2f54869937e741710777b3ef87ec` |
| `task9-attempt4-audit.stdout` | `992a3add52b00409a981998f88e250237e9e576ba72cbd3bbcb0f0ad81c009e4` |

The collector `task9-attempt4-checks.py` has SHA-256 `672596235d4d052879d718c3387f2fa690b65a06640e2de1f2be0cdd872cdf52`; the safety and audit scripts have hashes `423f0c4f241b091b038d94ce1908d6b6ac22b5bce9bef67e510322d600424bab` and `a5c0e7ae17881d17f0cf1bbee92ef4d756bce38700b5f2fa3ca02a762c2e08d5`. These are also Actor-local ignored files. `oma-qa` guided the scoped verification and evidence reporting; this is not the separately assigned independent product review.

### Task 8: final-workspace procedure

The integrated workspace must contain this tool and the unchanged historical table and collection. It needs read access to the historical Git object database and the task-19 shadow path named above and write access to its own `target/exec-fail-open`. From the final verification workspace, use a fresh log name and run the following. `set -C` refuses to replace an existing receipt; choose another fresh name if one already exists. The tool rejects missing or altered sources and differing existing destination files.

```sh
set -eu
set -C
mkdir -p target/traffic-bun
test "$(shasum -a 256 .arc-verify/traffic-bun/restore-predecessor.py | cut -d ' ' -f 1)" = 7bf1aec7a4c87ef52d7f85d7b10960585c9bf63bdee4263f5754740cf2af29b4
python3 -u .arc-verify/traffic-bun/restore-predecessor.py --workspace-root "$PWD" > target/traffic-bun/task9-final-restore.stdout 2> target/traffic-bun/task9-final-restore.stderr
python3 - <<'PY' > target/traffic-bun/task9-final-audit.stdout
from pathlib import Path
import hashlib, json, re, stat
root = Path.cwd()
report = (root / '.arc-verify/traffic/predecessor-recovery.md').read_bytes()
assert hashlib.sha256(report).hexdigest() == '78f8a822b4fb1296eb264fcdd6618582a9018de900d94552399ce7b7d20c02ea'
text = report.decode()
a = text.split('## Restored task-13 archive\n', 1)[1].split('## Restored task-19 frozen inputs\n', 1)[0]
f = text.split('## Restored task-19 frozen inputs\n', 1)[1].split('## Remaining prerequisite', 1)[0]
archive = re.findall(r'^\| `([0-9a-f]{64})` \| ([0-9]+) \| `([^`]+)` \|$', a, re.M)
frozen = re.findall(r'^\| `([^`]+)` \| ([0-9]+) \| `([0-9a-f]{64})` \|$', f, re.M)
assert len(archive) == 26 and len(frozen) == 13
items = [('archive', root / f'target/exec-fail-open/task13-preserved/archive/{h}.json', int(n), h) for h, n, _ in archive]
for p, n, h in frozen:
    items.append(('frozen', root / 'target/exec-fail-open/task19/frozen' / p, int(n), h))
    if Path(p).parts[:2] == ('target', 'exec-fail-open'):
        items.append(('original-location', root / p, int(n), h))
counts = {'archive': 0, 'frozen': 0, 'original-location': 0}
for kind, path, size, expected in items:
    info = path.lstat()
    assert stat.S_ISREG(info.st_mode) and info.st_size == size
    with path.open('rb') as stream:
        actual = hashlib.file_digest(stream, 'sha256').hexdigest()
    assert actual == expected, str(path)
    counts[kind] += 1
    print(json.dumps({'kind': kind, 'path': str(path), 'bytes': size, 'sha256': actual}, sort_keys=True))
assert counts == {'archive': 26, 'frozen': 13, 'original-location': 8}
print(json.dumps({'status': 'passed', 'counts': counts}, sort_keys=True))
PY
bun -e 'import {reference} from "./scripts/harness/exec-fail-open-inputs"; import {canonicalArchive} from "./scripts/harness/exec-fail-open-compact"; const root=process.cwd(); const p=".arc-verify/exec-fail-open/implementation/collection-next.json"; const entries=canonicalArchive(root,reference(root,p)); console.log("validated top-level archived selectors:",Object.keys(entries).length);' > target/traffic-bun/task9-final-recursive.stdout 2> target/traffic-bun/task9-final-recursive.stderr
shasum -a 256 target/traffic-bun/task9-final-restore.stdout target/traffic-bun/task9-final-restore.stderr target/traffic-bun/task9-final-audit.stdout target/traffic-bun/task9-final-recursive.stdout target/traffic-bun/task9-final-recursive.stderr
```

Check that the restore stream ends with `status: passed` and counts 26/13/8, the independent audit ends with the same counts, and the recursive validator reports 12 top-level selectors. The final-gate owner should capture exit codes and hashes for its own receipts and run its required full gates. The two exact unfiltered fixture commands are listed above; the fixture runtimes here show why wrappers must allow the full mission timeout. If access to either authenticated source is unavailable or a digest differs, report that prerequisite failure without inventing bytes. The deleted nested-dependency manifest remains unavailable; the unchanged fixture now authenticates the nested dependencies from predecessor containers, and both full fixture files passed here without that manifest.

The exact shell block above was extracted and replayed in this Actor checkout with `bash target/traffic-bun/task9-attempt4-handoff.sh`: exit 0. The second restoration verified 47 `existing-identical` destinations, the separate audit checked all 47, and the recursive validator again reported 12 top-level selectors. The Actor-local script has SHA-256 `466e5cec60e2939606580f6ab4e92cc7af7aaba0e0445eb5d0b8f2435e1d5d41`; its stdout is `7fcb1353f6c8659ba88ffefa45c66ad3c88df0d264e29d273706454b71d6f0cb`, and the replayed restoration stream is `e9ac6262e71bb2ab040a138c0c14a83172af50508062a6d5eee5c7dcb1e46114`. These replay files are under the Actor-local `target/traffic-bun`, not the final workspace.

The final-workspace restoration, integrated product QA, mixed Cargo–Bun measurement, coverage and full harness are outside this Actor-local result. Full Git status in this Actor checkout prints `av.yaml: Operation not permitted`; its state remains unknown. Serena tools were not exposed, so this task read the named files directly. The independent product review remains a separate assignment.


## Attempt 2 — current result

Status: **partial; the mission source workspace still denies the required writes**. On 11 November 2026, a direct `mkdir -p SOURCE/target/traffic-bun` returned `Operation not permitted`. The authenticated restoration script was then run from `SOURCE` and exited 1 with `PermissionError(1, 'Operation not permitted')` before installing its first archive entry. `SOURCE/target` remained absent after the checks. Consequently none of the 26 archive entries, 13 frozen files or eight original-location copies was restored to the final-gate workspace. No existing destination was overwritten. The checks below are current failure evidence, not proof of successful recovery.

Here `SOURCE` is `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/59981caa-53a9-40cb-b3a1-3c3dfbb22b09/workspace`, and `ACTOR2` is `SOURCE/.agent-valley/parallel/workspaces/1f5006e3792c6ab1-e34d3152baeb112c-2`. The earlier attempt's `ACTOR` in the section below means the separate `...-1` workspace.

### Provenance and authenticated inputs

- `SOURCE` had HEAD `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`, with other Actors' uncommitted product changes and untracked mission reports. This does not place those changes in the committed baseline. Full Git status also reported `av.yaml: Operation not permitted`.
- `ACTOR2` had initial and inspected HEAD/snapshot commit `88b51f24dd66f564310b151b9e35adaa7732a49f`, tree `a0e3f7e3bbabdd944b910b78f9dd8ffe12037937`. The snapshot ref resolved to that commit in `SOURCE`. The worker ref was not yet available there at inspection; no worker revision is inferred. The contract and this report were tracked in the Actor snapshot, but absent from the accepted baseline and untracked in `SOURCE`. This attempt updates the Actor-local report; it is not a committed-baseline change.
- The fresh source audit verified all **24** blob-backed rows, both inline rows and all **13** frozen rows against the sizes and SHA-256 values in `SOURCE/.arc-verify/traffic/predecessor-recovery.md`. That report's SHA-256 was `78f8a822b4fb1296eb264fcdd6618582a9018de900d94552399ce7b7d20c02ea`; the selected collection was `3bad816a4f054f1f52fbbb7479a278c89a119f35fb8f0c074dc8166a9cb246ac`. The row-by-row source paths, blob IDs, sizes and hashes are in the hashed audit output below and the unchanged historical table.
- The original container was read from `/Users/gracefullight/workspace/arc/.agent-valley/maintenance-logs/task19-shadow-workspace/target/exec-fail-open/task19/frozen/target/exec-fail-open/task13-preserved/collection-next-original.json`: 152,867,036 bytes, SHA-256 `4456ce62e9949a3a8e1b208983ec67318cf0ff73e596c76640f7a9149f6e9366`. After authenticating this container, the audit checked its two inline selections: `2cf4dd7f4b009e99e6bc48c6a02c110fa1e23d0df4db2b1631d814335932b14e` (39,113 bytes) and `9949863a0a81dbf0db11927cbc3f37ad459207a0e207dc239becbe92170785e8` (5,199,145 bytes). The 24 blobs came from the historical repository named in the preceding report; their presence in its object database does not establish reachability from that repository's HEAD.
- `git diff --exit-code 16772fe2aa350413225f3fb2bb9227d14ae5a6c4 --` for the named compact tests, fixture/validator modules, historical report and selected collection exited 0 in `SOURCE` after the run. The rest of the source workspace was not claimed clean.

### Commands and observations

All five commands below ran serially with `SOURCE` as cwd. The collector imposed no time limit, so it did not impose a shorter timeout than the mission's 7,200,000 ms per-check allowance. The recursive invocation is reproduced in the attempt-1 section; the exact argv, cwd, timestamps, durations, exit codes and both stream hashes for this attempt are in the JSONL receipt.

| Command | Exit | Current observation |
| --- | ---: | --- |
| `python3 -u ACTOR/target/traffic-bun/task9-attempt1-source-audit.py` | 0 | The 24 blobs, two inline selections and 13 frozen files matched their recorded hashes and sizes. |
| `python3 -u ACTOR/target/traffic-bun/task9-attempt1-restore.py` | 1 | `PermissionError(1, 'Operation not permitted')`; no destination was installed. |
| `bun -e '<reference and canonicalArchive invocation below>'` | 1 | Missing `target/exec-fail-open/task13-preserved/archive/094b5ff84e9d11b19878d7b00a1d4ef8334fc46dfb687d7b77cec5bdad0f7fb7.json`. |
| `bun test scripts/harness/exec-fail-open-compact.test.ts` | 1 | Full file: 7 passed, 9 failed, 106 assertions. Failures include the guarded, latest, attempt-12 and attempt-13 rotations and the previously filtered cases; each stopped on a missing preserved archive, original or frozen input. |
| `bun test scripts/harness/exec-fail-open-compact-flow.test.ts` | 1 | Full file: one synthetic flow passed and the combined predecessor-dependent flow failed on the missing `094b5f…` archive. |

An optional `ps` probe to inspect the running test was denied with `operation not permitted`; the process completion and exit were observed through the command session instead. No GUI, browser, Playwright or alternate route was used to bypass the write denial.

### Attempt-2 raw receipts and handoff

The raw files are under `ACTOR2/target/traffic-bun/`, an ignored Actor-local directory. They must not be assumed to transfer to `SOURCE` during integration. The collector `task9-attempt2-checks.py` has SHA-256 `2da4b9e17db84d415beb4c8d9fc93fe8eb389184b497e19e5bc5d65d6e6a18e3`; its JSONL receipt `task9-attempt2-checks.jsonl` has SHA-256 `9e85509fa25e040fd71f2b3da5c4b7be4603dc68a712b2a31e02527524adadf9`. It binds every raw stdout/stderr by path, byte count and SHA-256. The principal streams are:

| Raw file | SHA-256 |
| --- | --- |
| `task9-attempt2-source-audit.stdout` | `b346b3eeb358059808ced5e137bd1d6e1d7dcafadafc646945425b32938903c8` |
| `task9-attempt2-restoration.stdout` | `3a4783bac4687815843b3faf4295df1560afde4caf2364d75148f5ec3216fa3d` |
| `task9-attempt2-recursive.stderr` | `a18cff3d529758b7667ee1cca9cb5c62295dc3fb735ebb4b0157de33f0c23b19` |
| `task9-attempt2-compact.stderr` | `9f1b603454018725f0c53e31a1bb973a26c7b0e3d24e2ca737062cced7f9ffe8` |
| `task9-attempt2-compact-flow.stderr` | `e6d1e344dfba821afb601853b8bb7c0ca775ec27532c94cb9a61961514fa4934` |

For a supported runtime with write access to `SOURCE/target`, byte-identical copies of the prior scripts are retained as `ACTOR2/target/traffic-bun/task9-attempt2-source-audit.py`, SHA-256 `76b9dff974ff3e89e2a49c8ca4fddba74f770c59d766bc8a695cffd64a702b87`, and `task9-attempt2-restore.py`, SHA-256 `81f1d4572c8c26ad55b5d01e0037f74501034490518b61ad2c42f52d378e3c31`. The latter is fixed to `SOURCE`, verifies source sizes and hashes before installation, uses exclusive creation, rejects different existing files and symlinked parents, and rehashes each destination. In that runtime, capture its output under `SOURCE/target/traffic-bun`, require its `finish` event with `status: passed`, then rerun the recursive validator and both complete test files above without filters. Verify the resulting destination hashes and report any mismatch; do not recreate the deleted task-16 manifest. The current fixture's direct predecessor authentication has not passed in this attempt because the inputs remain absent.

`oma-qa` was used to structure the source-bound checks and report the failure accurately. This is not the separate independent product review. The full mission gates remain outside this task and were not run.

The final Actor-local check rehashed all ten streams listed in the JSONL receipt, checked the audit event counts, and confirmed the full-file test totals above. `git diff --check -- .arc-verify/traffic-bun/predecessor-recovery.md` passed. The Actor-local mission evidence directory remains below 30 files and 5 MB; the final integrated directory must be checked separately.

## Attempt 1 — retained account

Status: **partial; blocked on mission-workspace write access**. The retained sources were authenticated, but no predecessor inputs were restored to the mission source workspace. Its `target/` directory was absent and attempts to create it returned `PermissionError: [Errno 1] Operation not permitted`. The recursive validator and predecessor-dependent fixtures therefore remain failing. This report does not claim a full harness, Traffic acceptance, or independent product QA pass.

## Scope and source identity

- Mission source workspace (`SOURCE`): `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/59981caa-53a9-40cb-b3a1-3c3dfbb22b09/workspace`. During these checks, its HEAD was `16772fe2aa350413225f3fb2bb9227d14ae5a6c4`, tree `1834e0cccb50a12fea299618f9a360f4ca10e03f`. It also had other Actors' uncommitted product changes; this task did not edit them. The HEAD is not a claim that those changes are committed.
- This Actor workspace (`ACTOR`): `SOURCE/.agent-valley/parallel/workspaces/1f5006e3792c6ab1-e34d3152baeb112c-1`. Its initial and inspected HEAD is snapshot commit `6d923f520a2cdf5e8c39378648fc1e1223549707`, tree `0746d087d0a597b35493f501c6c8f45081606f5b`. The snapshot ref in the source repository resolves to that commit. The supplied worker ref was not present there when checked; no worker commit is inferred. `contract.md` is tracked in the Actor snapshot but absent from the accepted baseline; the source workspace had it as an untracked dependency. This report is an uncommitted Actor-local deliverable.
- The historical mission repository is `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace`. Its inspected HEAD was `00773c69a8e5e7fd417fd5cc78d03e222364f609`; the blob IDs below come from its object database and are not asserted to be reachable from that HEAD.
- `SOURCE/.arc-verify/traffic/predecessor-recovery.md` has SHA-256 `78f8a822b4fb1296eb264fcdd6618582a9018de900d94552399ce7b7d20c02ea`. Its tables record the 26 archive filenames, expected sizes and SHA-256 values, the 24 historical blob IDs, and the 13 frozen input paths and hashes. `SOURCE/.arc-verify/exec-fail-open/implementation/collection-next.json` has SHA-256 `3bad816a4f054f1f52fbbb7479a278c89a119f35fb8f0c074dc8166a9cb246ac`. Both match the accepted baseline and remained unchanged during the checks.
- The named archive fixture and test files were unchanged from the accepted baseline. Their individual hashes and before/after observations are in the check receipt below. Scoped source status showed other Actors' changes; full status remains limited by `av.yaml: Operation not permitted`. No Git index, product file, migration, or historical evidence was changed by this task.

## Authenticated sources and missing destinations

A fresh read-only audit streamed all **24** tabled blobs through `git cat-file blob` in the historical repository and checked each expected size and SHA-256. It also verified the retained original at

`/Users/gracefullight/workspace/arc/.agent-valley/maintenance-logs/task19-shadow-workspace/target/exec-fail-open/task19/frozen/target/exec-fail-open/task13-preserved/collection-next-original.json`

as 152,867,036 bytes and SHA-256 `4456ce62e9949a3a8e1b208983ec67318cf0ff73e596c76640f7a9149f6e9366`. After authenticating that container, the audit decoded and checked these two inline entries:

| Original logical path | Bytes | SHA-256 |
| --- | ---: | --- |
| `.arc-verify/exec-fail-open/implementation/commands/assemble.json` | 39113 | `2cf4dd7f4b009e99e6bc48c6a02c110fa1e23d0df4db2b1631d814335932b14e` |
| `.arc-verify/exec-fail-open/implementation/rejected-attempt10-collection.json` | 5199145 | `9949863a0a81dbf0db11927cbc3f37ad459207a0e207dc239becbe92170785e8` |

All **13** tabled files under the retained task-19 shadow `.../target/exec-fail-open/task19/frozen` matched their recorded sizes and hashes. The original appears among those 13 and was checked twice. The raw audit records every source path/blob ID, size and SHA-256; it did not write an archive file.

The required destinations remain absent at `SOURCE/target/exec-fail-open`: the 26 files at `task13-preserved/archive/<SHA-256>.json`; the preserved `task13-preserved/collection-next-original.json`; the 13 files under `task19/frozen/`; and the additional original-location copies of the audit, four prior-canonical files and two reviewer streams. The frozen original supplies the separate preserved original, making eight additional original-location files in total. No existing bytes were overwritten. The deleted `task16-attempt3/nested-fixture-dependencies.json` was not recreated; the current unchanged fixture source authenticates nested dependencies directly from the predecessor containers, but its production check did not reach completion here.

## Checks in the mission source workspace

Bun was `1.4.4` on Darwin arm64. Tests below were individually filtered to the named fixture; each reported 15 other tests filtered and is not a full-suite pass. The commands were run serially with a 7,200,000 ms allowance per check. `SOURCE` is the working directory in every row.

| Command or fixture | Exit | Observation |
| --- | ---: | --- |
| `bun -e '<reference and canonicalArchive invocation below>'` | 1 | `ENOENT` on `task13-preserved/archive/094b5ff84e9d11b19878d7b00a1d4ef8334fc46dfb687d7b77cec5bdad0f7fb7.json`. |
| `external archives authenticate persistent bytes and cannot become current evidence` | 0 | One synthetic test passed, 14 assertions. |
| `task-13 rotation externalizes archival entries from the original inline container` | 1 | Missing `task13-preserved/collection-next-original.json`. |
| `correction rotation retains the staged predecessor with a different inventory` | 1 | Missing archive `609e4132add8ff92af8dd88d32d82c7546df6da466cfc72128a96b3e3be4d419.json`. |
| `correction rotation preserves the previously rotated predecessor` | 1 | Missing archive `094b5ff84e9d11b19878d7b00a1d4ef8334fc46dfb687d7b77cec5bdad0f7fb7.json`. |
| `accepted QA transition binds prior bytes and rejects unrelated execution changes` | 1 | Missing `task19/frozen/.arc-verify/exec-fail-open/final.json`. |
| `production gates require the current combined archive and authenticated predecessors` | 1 | Missing archive `094b5ff84e9d11b19878d7b00a1d4ef8334fc46dfb687d7b77cec5bdad0f7fb7.json`. |

Each named fixture command was `bun test scripts/harness/exec-fail-open-compact.test.ts -t '<fixture name>'`. The recursive validator invocation was:

```sh
bun -e 'import {reference} from "./scripts/harness/exec-fail-open-inputs"; import {canonicalArchive} from "./scripts/harness/exec-fail-open-compact"; const root=process.cwd(); const p=".arc-verify/exec-fail-open/implementation/collection-next.json"; const entries=canonicalArchive(root,reference(root,p)); console.log("validated top-level archived selectors:",Object.keys(entries).length);'
```

A direct `python3 -c` invocation of `os.mkdir('SOURCE/target', 0o755)` exited 1 with the permission error above. The empty stdout has SHA-256 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`; the actual stderr is `ACTOR/target/traffic-bun/task9-attempt1-permission.stderr`, SHA-256 `ed43c02462208c5567b27cebfa0629c89736e4d7dc1a9a4d0556b933cd757ecf`. `SOURCE` in the command is the full path defined above, not a literal environment variable.

## Handoff and raw receipts

The raw files below are **Actor-local ignored files**, not committed evidence and not assumed to transfer through Actor integration. Their paths must be read from this retained Actor workspace while it is available:

- `ACTOR/target/traffic-bun/task9-attempt1-source-audit.py`, SHA-256 `76b9dff974ff3e89e2a49c8ca4fddba74f770c59d766bc8a695cffd64a702b87`; its stdout `task9-attempt1-source-audit.stdout`, SHA-256 `17f10be007d10697e60c9dfed492a942dd9e05803b47efaa99e3c51d919977b1`, records all source checks. The command `python3 -u target/traffic-bun/task9-attempt1-source-audit.py` exited 0 from `ACTOR`.
- `ACTOR/target/traffic-bun/task9-attempt1-checks.jsonl`, SHA-256 `38185f5e2567a818e4040647f6586299a7e133219c03cc8fd2a341c3ec557a6b`, records argv, cwd, duration, exit and SHA-256 for each raw stdout/stderr. Its runner `task9-attempt1-checks.py` has SHA-256 `aa0db8f60a6bbfda87e189c73afeead61e728a04fbe7b91ac327a9d75174354e`; `python3 -u target/traffic-bun/task9-attempt1-checks.py` exited 0 as a collector while recording the individual failures above.
- `ACTOR/target/traffic-bun/task9-attempt1-restore.py`, SHA-256 `81f1d4572c8c26ad55b5d01e0037f74501034490518b61ad2c42f52d378e3c31`, is a prepared restoration script, **not run against SOURCE**. It uses the recorded Git blobs and shadow inputs, checks sizes and hashes before and after writes, rejects different existing bytes and symlinked parents, and writes only under `SOURCE/target/exec-fail-open`. Its synthetic checks for new files, identical existing files, different existing bytes, symlinked parents and digest mismatch exited 0; `task9-attempt1-restore-safety.stdout` has SHA-256 `8c9f85bc32fcc60cac13afdac2868d7c6fa690f9d686dd79223218360ba902b0`. This synthetic check does not demonstrate restored production inputs.

The final-gate owner needs a runtime with write access to `SOURCE/target` and read access to the retained Actor workspace. From `SOURCE`, the prepared recovery can be run and recorded with:

```sh
mkdir -p target/traffic-bun
set -o noclobber
python3 -u /Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/59981caa-53a9-40cb-b3a1-3c3dfbb22b09/workspace/.agent-valley/parallel/workspaces/1f5006e3792c6ab1-e34d3152baeb112c-1/target/traffic-bun/task9-attempt1-restore.py > target/traffic-bun/task9-restoration.stdout 2> target/traffic-bun/task9-restoration.stderr
restore_status=$?
shasum -a 256 target/traffic-bun/task9-restoration.stdout target/traffic-bun/task9-restoration.stderr
test "$restore_status" -eq 0
```

Check the Python exit status and the final event before treating the recovery as complete. Verify all destinations, then rerun the exact recursive validator and the six named fixture commands above. If a source or destination differs, the script fails without overwriting the differing file; report the mismatch rather than reconstructing it. The full mission gates remain the final-gate owner's responsibility.

`oma-qa` guidance was used for the scoped recovery checks and truthful reporting. This is not the separate independent `oma-qa` review required for the integrated product.
