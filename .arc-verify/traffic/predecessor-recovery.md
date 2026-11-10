# Historical predecessor recovery

## Operator decision, 10 November 2026

The operator confirmed that `target/exec-fail-open/task16-attempt3/nested-fixture-dependencies.json` was deleted with an earlier workspace and cannot be recovered. The three archive fixtures now obtain their external dependencies from the retained predecessor containers and authenticate each selected byte sequence against its recorded size and SHA-256. The repeated-rotation fixture still checks that a nested-only dependency exists and rejects missing or altered nested bytes. No replacement manifest was created, and the retained historical receipts were not changed. The earlier status and recovery account below describe the original investigation, before this decision.

Status: **partial**. The task-13 archive chain and the 13 byte-pinned inputs consumed by the task-19 frozen QA fixture have been recovered in this Actor worktree. The recursive archive validator, the original rotation fixture and the frozen QA-transition fixture pass here. The separate nested-dependency manifest remains unavailable, and a production-archive fixture fails on that missing input. The full historical or Traffic harness has not been claimed to pass.

## Source identity and scope

- Accepted committed baseline: `4d2f71c1a1482925f6c9918296d7ceb3435b7592`, tree `4455896fdf976244313a70a9fa38b730b344b411`. Its `.arc-verify/exec-fail-open/implementation/collection-next.json` has SHA-256 `3bad816a4f054f1f52fbbb7479a278c89a119f35fb8f0c074dc8166a9cb246ac`, identical to the current file.
- This Actor's initial checkout is snapshot commit `4e3c53449e3b7d489c5fe0a7ca372ff15130647e`, tree `004408dcbd3195a93b5dcd9317bf70fcff233602`. The supplied snapshot ref resolves in the mission source repository to that commit; no worker ref was available there at inspection. These ignored `target/` files are Actor-local additions and are not in the snapshot or accepted commit. Do not assume an integration receipt transfers ignored files.
- The historical mission source repository is `/Users/gracefullight/workspace/arc/.agent-valley/operations/cad11807-cfad-4f23-9297-6c4a12f033b4/workspaces/025db4e3-dd16-4743-ba66-93110c401133/workspace`. Its inspected HEAD is `00773c69a8e5e7fd417fd5cc78d03e222364f609`, tree `b98f996caa04fe13eeeeab1072c3c528f6d18c65`; it has staged modifications. Its local Git object database is the source for the objects listed below. Presence of a blob in that database does not establish that it was committed by that HEAD.
- Historical evidence refs establish four concrete tracked-container sources at path `.arc-verify/exec-fail-open/implementation/collection-next.json`: snapshot `3406d93686984399b418e100ef15ee02b7b5c934` tree `6b899dc4c95230cbacd0ce0b2171920630802079`, blob `126f449d9b073d5e314ce86ef43c3de307bf53ff`; snapshot `6de0519272d7c8bc217f8730da415d187e04be3b` tree `12f1370853770f21ac41b6c6014fa31cf3d56ea4`, blob `b76addcd8af5bcd2c4f8eff7383035dc23fed0fd`; snapshot `6eb214fe11a55e14229bfad2b232b5ad2ceec02f` tree `7054cb4c23c9d4152b0b7b26351dedfbc9fcaf5d`, blob `18843171aed14e861b435c47b81e27f49565154d`; snapshot `11f3ae928f3f9c131ced71133a1bc3157a652fa2` tree `426c4770890d6757d5ff820564eae726730a56cd`, blob `3268049c2b1233e35b7d396abd1bfb4f1b13f8e0`. Each is an Actor snapshot ref, not a claim about the accepted baseline.
- The retained task-19 shadow source is `/Users/gracefullight/workspace/arc/.agent-valley/maintenance-logs/task19-shadow-workspace/target/exec-fail-open/task19/frozen`. Its `manifest.json` has observed SHA-256 `3579e50c2c3c2f3354892babe6fa300bf0ac5c4e25c91b5ada182d71f21b6631` and describes the 13 files below. I found no separately recorded historical digest for the manifest itself; it was not restored or presented as independently authenticated. The 13 inputs are bound by the committed fixture, the pinned final/review, the audit, or their authenticated inputs and were independently checked against their source bytes.
- No OMA domain skill was used: this task was evidence recovery, not an architecture decision. The shared execution policy and i18n rule were read. Serena was not exposed; named files and non-code evidence were inspected directly. `git status --short` reported `av.yaml: Operation not permitted`; status and diff excluding that path were initially clean. That file was not opened or bypassed.

## Restored task-13 archive

The accepted baseline container selects 12 external archive entries. Its three authenticated predecessor containers add 14 distinct external entries, for **26 unique files** across four container levels. Every restored file is a regular file at `target/exec-fail-open/task13-preserved/archive/<SHA-256>.json`; size and digest were checked before writing and rechecked afterward. The table lists the Git blob IDs actually read from the historical repository. The two inline entries were extracted from the authenticated 53-entry original container, itself blob `bf617511ec535937e756c3b9410b712830fb9a66`; their sizes and SHA-256 values matched the accepted/nested selectors. The object database includes objects not reachable from the available refs, so only the four snapshot mappings above are represented as tracked snapshot evidence.

| SHA-256 and archive filename | Bytes | Historical source blob |
| --- | ---: | --- |
| `094b5ff84e9d11b19878d7b00a1d4ef8334fc46dfb687d7b77cec5bdad0f7fb7` | 45723 | `1b5bd4eb6a4c059fa1ba8c5572f3eabf62341b1d` |
| `121f7bc6cb2bd278782e2a5dff800b28530d2153a87f857b90a497013b71d298` | 44777 | `d1d2d9494e4f813d6ee4dc1dd8cc443f8237abe7` |
| `13754eff63170ea76e7376fe477f992aa24a9762ed1300f2ae6743cbaa3d36fc` | 38114 | `022105699a2b085a7e2c305efe5679ee822d92c0` |
| `14a49423f7f53f3cd39f5b0462ee105bc6a12e45f36423447857da3590f07bc2` | 45467 | `79b9495860853480734d48d5976f5fb4e3715590` |
| `14bb652542229e85860db6f79003f541a0195e6e64a9fe89b268b2e98318282d` | 12170714 | `b76addcd8af5bcd2c4f8eff7383035dc23fed0fd` |
| `1567633dec40c085d0625abde7b909560ccc1f186fee8dc58378e8cc15d35261` | 28454803 | `18843171aed14e861b435c47b81e27f49565154d` |
| `1b09df71f845148508b3b4492af5150fcd314b91c0180d6457303e8cf5b7ae31` | 45716 | `c2db6fac588a9269e488c3b24289f35cab63067f` |
| `2b51241ae1bc5911533d41441eede185251a9473002be55a150e9bcbd915d626` | 76075 | `80e1986df606e579fc4402eea7f99794f14485d1` |
| `2cf4dd7f4b009e99e6bc48c6a02c110fa1e23d0df4db2b1631d814335932b14e` | 39113 | `inline in bf617511ec535937e756c3b9410b712830fb9a66` |
| `31cc55d3227f0476d00c4bdd4e11084ce1c6d38586c84b55385d6ed57ec8219d` | 105408 | `fd4c490ecf19bd0943b73e4c12cd0970fee988bd` |
| `4456ce62e9949a3a8e1b208983ec67318cf0ff73e596c76640f7a9149f6e9366` | 152867036 | `bf617511ec535937e756c3b9410b712830fb9a66` |
| `4b8fb51b81cd9c6596c553ebb80ca9bdf45387515df9884620912a800b627e21` | 105640 | `300e261c0f337a5957e695287ef6ea6787630d0f` |
| `609e4132add8ff92af8dd88d32d82c7546df6da466cfc72128a96b3e3be4d419` | 1243746 | `90ec033625c4f37c9744f1a359a5507ef98d882c` |
| `9949863a0a81dbf0db11927cbc3f37ad459207a0e207dc239becbe92170785e8` | 5199145 | `inline in bf617511ec535937e756c3b9410b712830fb9a66` |
| `a26d3ef1bc89a9be590d90b34911f38ed3490180b697e87e7a182f3d33c7dc04` | 1237949 | `e3eb9a512ac6dd0de52890d48b7246f2e4c27dca` |
| `a7dc262e8a600f7e53884fbf6a5946288eac711cb28af0459be570bf6d33816b` | 104690 | `8db3a614347093b63edd64475796e3d205f8d258` |
| `bb8fe609da5fe5b0f5eb237fc740a6bede7f94fc6845ca720aafaf8b9477db6d` | 38114 | `b569b41d3080d8ba2fefef2270b7fd31111f51dc` |
| `bd193ff2fa91e70b9de4c69ecc0c0866ebd8849897d397c92e75890117207882` | 76754 | `5fcec5576c60ba3ada1e18e0a899e62b124c8b7d` |
| `bf4ef2414086d1da6a99dc05b5c43dd82f82dcda004b92b0cf0994643e08ec22` | 76075 | `e27c86176e71d24cbd4e9d829551713b28ff6ef8` |
| `c17f5e67ccbdb71bd78f5e5d2e04a2412fbb1dd7a66df71ec3e17c20487a14dd` | 39140 | `9af9abbb742b5bbbaa8caf0e904771a6eb0a6148` |
| `cf4b46bee490dcce468f62701751b0b2e214380e13a4e3eea0e65d0200402324` | 2089564 | `126f449d9b073d5e314ce86ef43c3de307bf53ff` |
| `d23a70c9bb337a54be4d9169730fce34669c6910f9981442d14f7000510c121d` | 65514945 | `3268049c2b1233e35b7d396abd1bfb4f1b13f8e0` |
| `d26c3323dca5b9db06783ffa8d2c4a6f11310b24ae8f8a49717871e59f382050` | 38794 | `54faf6777ff1a8906d9c45c28e23f253bda4f2bc` |
| `d69b801f8950243d4379f025dc4486172779d8348c9426d06ad909cf6f3c9327` | 1278975 | `3a09a2ea7bbb5b7c701f9b3c2c43bf33b627e4a0` |
| `f0e14c76647d3da30e8e360eeeaef73cf2fefb1178c9a00a6526e830ee4a5603` | 77109 | `42ad609507261f36926d4d71394ec36f5c049ce0` |
| `f725d2631204511772f80756cb08595858a4d73b77b9cab21432ab97f866c2a2` | 104721 | `22a575494a2df6a980b8ef7b2849fb47b63cfd75` |

The exact original is additionally at `target/exec-fail-open/task13-preserved/collection-next-original.json`, 152,867,036 bytes, SHA-256 `4456ce62e9949a3a8e1b208983ec67318cf0ff73e596c76640f7a9149f6e9366`. This is the selector digest in the accepted collection and the value pinned by `scripts/harness/exec-fail-open-evidence.ts`; it is not inferred from the separate four-entry `canonical-archive.json`. The four prior canonical files are also restored at `target/exec-fail-open/task13-preserved/prior-canonical/` with the four SHA-256 values in the frozen table. They are bound by the authenticated historical QA receipt.

## Restored task-19 frozen inputs

Each path in this table is relative to `target/exec-fail-open/task19/frozen/`. All 13 files match the shadow source byte for byte, including size and expected SHA-256. The task-13 audit and the two raw reviewer streams were additionally restored at their original `target/exec-fail-open/...` locations using the same pinned digests.

| Relative path | Bytes | SHA-256 |
| --- | ---: | --- |
| `.arc-verify/exec-fail-open/final.json` | 223371 | `5fecbfc96353272b61732e8fe85b29177db7508a006adbf3140121b7e7bdcb8d` |
| `.arc-verify/exec-fail-open/qa/review.json` | 439490 | `c0b7cfb81f1de855436cab07ea32817bb2cc41e919a2c282451e013c9bc17dd4` |
| `.arc-verify/exec-fail-open/qa/review.md` | 123687 | `ded9cd8ef940dc46081ecedf6f3cc2659926d7e7a9391b812db4839d93d4b1a7` |
| `crates/arc-cli/src/main.rs` | 8815 | `860597efc78e542d91df152ec4db76dc387b19e2e901963d694de3ac9d2d3ba6` |
| `docs/managed-exec.md` | 5038 | `680abfe5fabd880542b16b6135e7c78206a05d4c9afae92065cfca36a45cfd7d` |
| `target/exec-fail-open/task13-preserved/archive-audit.json` | 6438 | `6c741b17e3679a9828781b30529f0dcd01aa207faa5939b09a2b439bb0a4e1b4` |
| `target/exec-fail-open/task13-preserved/collection-next-original.json` | 152867036 | `4456ce62e9949a3a8e1b208983ec67318cf0ff73e596c76640f7a9149f6e9366` |
| `target/exec-fail-open/task13-preserved/prior-canonical/gates.json` | 44744 | `b7b6c26811f1039685ade17cdfd089e2728521e0a86368a378a784cae399c124` |
| `target/exec-fail-open/task13-preserved/prior-canonical/timing.json` | 104626 | `43350a298e3e0ea391516ea6dd57883bf86e25bb7bdd59024512b83899dede8d` |
| `target/exec-fail-open/task13-preserved/prior-canonical/historical.json` | 76075 | `3aff3254dd1fd7d866448a2a2d02e57e0c56c3ae72d01e33014d5df99bef19ea` |
| `target/exec-fail-open/task13-preserved/prior-canonical/preservation.json` | 38111 | `f60e53e78f8b5a3914bcfb04d15506078946bdb3722902a8e4e3b7c19eafd8ce` |
| `target/exec-fail-open/task6-attempt4-independent/workspace.stdout` | 37439 | `3e852ef2e156a5787d3d18817d3cefc80804e510f32c418c5d1a932619030c7a` |
| `target/exec-fail-open/task6-attempt4-independent/workspace.stderr` | 6912 | `ccf4d8e6e09f873ccc119ea3ad58b59fb2e10c914ae4d4c526d33aeaaa08da4b` |

The source audit is SHA-256 `6c741b17e3679a9828781b30529f0dcd01aa207faa5939b09a2b439bb0a4e1b4`; it states that the original is exact, has 53 selections, and names the prior-canonical hashes. The review receipt pins the workspace stdout to `3e852ef2e156a5787d3d18817d3cefc80804e510f32c418c5d1a932619030c7a` and stderr to `ccf4d8e6e09f873ccc119ea3ad58b59fb2e10c914ae4d4c526d33aeaaa08da4b`. No historical receipt or report was changed.

## Remaining prerequisite and reproducible handoff

`target/exec-fail-open/task16-attempt3/nested-fixture-dependencies.json` remains **unavailable**. The historical report `.arc-verify/exec-fail-open/technical-verification.md:2761` records its required SHA-256 as `30eaa9b742c41d7b990ea0d9ae0b6a8e5dad62ac7b1ec9b389088694169c725f`. No matching file was found by name in the retained Agent Valley directories, by digest among the inspected smaller blobs in the historical Git object database, or in the inspected maintenance logs, reports, mission records and organization evidence. The four entries named in the historical report were restored as part of the verified chain, but their presence does not reproduce the missing manifest. No replacement manifest or historical observation was invented.

The Verification Engineer can copy the authenticated files from this Actor's retained `target/exec-fail-open/` directory into the integrated worktree, checking each digest against the tables before writing and refusing to overwrite different existing bytes. The original 13 frozen inputs are independently available at the shadow source above. If the Actor workspace is no longer retained, the Git object IDs and shadow source in this report identify alternative inputs; use `git -C <historical mission source repository> cat-file blob <blob-id>`, verify the table's SHA-256 and size before writing, and decode the two inline records from the verified original container. Verify the resulting integrated paths with the existing recursive validator and the focused fixtures listed below. Recover the exact missing manifest from an authenticated retained source, then rerun the production-archive fixture; any alternative validation design is outside this task and has not been established as equivalent.

## Checks and results

- `git rev-parse HEAD HEAD^{tree} 4d2f71c1a1482925f6c9918296d7ceb3435b7592^{tree}`: passed; values are listed above. Scoped status excluding `av.yaml` was clean before the report and restorations.
- Direct reads of the baseline container, the historical report and `exec-fail-open-compact-historical-fixture.ts`, `exec-fail-open-container.ts`, `exec-fail-open-compact.ts`, `exec-fail-open-compact-archive-fixture.ts` and `exec-fail-open-evidence.ts` established the selectors and checks. Git evidence refs and `git cat-file --batch-all-objects --batch-check` were inspected; 4,691 historical blobs smaller than 2 MiB were checked for the pinned frozen files and the missing manifest. The repo-root object database contained no blobs of the initial required archive sizes. The mission and old mission repositories did contain matching blobs.
- A bounded `find` of `/Users/gracefullight/workspace/arc/.agent-valley` for the audit, nested manifest and reviewer stream filenames found the task-19 shadow source, not the nested manifest; Git, build, dependency and incremental directories were pruned. A SHA-256 scan of 3,383 maintenance-log files plus 90 report, operation-report, mission and organization files, each at most 2 MiB, found no nested-manifest digest. These are bounded observations, not proof that no copy exists anywhere.
- `bun -e 'import {reference} from "./scripts/harness/exec-fail-open-inputs"; import {canonicalArchive} from "./scripts/harness/exec-fail-open-compact"; const root=process.cwd(); const p=".arc-verify/exec-fail-open/implementation/collection-next.json"; const entries=canonicalArchive(root,reference(root,p)); console.log("validated top-level archived selectors:",Object.keys(entries).length);'`: exit 0, 12 top-level selectors; the maintained validator also traversed the three nested predecessor containers and authenticated their external selections.
- `bun test scripts/harness/exec-fail-open-compact.test.ts -t 'task-13 rotation externalizes archival entries from the original inline container'`: exit 0, one passed, 30 assertions, 15 filtered.
- `bun test scripts/harness/exec-fail-open-compact.test.ts -t 'external archives authenticate persistent bytes and cannot become current evidence'`: exit 0, one passed, 14 assertions, 15 filtered. This is a synthetic negative fixture.
- `bun test scripts/harness/exec-fail-open-compact.test.ts -t 'accepted QA transition binds prior bytes and rejects unrelated execution changes'`: first exited 1 on missing frozen `archive-audit.json`; after its hash-verified restoration, exit 0, one passed, eight assertions, 15 filtered.
- `bun test scripts/harness/exec-fail-open-compact.test.ts -t 'production gates require the current combined archive and authenticated predecessors'`: exit 1, missing `target/exec-fail-open/task16-attempt3/nested-fixture-dependencies.json`; one failed, 15 filtered. This remains a concrete blocker for that historical fixture.
- A direct read-only comparison confirmed all 13 frozen inputs match the shadow source and each listed size and SHA-256. These checks are not the requested current-source Traffic acceptance, independent QA, coverage gate or full harness.
- Final verification rehashed all 26 archive files and all 13 frozen inputs, confirmed the nested manifest is still absent, and checked the report for trailing whitespace. `git diff --check -- . ':(exclude)av.yaml'` passed. Scoped Git status showed only this untracked report; `git check-ignore` confirmed the restored `target/` inputs are ignored.
