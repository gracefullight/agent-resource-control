# Cycle 22 amendment 15
Base 8b7ca428e5f5aec8a40bd23922e3b2b7edf225cc. Recovery retained;index preimages PASS. SHA256:
cycle22-task1-clean.patch 4ba21e4c4dc168ead4be453dfadecc6c381ba65d2a150d546814191baaee6ecc
cycle22-task1-untracked.tar 95f3bd7b6643a4871ac5ba17d5d4da8eac05068d32194b48bd5d9a2910c32036
cycle22-omitted-files.txt 5c740756e25e28126ac22764f1957fe370a1afcfbd4703a0bc399174274b5d28
Payloads:contract.md,scripts/archive/job-meter/evidence.test.ts(4a3d9bd33cf525180fcd1346ba0247c122e5bbd78a9ede405d6d3535cf23a53b;archived/bunfig-excluded).
Chief:repair -> same backend gates -> QA. Technical/Design/Marketing advice:retain stack,seams,outcomes/journeys. Skills oma-backend/oma-debug/oma-frontend. Serena absent;scoped reads/diffs;global unknown;av.yaml untouched.

## Timing inventory
C/D/I/M=crates/arc-{cli,daemon,ipc,meter};U=apps/desktop;T=U/src-tauri;X=C/src/exec;Y=D/src/db_worker;B=T/tests;H=D/tests/support;R=C/tests/job_accounting;Z=B/job_meter_acceptance;F=B/daemon_bridge_support. .rs implicit;#=test prefix;{} expand;oN=omitted.
- X/preparation#deadline;o17#run;X/transport/preparation_tests:1800ms across fallback/REGISTER/CLAIM;expiry/SIGTERM ACK=>NOT_STARTED/1/no child.
- I/src/client/{live_tests,repair_tests};I/src/meter/job/transport_tests:send clock=>Uncertain,expired=>Unavailable,cancel1800ms,clamps500/2000ms.
- X/accounting/tests;X/transport/{historical_bounded_tests,historical_integrity}:clock500ms(200/150),same bytes/ID,expiry;channels/history.
- X/signals/accounting_tests#sole_waiter:old freshness=>production wait4->one injected sample->observed_at;ECHILD proves reap order. X/accounting/tests#terminal_builder:exact run endpoint. CPU>=0,peer<0.08s,owned delta>0.03s,exit0/17/SIGTERM kept.
- X/signals/tests:ready/event/release ACKs;60s barrier/isolation guards;signals once,masks/dispositions,sole reaping,detachment.
- X/launch/tests#expired_or_lost_release_is_rejected_by_native_pre_exec_before_the_target:closed ACK or deadline=now=>real pre_exec spawn error,marker absent;60s ACK guard,not expiry proof.
- o23#completed_lookups;D/tests/traffic_cli/terminal_outcomes#slow_completed_lookup:250ms original,1 admitted/0 renewed,ordered unknowns,held reply.
- D/tests/traffic_cli/unattached#restart_retains_unknown_child:recorded updated_at=>daemon audits,permit1@499ms/0@500ms,2 events/1 launch,no test reconcile.
- Y/governor_transport_tests#governor_{busy_job_route,unspawned_ack_after_expiry,late_unspawned_ack}:frozen owner=>DB_BUSY;paused2s+10ms/Drop before ACK=>OK/CANCELLED,no revival/spawn/claim.
- M/src/observation_cache_tests;Y/{policy_repair_tests,policy_clock_tests,observation#clock_tests,governor_tests/recovery}:lease clock=>Busy,order,retirement,old-Drop isolation,expiry,once.
- T/src/bridge/permit_tests;o7#registered_timeout:actual sync_read/blocking/Permit+receipt ACK=>Busy,reuse ok/error/unwind,dropped waiter holds,shutdown Cancelled.
- T/src/meter_bridge_tests#meter_native_deadline;B/memory_pressure/consumer_reads:timeout/EOF/Uncertain/reuse kept;permit tests replace races.
- F/fault_server#FaultServer;B/daemon_bridge#rejects_malformed_response:completion AFTER client return;retry IDs/query equal,all EOFs;sockets/workers close/join on unwind/partial frame. Uncertain/malformed/cached17;3 requests/3 EOFs,1804ms/60s.
- H/deadline_recovery;D/tests/worker_runtime:ACK+paused301ms=>4+4 timeouts,caps8/4,recovery/shutdown.
- Y/tests;D/src/lifecycle_tests;H/admission_load:ACKs=>cancel rollback,postcommit expiry receipt,ACK after COMMIT.
- H/native_fixture#Drop;R#{unwind_before_identity,drop_after_wrapper_exit};D/tests/bounded#pipe_holder:pre-spawn channels,pre-publication/adoption checkpoint;readiness error/unwind/cancel=>parent kill/wait+identity/reap receipt;ECHILD+Missing before dir removal;unconfirmed=>retain. Exit17,2x262144B/full-pipe drains,identity/stale-ID kept.
- Z/{calibration,lifecycle};R:two10s workers/reaped descendants,C/wait4 error<=max(0.1s,5%),escaped CPU>=400ms,queue/stopped exclusion,Partial/17/signal/once.
- o14-16;U/tests/governor.test.ts:FakeClock/deferred=>stale/reconnect/retire,one reader,time/focus,Unknown!=zero.
- D/tests/managed_exec/fault;H/measurements#run:held sender=>TimedOut10ms;workload/latency/RSS kept.
Other guards=changed timeout/readiness hunks vs Base:C/tests/**;D/tests/{resident,classification,memory_pressure,traffic_cli,traffic_bun_scripts,managed_exec}/**;H/{runtime,native_exec,bun_fault_lifecycle,meter_partial};F/{job_meter_reads,meter_native};B/{governor,job_meter_acceptance,memory_pressure,registered_commands_support}/**;o7#Peer;o25;D/src/database_failure_tests;Y/{jobs_tests,policy_tests,schema_tests,traffic_fault_tests,traffic_witness_repair_tests};crates/arc-storage/tests/memory_pressure/backlog. Readiness/ACK/release/EOF/exit;60s+(not speed claims). D/src/limits#holding_fixture600s setup only.
Coverage worst:calibration10.337068s/60s=5.80x;CLI0.132745s/60s=452x(prior0.484795s=123x);IPCmax0.411583ms/20ms=48.59x. Production ms unchanged:header1000/frame2000/write500/DB1500/exec1000/busy100/shutdown3500,bridge2000/preparation1800/terminal500.

## 38 omitted-path dispositions
F=fresh;P=budget;R=recorded/injected;N=no timing;E=counts.38 rechecked,no stale edits.
1 Cargo.lock N;2 T/src/bridge F;3 T/src/bridge/governor P;4 T/src/commands N;5 T/src/dto R;6 T/src/lib N;7 B/registered_commands F;
8-10 U/src/bridge/{client.ts:R,policy-contract.ts:R,types.ts:N};11-13 U/src/dashboard/{desktop-refresh-controller.ts:R,desktop-refresh-state.ts:R,selected-job-panel.tsx:R};14 U/tests/dashboard.test.ts R;15 U/tests/fixtures/desktop-test-harness.ts R;16 U/tests/lifecycle.test.ts R;
17 C/src/exec F;18 X/governor F;19 X/launch P;20 X/launch/governor P;21 X/traffic P;22 X/traffic/lifecycle P;23 C/src/job F;24 C/src/main F;25 C/tests/governor/support F;
26 D/src/connection/legacy_projection N;27 Y/governor_tests F;28 Y/policy F;29 D/src/governor_dispatch F/P;30 D/src/policy_dispatch P;31 D/src/policy_dispatch_tests R;32 I/src/governor/wire P;33 I/src/lib N;34 crates/arc-policy/src/lib N;35/36 crates/arc-storage/src/jobs/{traffic,traffic/store} R;37 docs/requirements/traceability.md E;38 scripts/coverage/config.ts N(11 crates).

## Checks and handoff
Final Cargo allowlist:
```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Active22 confirmed;Rust-owned templates/archive excluded:
```
bun test apps/desktop/tests/bridge-client.test.ts apps/desktop/tests/contracts.test.ts apps/desktop/tests/credits.test.ts apps/desktop/tests/dashboard.test.ts apps/desktop/tests/governor-contract.test.ts apps/desktop/tests/governor.test.ts apps/desktop/tests/lifecycle.test.ts apps/desktop/tests/meter.test.ts apps/desktop/tests/traffic-scripts-contract.test.ts apps/desktop/tests/traffic-scripts.test.ts scripts/coverage/accounting.test.ts scripts/coverage/desktop-foundation.test.ts scripts/coverage/evidence.test.ts scripts/coverage/gate.test.ts scripts/coverage/hygiene.test.ts scripts/coverage/terminal-outcomes.test.ts scripts/coverage/traffic-accounting.test.ts scripts/harness/coverage-reuse.test.ts scripts/harness/equivalence.test.ts scripts/harness/traffic-targets.test.ts scripts/requirements/traceability-parser.test.ts scripts/requirements/traceability.test.ts
```
Focused task1 offline:desktop bridge14/permits4/meter1;CLI signals14/accounting11/preparation5/pre_exec1;bounded10/pipe_holder3 detached;trace29/29,316 IDs/18 counts. Earlier compile/socket/identity/AV failures repaired.
AV4 PASS 2026-11-14T20:20:17Z:fmt/Clippy/workspace/active22/typecheck/check/architecture1-4 reused;success streams omitted by AV. Task2 desktop build/smoke,trace316/18,mapping253/148 PASS. Coverage11/11>=80%,min87.97%,one collection;unchanged harness reuse PASS(all5);six gates PASS;QA task3. Inputs855c3d71 unchanged;results excluded by maintained identity. Only contract.md<=8000/gates.json<=40000/qa.json<=20000;logs target/. Protected/deletions intact.
