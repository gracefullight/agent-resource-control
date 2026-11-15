# Python TEST contract
Baseline 6b94c0bd3b309ae7b5d7ce4c3ebef50b62b5e85c;macOS Python TEST:DET-003/004/008,TRF-001/002/005. Inherit .arc-verify/cycle-22/contract.md amendment 15. Directors:terminal-first/reuse;no redesign/retention/marketing/publication.

## Grammar
Braces expand literally.
Native argv only;literal API/compounds keep protocol-1 UNKNOWN/OBSERVE. Exe basenames={pytest,python,python3,python3.{8,9,10,11,12,13,14,15}},bare/relative/absolute path,no trailing slash;e.g .venv/bin/python. Pure lexical:no I/O/lookup/probe/execute/import. Original OsString exe/argv/import semantics unchanged. Prefix exactly -m pytest|-m unittest;no interpreter flags.
Pytest:empty=discovery;non-option path/dir[::ID...],all components nonempty,parameters opaque.
0-arity:-q,--quiet,-qq,-v,--verbose,-vv,-x,--exitfirst,-s,--disable-warnings,--collect-only,--co.
1-arity separate:-k EXPR,-m EXPR(opaque,spaces allowed).
1-arity separate/--name=value:--maxfail(positive u32 decimal),--tb(auto/long/short/line/native/no),--capture(fd/sys/no/tee-sys),--color(yes/no/auto).
Unittest:empty=discovery;selectors=ID(.ID)* (ID=[A-Za-z_][A-Za-z0-9_]*) or relative/absolute .py files;discover first switches to options-only.
Both modes 0-arity:-v,--verbose,-q,--quiet,-f,--failfast,-b,--buffer,-c,--catch;1-arity separate:-k PATTERN.
Discover 1-arity:-s/--start-directory DIR,-p/--pattern GLOB,-t/--top-level-directory DIR;shorts separate,longs separate/=value;defaults retained.
Options/selectors interleave except discover;repeats allowed;values nonempty,not starting '-'. Other flags/clusters/attached shorts,--,help/version,modes,shorthand,malformed/ambiguous/missing values,empty token/non-UTF-8/NUL/over-budget=>whole UNKNOWN/OBSERVE,non-blocking bypass,no queue/reservation.
Budget bytes/tokens/invocations/depth/work=16384/1024/128/8/65536;charge scans first,checked arithmetic,existing reasons/counters.

## Seams
P=traffic.python-tests;admission P.{capabilities,job}/P.v1;diagnostics P.diagnostics.{capabilities,job}/P.diagnostics.v1 . PYTHON_PYTEST(console/module),PYTHON_UNITTEST=>TEST/HOST_LOCAL/NONE. GovernorJob{Request,Result}/Diagnostic{Request,Result} unchanged;protocol1,null probes,auth/ID/version/cap checked before enums. Legacy Python=>UNKNOWN/UNKNOWN/UNRESOLVED_COMMAND/OBSERVE(lifecycle/classification;diagnostics admission=null);old admission rejects Python;DB unchanged. Unsupported=>legacy observe;malformed=>error;uncertain=>same ID/witness,no replay;fallback original deadline.
Reuse existing TEST/global budget/Governor/managed execution/SQLite writer/Meter;no new deps/schema/scheduler/writer.
T1 Chief/oma-pm:this file,no gates. T2-5 backend-engineer/oma-backend. T2:detector/Traffic/CLI/IPC/daemon interfaces/projections,Rust exporter/generated TS/wire. T3 backend-native:native/repairs;T4 backend-desktop:decode/registered tests. T5 backend-gates:exclusive repairs/gates/docs/help/traceability/gates.json,sole manifest/lock writer. T6 independent-reviewer/oma-qa:read-only qa.json,no native gates. T2 before T3/T4.
D=crates/arc-daemon/tests;U=apps/desktop. T3 D/traffic_python_tests/mod.rs API:Read=fn(&Path,&str,Value)->Value,Peer{Cargo,Bun},semantics(Read),lifecycle(Read),one_test_slot(Read,Peer,bool),direct_read;true=Python first. Read:traffic_job_diagnostics/meter_summary;T4 registered Tauri handler;reuse Case/owned cleanup/serial.

## Tests
T2 crates/arc-detector/tests/python_tests.rs:
python_entrypoints_and_core_selectors_are_test
unresolved_python_forms_observe_within_budget
python_classification_never_executes_or_imports
Grammar/venv/spaces/bytes/limits/malformed/poison exe-import sentinels/literal-legacy parity.
T3 D/traffic_python_tests.rs:
native_python_execution_semantics
native_python_lifecycle_and_compatibility
Matching installed Python/pytest;own offline projects/venv;missing prerequisites fail,no downloads/skips/fakes;fixture-only plugin/config isolation,ARC child coverage.
Pytest console/module+unittest discovery/selectors:direct parity(success/failure,stdin/out/err,argv,cwd/env/imports,signals).
Governor pause/resume;fresh Critical;queued cancel(0 starts);nested permit/Meter lineage;lost enqueue/start/finish ACKs;restart;bounded outage fail-open. Real-daemon/synthetic faults separate;body-entry/ACK/release barriers/clocks,coverage-deterministic;no deadline/permit edits.
D/traffic_python_tests.rs and U/src-tauri/tests/traffic_python_tests.rs each:
python_then_cargo_one_test_slot
cargo_then_python_one_test_slot
python_then_bun_one_test_slot
bun_then_python_one_test_slot
Real cargo test/bun test:second starts=0 before release,TEST permits<=1 throughout,each executes once,all release. SQLite/CLI/registered bridge:same ID/class/outcome/all permit facts+Meter/global budget.
T4:old-client/new-daemon,new-client/old-daemon,unsupported fallback,wait/unconfirmed/run/cancel/bypass/unknown,stale reads,keyboard/focus/polite announcements;refresh read-only,Unknown!=success/zero;no GUI/browser.

## Gates
Root cwd;6 gates(active-22 Bun);empty verifyCommand waives none:
```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bun test apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/governor-contract.test.ts apps/desktop/tests/governor.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts apps/desktop/tests/traffic-scripts-contract.test.ts apps/desktop/tests/traffic-scripts.test.ts scripts/coverage/accounting.test.ts scripts/coverage/desktop-foundation.test.ts scripts/coverage/evidence.test.ts scripts/coverage/gate.test.ts scripts/coverage/hygiene.test.ts scripts/coverage/terminal-outcomes.test.ts scripts/coverage/traffic-accounting.test.ts scripts/harness/coverage-reuse.test.ts scripts/harness/equivalence.test.ts scripts/harness/traffic-targets.test.ts scripts/requirements/traceability-parser.test.ts scripts/requirements/traceability.test.ts
bun run test:coverage
./scripts/harness/validate.sh
```
Also expand `bun run desktop:{typecheck,check,build,smoke:tauri}`;`bun scripts/requirements/traceability.ts`.
T5 docs/managed-exec.md/CLI:grammar/examples/status/Meter;docs/requirements/traceability.md:rows/mappings/counts;DET-003/TRF-001 remain partial.
T5 reuses current-input AV integrationCheck successes(argv/cwd/pins/hash),no successful duplicates. Final code/docs -> exclusive T5 gates -> independent T6 QA. T2 adds this contract to scripts/coverage/config.ts FROZEN_INPUTS;gates/QA excluded. One final successful production-line run,>=80% independently for C=arc-{core,detector,storage,ipc,daemon,cli,traffic,desktop,platform,meter,policy}. ARC_COVERAGE_RECEIPT=<run>/reuse.json feeds unchanged harness;SKIP_COVERAGE unset;changed inputs invalidate reuse.
Only .arc-verify/python-tests/{contract.md,gates.json,qa.json},regular/nonignored,caps8000/40000/20000 bytes;no validators/checker/report tasks. Git-visible receipts:argv/cwd/time/exit/input+artifact hashes,case/job/permit facts;no ignored-only handoff.
gates.json true:sixGates.passed,desktop.{typecheck,check,build,nativeSmoke}.passed,coverage.{perCrate.<C>.passed,harnessReuse.{passed,sameInput},harnessUnchanged},inputBinding.{finalInputsMatch,pythonContractIncluded},documentation.traceabilityReconciled. coverage.threshold=80;coverage.successfulFinalInputCollections=1;reuse.actorDuplicateSuccessfulChecks=0;delivery.evidenceFiles=["contract.md","gates.json","qa.json"].
qa.json true(oma-qa/independent-reviewer):review.complete,inputBinding.{gatesMatched,finalInputsMatched},sequencing.backendGatesCompletedFirst,documentation.traceabilityReconciled,scope.pythonTestSliceOnly.
Preserve state/assignments/WIP/.agents/denied av.yaml;repair failures;external-only blockers.
Limits:3 tasks/172800s/400runs/3retries/1000ms;inherit absolute deadline/cancellation;checks/replays >=7200000ms(not120000 estimates). Serena absent;scoped reads only;runtime unverified.
