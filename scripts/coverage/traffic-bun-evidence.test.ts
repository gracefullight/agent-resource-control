import { test, expect } from "bun:test";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import { basename, relative, resolve } from "node:path";
import { hash } from "./source";
import { allowance, boundJSON } from "./traffic-receipts";
import { beginHarnessInventory, binaryCommand, capture, compilerArtifact, evidenceBudget, finalArtifacts, finishHarnessInventory, missionInputs, nativeCommand, requiredCommands, sourceContext, validateBinaryBinding, validateCommandSet, validateFinalEvidence, validateGateSummary, validateHarnessInventory, validateNestedCoverage, validateReceipt, validateRecordedSource } from "./traffic-bun-evidence";
import { deriveCase, deriveOrders, measurementShim } from "./traffic-bun-measurement";
import { validateFullCargoOutput, validateLiveBunOutput } from "./traffic-evidence";
import { PINS } from "./config";

const root = realpathSync(resolve(import.meta.dir, "../.."));
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const boot = "12345678-1234-1234-1234-123456789abc";
const ident = (pid: number, start: number) => ({ pid, start_token: String(start), boot_id: boot });
const ref = (path: string) => ({ path: relative(root, path), sha256: hash(readFileSync(path)) });

// These are unit fixtures, not live acceptance evidence. Final validation also
// requires a source-bound successful native command and its matching output.
function simulatedCase(order: "cargo" | "bun", mode: "bypass" | "scheduled") {
  mkdirSync(resolve(root, "target/traffic-bun"), { recursive: true });
  const base = mkdtempSync(resolve(root, "target/traffic-bun/validator-fixture-"));
  const time = Date.now() - 3000, us = (ms: number) => (time + ms) * 1000;
  // Unit identities use tiny fixture executables; native runs bind real tools.
  const toolDir = resolve(base, "unit-tools"); mkdirSync(toolDir);
  const cargoExe = resolve(toolDir, "cargo"), bunExe = resolve(toolDir, "bun");
  for (const path of [cargoExe, bunExe]) { writeFileSync(path, "#!/bin/sh\nexit 0\n"); chmodSync(path, 0o700); }
  const exec = (path: string) => ({ path, sha256: hash(readFileSync(path)) });
  const fakeDaemon = resolve(base, "arc-daemon"), fakeCli = resolve(base, "arcctl");
  for (const path of [fakeDaemon, fakeCli]) { writeFileSync(path, "#!/bin/sh\nexit 0\n"); chmodSync(path, 0o700); }
  mkdirSync(resolve(base, "shims"));
  for (const tool of ["cargo", "bun"]) writeFileSync(resolve(base, "shims", tool), measurementShim(root));
  const jobs = ["cargo", "bun"].map((tool, i) => {
    const cwd = resolve(base, tool); mkdirSync(cwd, { recursive: true });
    if (i === 0) {
      mkdirSync(resolve(cwd, "src")); mkdirSync(resolve(cwd, "target"));
      writeFileSync(resolve(cwd, "Cargo.toml"), "[package]\nname='fixture'\nversion='0.1.0'\n[workspace]\n");
      writeFileSync(resolve(cwd, "src/lib.rs"), "#[test] fn real_cargo_barrier() { /* MIXED_BODY_LOG MIXED_RELEASE */ }\n");
      writeFileSync(resolve(cwd, "target/CACHEDIR.TAG"), "fixture\n");
    } else writeFileSync(resolve(cwd, "barrier.test.ts"), "// bun:test real bun barrier MIXED_BODY_LOG MIXED_RELEASE synthetic unit fixture\n");
    const toolPid = 2100 + i, testPid = i === 0 ? 2200 : toolPid;
    const start = tool === order ? 110 : mode === "scheduled" ? 500 : 200;
    const toolId = ident(toolPid, us(start));
    const testId = i === 0 ? ident(testPid, us(start + 10)) : toolId;
    const wrapperId = mode === "bypass" ? toolId : ident(2300 + i, us(tool === order ? 100 : 330));
    writeFileSync(resolve(cwd, "launches"), `${toolPid}\n`);
    if (mode === "scheduled" && tool !== order) {
      const firstPid = order === "cargo" ? 2200 : 2101;
      const firstId = (order === "cargo" ? "a" : "b").repeat(32), secondId = (tool === "cargo" ? "a" : "b").repeat(32);
      writeFileSync(resolve(cwd, "committed-release"), `${toolPid} ${firstPid} ${firstId} RELEASED ${time + 365} ${secondId} ACTIVE ${time + 365} ${time + 350}\n`);
    }
    writeFileSync(resolve(cwd, "bodies"), `${testPid}\n`);
    writeFileSync(resolve(cwd, "stdout.log"), i === 0 ? "test real_cargo_barrier ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n" : "");
    writeFileSync(resolve(cwd, "stderr.log"), (i === 1 ? "(pass) real bun barrier\n1 pass\n0 fail\n" : "") +
      (mode === "scheduled" && tool !== order ? "STARTED\nCommand started.\n" : ""));
    return { tool, cwd, argv: i === 0 ? ["test", "--offline", "--", "--nocapture"] : ["test"],
      process: { wrapper: wrapperId, tool: toolId, test: testId, parentVerified: true,
        toolExecutable: exec(i === 0 ? cargoExe : bunExe), testExecutable: exec(i === 0 ? cargoExe : bunExe),
        wrapperExecutable: exec(mode === "bypass" ? (i === 0 ? cargoExe : bunExe) : fakeCli) },
      launches: ref(resolve(cwd, "launches")), bodies: ref(resolve(cwd, "bodies")),
      shim: ref(resolve(base, "shims", tool)), wrapperExitObservedAtUnixUs: us(1000),
      source: ref(resolve(cwd, i === 0 ? "src/lib.rs" : "barrier.test.ts")),
      manifest: i === 0 ? ref(resolve(cwd, "Cargo.toml")) : null, stdout: ref(resolve(cwd, "stdout.log")),
      stderr: ref(resolve(cwd, "stderr.log")), exitCode: 0, outcome: "EXITED:0",
      record: mode === "scheduled" ? {jobId: i === 0 ? "a".repeat(32) : "b".repeat(32), state: "COMPLETED", exitCode: 0, outcomeKind: "EXITED", class: "TEST"} : null };
  });
  const second = order === "cargo" ? "bun" : "cargo", secondJob = jobs[second === "cargo" ? 0 : 1];
  const names = mode === "bypass" ? [["test-start", order], ["test-start", second], ["barrier-release", order], ["test-end", order], ["test-end", second]] :
    [["test-start", order], ["queued", second], ["notice", second], ["barrier-release", order], ["test-end", order],
      ["permit-release", order], ["grant", second], ["test-start", second], ["test-end", second]];
  const events = names.map(([event, tool], seq) => ({ event, tool, seq, atUs: seq * 10 }));
  let notice: ReturnType<typeof ref> | null = null;
  if (mode === "scheduled") {
    writeFileSync(resolve(secondJob.cwd, "queued.log"), "QUEUED\nWaiting to start.\n");
    notice = ref(resolve(secondJob.cwd, "queued.log"));
  }
  const firstJob = jobs[order === "cargo" ? 0 : 1];
  return { schemaVersion: 1, id: `${order}_first-${mode}`, order, mode, daemon: {identity: ident(2000, us(50)), executable: exec(fakeDaemon)}, jobs, events, notice,
    firstEndObservation: {test: firstJob.process.test, observedStoppedAtUnixUs: us(mode === "scheduled" ? 650 : 360)},
    secondEndObservation: {test: secondJob.process.test, observedStoppedAtUnixUs: us(mode === "scheduled" ? 900 : 370)},
    overlapObservation: mode === "bypass" ? {first: firstJob.process.test, second: secondJob.process.test, bothLive: true, observedAtUnixUs: us(250)} : null,
    committedRelease: mode === "scheduled" ? ref(resolve(secondJob.cwd, "committed-release")) : null,
    sqliteExecutable: mode === "scheduled" ? exec(realpathSync("/usr/bin/sqlite3")) : null,
    queue: mode === "scheduled" ? {jobId: secondJob.record!.jobId, state: "QUEUED", queuedAtMs: time + 350, activePermits: 1, childAbsent: true, firstTestLive: true,
      firstJobId: jobs[order === "cargo" ? 0 : 1].record!.jobId, firstClass: "TEST", firstPermitState: "ACTIVE"} : null,
    permit: mode === "scheduled" ? {releasedAtMs: time + 365, releaseObservedAtUnixUs: us(800), secondBodyObservedAtUnixUs: us(600), releaseState: "RELEASED", releaseJobId: jobs[order === "cargo" ? 0 : 1].record!.jobId,
      grantedAtMs: time + 365, grantState: "ACTIVE", grantJobId: secondJob.record!.jobId, queueWaitMs: 15,
      secondLaunchAtMs: Math.floor(statSync(resolve(root, secondJob.launches.path)).mtimeMs)} : null };
}
function window() { return { started: new Date(Date.now() - 5000).toISOString(), ended: new Date(Date.now() + 5000).toISOString() }; }

test("unit fixture derives peaks, wait and outcomes in both orders", () => {
  expect(basename(realpathSync(Bun.spawnSync(["rustup", "which", "cargo"]).stdout.toString().trim()))).toBe("cargo");
  const cases = [simulatedCase("cargo", "bypass"), simulatedCase("cargo", "scheduled"), simulatedCase("bun", "bypass"), simulatedCase("bun", "scheduled")];
  const orders = deriveOrders(root, cases, window());
  for (const order of ["cargo_first", "bun_first"]) {
    expect(orders[order].bypass.peakConcurrentTests).toBe(2);
    expect(orders[order].scheduled.peakConcurrentTests).toBe(1);
    expect(orders[order].scheduled.queueWaitMs).toBe(15);
    expect(orders[order].scheduled.invocations).toEqual([1, 1]);
  }
  expect(() => deriveOrders(root, cases.slice(1), window())).toThrow();
  expect(() => deriveOrders(root, [cases[0], cases[1], cases[2], cases[2]], window())).toThrow();
});

test("unit fixture rejects missing, altered, duplicate and mismatched observations", () => {
  const raw = simulatedCase("cargo", "scheduled"), w = window();
  const missing = clone(raw); missing.events.splice(1, 1);
  expect(() => deriveCase(root, missing, w)).toThrow();
  const order = clone(raw); order.events[5].event = "grant";
  expect(() => deriveCase(root, order, w)).toThrow();
  const wait = clone(raw); wait.permit!.queueWaitMs++;
  expect(() => deriveCase(root, wait, w)).toThrow();
  // The controller observes exit and release after the second body is live;
  // the process start identity and the committed snapshot remain unchanged.
  expect(Number(raw.jobs[1].process.tool.start_token)).toBeLessThan(raw.firstEndObservation.observedStoppedAtUnixUs);
  expect(Number(raw.jobs[1].process.tool.start_token)).toBeLessThan(raw.permit!.releaseObservedAtUnixUs);
  expect(() => deriveCase(root, raw, w)).not.toThrow();
  const early = simulatedCase("cargo", "scheduled"), witnessPath = resolve(root, early.committedRelease!.path);
  const originalWitness = readFileSync(witnessPath, "utf8");
  writeFileSync(witnessPath, originalWitness.replace(" RELEASED ", " ACTIVE "));
  early.committedRelease = ref(witnessPath);
  expect(() => deriveCase(root, early, w)).toThrow();
  const commit = clone(raw); commit.committedRelease = null;
  expect(() => deriveCase(root, commit, w)).toThrow();
  // Release and grant milliseconds may tie; the non-waiting pre-exec witness
  // records both states in one committed database snapshot.
  expect(raw.permit!.releasedAtMs).toBe(raw.permit!.grantedAtMs);
  expect(() => deriveCase(root, raw, w)).not.toThrow();
  const staleIdentity = clone(raw); staleIdentity.jobs[0].process.tool.start_token = "1000000";
  expect(() => deriveCase(root, staleIdentity, w)).toThrow();
  const justBeforeWindow = clone(raw); justBeforeWindow.daemon.identity.start_token = String(Date.parse(w.started) * 1000 - 1);
  expect(() => deriveCase(root, justBeforeWindow, w)).toThrow();
  const staleDaemon = clone(raw); staleDaemon.daemon.identity.start_token = "1000000";
  expect(() => deriveCase(root, staleDaemon, w)).toThrow();
  const reusedPid = clone(raw); reusedPid.daemon.identity.pid = reusedPid.jobs[0].process.wrapper.pid;
  expect(() => deriveCase(root, reusedPid, w)).toThrow();
  const otherBoot = clone(raw); otherBoot.jobs[1].process.tool.boot_id = "87654321-4321-4321-4321-cba987654321";
  expect(() => deriveCase(root, otherBoot, w)).toThrow();
  const daemonBoot = clone(raw); daemonBoot.daemon.identity.boot_id = "87654321-4321-4321-4321-cba987654321";
  expect(() => deriveCase(root, daemonBoot, w)).toThrow();
  const lateExit = clone(raw); lateExit.secondEndObservation.observedStoppedAtUnixUs = (Date.parse(w.ended) + 1000) * 1000;
  expect(() => deriveCase(root, lateExit, w)).toThrow();
  const hashMismatch = clone(raw); hashMismatch.jobs[1].bodies.sha256 = "0".repeat(64);
  expect(() => deriveCase(root, hashMismatch, w)).toThrow();
  const sourceMismatch = clone(raw); sourceMismatch.jobs[0].source.sha256 = "0".repeat(64);
  expect(() => deriveCase(root, sourceMismatch, w)).toThrow();
  const noNotice = clone(raw); noNotice.notice = null;
  expect(() => deriveCase(root, noNotice, w)).toThrow();
  const duplicated = clone(raw); const bodyPath = resolve(root, duplicated.jobs[1].bodies.path);
  writeFileSync(bodyPath, "2101\n2101\n"); duplicated.jobs[1].bodies = ref(bodyPath);
  expect(() => deriveCase(root, duplicated, w)).toThrow();
  const stale = {started: new Date(0).toISOString(), ended: new Date(1000).toISOString()};
  expect(() => deriveCase(root, raw, stale)).toThrow();
  const alteredShim = simulatedCase("bun", "scheduled"), shimPath = resolve(root, alteredShim.jobs[0].shim.path);
  writeFileSync(shimPath, measurementShim(root).replace('exec "$MIXED_REAL_TOOL" "$@"', '"$MIXED_REAL_TOOL" "$@" &\nwait'));
  alteredShim.jobs[0].shim = ref(shimPath);
  expect(() => deriveCase(root, alteredShim, w)).toThrow();
});

test("kernel start identity rejects an early or ambiguous second launch", () => {
  for (const order of ["cargo", "bun"] as const) {
    const raw = simulatedCase(order, "scheduled"), w = window();
    const secondIndex = order === "cargo" ? 1 : 0;
    const setStart = (value: any, start: number) => {
      const process = value.jobs[secondIndex].process;
      process.tool.start_token = String(start);
      if (order === "cargo") {
        // Bun runs the test body in the same process as the native tool.
        process.test.start_token = String(start);
        value.secondEndObservation.test.start_token = String(start);
      }
    };
    // The committed-state shim witness and all other observations remain valid:
    // the process was created early but its shim first ran after the release.
    const early = clone(raw);
    setStart(early, (raw.permit!.releasedAtMs - 1) * 1000);
    expect(() => deriveCase(root, early, w)).toThrow("kernel start is not unambiguously after release");
    const ambiguous = clone(raw);
    setStart(ambiguous, raw.permit!.releasedAtMs * 1000 + 999);
    expect(() => deriveCase(root, ambiguous, w)).toThrow("kernel start is not unambiguously after release");
    const after = clone(raw);
    setStart(after, (raw.permit!.releasedAtMs + 1) * 1000);
    expect(() => deriveCase(root, after, w)).not.toThrow();
    // Polling the already committed release late must not move its timestamp.
    expect(raw.permit!.secondBodyObservedAtUnixUs).toBeLessThan(raw.permit!.releaseObservedAtUnixUs);
    expect(() => deriveCase(root, raw, w)).not.toThrow();
    const prompt = clone(raw);
    prompt.permit.releaseObservedAtUnixUs = (raw.permit!.releasedAtMs + 1) * 1000;
    expect(() => deriveCase(root, prompt, w)).not.toThrow();
  }
});

test("genuine command receipt binds source and raw files, and rejects corruption and skips", async () => {
  const receiptRef = await capture(root, "validator-smoke", ["bun", "-e", "console.log('receipt smoke')"], "measurement-engineer");
  expect(receiptRef.exit).toBe(0);
  const receipt = boundJSON(root, receiptRef, "target/traffic-bun");
  const context = sourceContext(root);
  expect(validateReceipt(root, receipt, context, "measurement-engineer").stdout.trim()).toBe("receipt smoke");
  for (const mutation of [
    (r: any) => r.sourceHash = "0".repeat(64),
    (r: any) => r.sourceHashAfter = "0".repeat(64),
    (r: any) => r.acceptedHead = "0".repeat(40),
    (r: any) => r.fixtureOnly = true,
    (r: any) => r.skipFlags = true,
    (r: any) => r.argv.push("--ignored"),
    (r: any) => r.output.stdout.sha256 = "0".repeat(64),
    (r: any) => r.output.stderr.path = "target/traffic-bun/missing.log",
    (r: any) => r.exit = 1,
    (r: any) => r.timeoutAllowanceMs = 120000,
  ]) {
    const bad = clone(receipt); mutation(bad);
    expect(() => validateReceipt(root, bad, context, "measurement-engineer")).toThrow();
  }
});

function inventoryFixture(prefix: string) {
  const repo = mkdtempSync(resolve(root, `target/traffic-bun/${prefix}-`));
  const git = (...args: string[]) => {
    const result = Bun.spawnSync(["git", ...args], {cwd: repo, stdout: "pipe", stderr: "pipe"});
    expect(result.exitCode).toBe(0);
    return result.stdout;
  };
  git("init", "-q");
  for (const dir of ["crates", "scripts/coverage", "scripts/harness", ".arc-verify/rust-coverage/contract", ".arc-verify/traffic-bun"]) {
    mkdirSync(resolve(repo, dir), {recursive: true});
  }
  for (const path of ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "mise.toml", "package.json", ".gitignore",
    ".arc-verify/rust-coverage/contract/accounting.json", "scripts/coverage/validator.ts"]) {
    writeFileSync(resolve(repo, path), `fixture ${path}\n`);
  }
  return {repo, git};
}

test("harness inventory captures the actual Git output and rejects untracked delivered source", () => {
  const {repo, git} = inventoryFixture("inventory-fixture");
  writeFileSync(resolve(repo, ".arc-verify/traffic-bun/restore-predecessor.py"), "print('restore')\n");
  git("add", "--all");
  writeFileSync(resolve(repo, "source.rs"), "source\n");
  const required = ["source.rs", "scripts/coverage/validator.ts"];
  const collect = (name: string) => {
    const runDir = `target/traffic-bun/commands/${name}`;
    mkdirSync(resolve(repo, runDir), {recursive: true});
    const prepared = beginHarnessInventory(repo, runDir);
    const result = Bun.spawnSync(["/bin/sh", "-c", "git ls-files"], {
      cwd: repo, env: {...process.env, ...prepared.env}, stdout: "pipe", stderr: "pipe",
    });
    expect(result.exitCode).toBe(0);
    const sourceHash = missionInputs(repo).sha256;
    const trackedInventory = finishHarnessInventory(repo, runDir, sourceHash, prepared);
    expect(result.stdout.equals(readFileSync(resolve(repo, trackedInventory.actual!.path)))).toBe(true);
    return {runDir, sourceHash, trackedInventory};
  };
  const untracked = collect("untracked");
  expect(() => validateHarnessInventory(repo, untracked, required)).toThrow("untracked");
  git("add", "source.rs");
  expect(() => validateHarnessInventory(repo, untracked, required)).toThrow();
  const complete = collect("complete");
  expect(() => validateHarnessInventory(repo, complete, required)).not.toThrow();
  const missing = clone(complete); delete missing.trackedInventory.actual;
  expect(() => validateHarnessInventory(repo, missing, required)).toThrow();
  const stale = clone(complete); stale.sourceHash = "different";
  expect(() => validateHarnessInventory(repo, stale, required)).toThrow();
  const changed = clone(complete), actual = resolve(repo, changed.trackedInventory.actual.path);
  writeFileSync(actual, "source.rs\n");
  changed.trackedInventory.actual.sha256 = hash(readFileSync(actual));
  expect(() => validateHarnessInventory(repo, changed, required)).toThrow("inventory changed");
  const again = collect("again");
  writeFileSync(resolve(repo, "other.rs"), "other\n"); git("add", "other.rs");
  expect(() => validateHarnessInventory(repo, again, required)).toThrow("inventory changed");
  const evidence = collect("evidence");
  writeFileSync(resolve(repo, finalArtifacts[0]), "{}\n"); git("add", finalArtifacts[0]);
  expect(() => validateHarnessInventory(repo, evidence, required)).toThrow("inventory changed");
});

test("production harness inventory binds the delivered restoration script", () => {
  const {repo, git} = inventoryFixture("restore-inventory-fixture");
  git("add", "--all");
  const script = ".arc-verify/traffic-bun/restore-predecessor.py";
  writeFileSync(resolve(repo, script), "print('original restoration')\n");
  const collect = (name: string) => {
    const runDir = `target/traffic-bun/commands/${name}`;
    mkdirSync(resolve(repo, runDir), {recursive: true});
    const prepared = beginHarnessInventory(repo, runDir);
    const result = Bun.spawnSync(["/bin/sh", "-c", "git ls-files"], {
      cwd: repo, env: {...process.env, ...prepared.env}, stdout: "pipe", stderr: "pipe",
    });
    expect(result.exitCode).toBe(0);
    const sourceHash = missionInputs(repo).sha256;
    return {runDir, sourceHash, trackedInventory: finishHarnessInventory(repo, runDir, sourceHash, prepared)};
  };
  const untracked = collect("untracked-restoration");
  expect(missionInputs(repo).files[script].sha256).toBe(hash(readFileSync(resolve(repo, script))));
  expect(() => validateHarnessInventory(repo, untracked)).toThrow("untracked");
  git("add", "--", script);
  const recorded = collect("tracked-restoration");
  expect(() => validateHarnessInventory(repo, recorded)).not.toThrow();
  writeFileSync(resolve(repo, script), "print('changed restoration')\n");
  git("add", "--", script);
  expect(missionInputs(repo).sha256).not.toBe(recorded.sourceHash);
  expect(() => validateHarnessInventory(repo, recorded)).toThrow("source");
  writeFileSync(resolve(repo, script), "print('original restoration')\n");
  git("add", "--", script);
  const other = ".arc-verify/traffic-bun/another-helper.sh";
  writeFileSync(resolve(repo, other), "#!/bin/sh\nexit 0\n");
  const untrackedHelper = collect("untracked-helper");
  expect(missionInputs(repo).files[other]).toBeDefined();
  expect(() => validateHarnessInventory(repo, untrackedHelper)).toThrow("untracked");
  git("add", "--", other);
  expect(() => validateHarnessInventory(repo, collect("tracked-helper"))).not.toThrow();
  unlinkSync(resolve(repo, script));
  expect(() => missionInputs(repo)).toThrow();
});

test("evidence-only descendant preserves recorded source provenance", () => {
  const repo = mkdtempSync(resolve(root, "target/traffic-bun/revision-fixture-"));
  const git = (...args: string[]) => {
    const result = Bun.spawnSync(["git", ...args], {cwd: repo, stdout: "pipe", stderr: "pipe"});
    expect(result.exitCode).toBe(0);
    return result.stdout.toString().trim();
  };
  const commit = (message: string) => git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", message);
  git("init", "-q");
  writeFileSync(resolve(repo, "source.rs"), "fn source() {}\n"); git("add", "source.rs"); commit("source");
  const head = git("rev-parse", "HEAD"), tree = git("rev-parse", "HEAD^{tree}"), sourceHash = hash(readFileSync(resolve(repo, "source.rs")));
  const recorded = {acceptedHead: head, acceptedTree: tree, head, tree, sourceHash};
  mkdirSync(resolve(repo, ".arc-verify/traffic-bun"), {recursive: true});
  writeFileSync(resolve(repo, ".arc-verify/traffic-bun/qa.md"), "evidence only\n");
  git("add", ".arc-verify/traffic-bun/qa.md"); commit("evidence");
  const context: any = {acceptedHead: head, acceptedTree: tree, head: git("rev-parse", "HEAD"), tree: git("rev-parse", "HEAD^{tree}"), sourceHash};
  expect(context.head).not.toBe(recorded.head);
  expect(context.tree).not.toBe(recorded.tree);
  expect(() => validateRecordedSource(repo, recorded, context)).not.toThrow();
  expect(() => validateRecordedSource(repo, {...recorded, tree: context.tree}, context)).toThrow();
  writeFileSync(resolve(repo, "source.rs"), "fn changed() {}\n");
  expect(() => validateRecordedSource(repo, recorded, {...context, sourceHash: hash(readFileSync(resolve(repo, "source.rs")))})).toThrow();
  const unrelated = git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit-tree", tree, "-m", "unrelated");
  expect(() => validateRecordedSource(repo, {...recorded, head: unrelated}, context)).toThrow();
});

test("observed CLI and daemon must match ordered Cargo artifact receipts", () => {
  const cli = {path: "/target/arcctl", sha256: "a".repeat(64)};
  const daemon = {path: "/target/arc-daemon", sha256: "b".repeat(64)};
  const runDir = "target/traffic-bun/commands/source-fixture", binaryTarget = `${runDir}/binaries`;
  const preparation = {argv: binaryCommand, runDir, binaryTarget, productBinary: cli, ended: "2026-01-01T00:00:00.000Z"};
  const native = {argv: nativeCommand, binaryTarget, binaryPreparation: {path: `${runDir}/receipt.json`}, productBinary: daemon, started: "2026-01-01T00:00:01.000Z"};
  const cases = [{mode: "scheduled", daemon: {executable: daemon}, jobs: [{process: {wrapperExecutable: cli}}]}];
  expect(() => validateBinaryBinding(cases, native, preparation)).not.toThrow();
  const wrong = clone(cases); wrong[0].jobs[0].process.wrapperExecutable.sha256 = "0".repeat(64);
  expect(() => validateBinaryBinding(wrong, native, preparation)).toThrow();
  const stale = {...preparation, ended: "2026-01-01T00:00:02.000Z"};
  expect(() => validateBinaryBinding(cases, native, stale)).toThrow();
  const otherDaemon = clone(cases); otherDaemon[0].daemon.executable.path = "/other/arc-daemon";
  expect(() => validateBinaryBinding(otherDaemon, native, preparation)).toThrow();
});

test("Cargo artifact output identifies the exact source package and target binary", () => {
  const fixture = mkdtempSync(resolve(root, "target/traffic-bun/artifact-fixture-"));
  const target = "target/traffic-bun/commands/source-fixture/binaries";
  const binary = resolve(fixture, target, "debug/arcctl");
  mkdirSync(resolve(binary, ".."), {recursive: true}); writeFileSync(binary, "binary one"); chmodSync(binary, 0o700);
  const row: any = {reason: "compiler-artifact", target: {name: "arcctl", kind: ["bin"]}, profile: {test: false},
    executable: binary, fresh: false, manifest_path: resolve(fixture, "crates/arc-cli/Cargo.toml")};
  const output = JSON.stringify(row) + "\n", original = compilerArtifact(fixture, output, "arcctl", target);
  expect(original.path).toBe(binary);
  writeFileSync(binary, "binary two");
  expect(compilerArtifact(fixture, output, "arcctl", target).sha256).not.toBe(original.sha256);
  row.executable = resolve(fixture, "target/debug/arcctl");
  expect(() => compilerArtifact(fixture, JSON.stringify(row), "arcctl", target)).toThrow();
  row.executable = binary; row.manifest_path = resolve(fixture, "crates/arc-daemon/Cargo.toml");
  expect(() => compilerArtifact(fixture, JSON.stringify(row), "arcctl", target)).toThrow();
  row.manifest_path = resolve(fixture, "crates/arc-cli/Cargo.toml"); row.fresh = true;
  expect(() => compilerArtifact(fixture, JSON.stringify(row), "arcctl", target)).toThrow();
  expect(() => compilerArtifact(fixture, output + output, "arcctl", target)).toThrow();
});

test("nested coverage must execute completely inside its containing collector", () => {
  const id = `nested-${crypto.randomUUID()}`, dir = `target/traffic-bun/commands/${id}`;
  const raw = `target/traffic/native-coverage/${id}`, collection = `.arc-verify/rust-coverage/baseline/runs/${id}`;
  for (const path of [dir, raw, collection]) mkdirSync(resolve(root, path), {recursive: true});
  const write = (path: string, text: string) => { writeFileSync(resolve(root, path), text); return ref(resolve(root, path)); };
  const stdout = "running 1 test\ntest nested ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
  const started = new Date(Date.now() - 4000).toISOString(), ended = new Date(Date.now() + 4000).toISOString();
  const testReceipt: any = {argv: ["rustup", "run", PINS.rust, "cargo", "llvm-cov", "--workspace", "--all-targets", "--locked", "--offline", "--no-report", "--no-cfg-coverage", "--no-cfg-coverage-nightly"],
    cwd: root, started: new Date(Date.now() - 1000).toISOString(), ended: new Date(Date.now() + 1000).toISOString(),
    pid: 1234, exit: 0, stdout, stderr: "", output: {stdout: write(`${raw}/stdout.log`, stdout), stderr: write(`${raw}/stderr.log`, "")}};
  const nested: any = {runId: id, pass: true, exit: 0, receipts: [testReceipt]};
  const collector: any = {runDir: dir, started, ended, nestedCoverage: {runId: id,
    stdout: write(`${dir}/native-test.stdout.log`, stdout), stderr: write(`${dir}/native-test.stderr.log`, "")}};
  const refresh = () => {
    const content = JSON.stringify(nested);
    write(`${collection}/receipt.json`, content);
    collector.nestedCoverage.receipt = write(`${dir}/native-receipt.json`, content);
  };
  const message = `Coverage reports: ${collection}\n`;
  refresh();
  expect(validateNestedCoverage(root, collector, message)).toBe(id);
  const missing = clone(collector); delete missing.nestedCoverage;
  expect(() => validateNestedCoverage(root, missing, message)).toThrow();
  nested.receipts = []; refresh();
  expect(() => validateNestedCoverage(root, collector, message)).toThrow();
  nested.receipts = [testReceipt];
  testReceipt.started = new Date(Date.now() - 10000).toISOString(); refresh();
  expect(() => validateNestedCoverage(root, collector, message)).toThrow();
  testReceipt.started = new Date(Date.now() - 1000).toISOString();
  testReceipt.argv.push("--skip"); refresh();
  expect(() => validateNestedCoverage(root, collector, message)).toThrow();
  testReceipt.argv.pop();
  const skipped = stdout.replace("0 ignored", "1 ignored");
  testReceipt.stdout = skipped;
  testReceipt.output.stdout = write(`${raw}/stdout.log`, skipped);
  collector.nestedCoverage.stdout = write(`${dir}/native-test.stdout.log`, skipped);
  refresh();
  expect(() => validateNestedCoverage(root, collector, message)).toThrow();
});

test("evidence budget checks staged blobs, missing paths and local mismatches", () => {
  const repo = mkdtempSync(resolve(root, "target/traffic-bun/budget-fixture-"));
  const git = (...args: string[]) => {
    const result = Bun.spawnSync(["git", ...args], {cwd: repo, stdout: "pipe", stderr: "pipe"});
    expect(result.exitCode).toBe(0);
  };
  git("init", "-q"); mkdirSync(resolve(repo, ".arc-verify/traffic-bun"), {recursive: true});
  const file = resolve(repo, ".arc-verify/traffic-bun/file.md");
  writeFileSync(file, "small\n"); git("add", ".arc-verify/traffic-bun");
  expect(evidenceBudget(repo).indexed).toBe(1);
  chmodSync(file, 0o755);
  expect(() => evidenceBudget(repo)).toThrow();
  chmodSync(file, 0o644);
  unlinkSync(file);
  expect(() => evidenceBudget(repo)).toThrow();
  writeFileSync(file, "x".repeat(5_000_000)); git("add", ".arc-verify/traffic-bun");
  writeFileSync(file, "small\n");
  expect(() => evidenceBudget(repo)).toThrow();
  for (let i = 0; i < 30; i++) writeFileSync(resolve(repo, `.arc-verify/traffic-bun/${i}.md`), "x");
  git("add", ".arc-verify/traffic-bun");
  for (let i = 0; i < 30; i++) unlinkSync(resolve(repo, `.arc-verify/traffic-bun/${i}.md`));
  expect(() => evidenceBudget(repo)).toThrow();
});

test("final evidence requires matching indexed summaries and QA artifacts", () => {
  const repo = mkdtempSync(resolve(root, "target/traffic-bun/final-index-fixture-"));
  const git = (...args: string[]) => {
    const result = Bun.spawnSync(["git", ...args], {cwd: repo, stdout: "pipe", stderr: "pipe"});
    expect(result.exitCode).toBe(0);
  };
  git("init", "-q");
  mkdirSync(resolve(repo, ".arc-verify/traffic-bun"), {recursive: true});
  for (const path of finalArtifacts) writeFileSync(resolve(repo, path), path.endsWith(".json") ? "{}\n" : "oma-qa\n");
  expect(() => evidenceBudget(repo, finalArtifacts)).toThrow("not indexed");
  git("add", ...finalArtifacts.slice(0, -1));
  expect(() => evidenceBudget(repo, finalArtifacts)).toThrow("not indexed");
  git("add", finalArtifacts.at(-1)!);
  expect(evidenceBudget(repo, finalArtifacts).indexed).toBe(finalArtifacts.length);
  writeFileSync(resolve(repo, finalArtifacts[0]), "{\"changed\":true}\n");
  expect(() => evidenceBudget(repo, finalArtifacts)).toThrow("indexed and working evidence differ");
});

test("gate helpers reject missing commands, failed or skipped output and stale summaries", () => {
  const commands = requiredCommands.map(argv => ({receipt: {argv}}));
  expect(() => validateCommandSet(commands)).not.toThrow();
  expect(() => validateCommandSet(commands.slice(1))).toThrow();
  expect(() => validateCommandSet([...commands.slice(0, -1), commands[0]])).toThrow();
  expect(() => validateGateSummary({coveragePassed: true, fullHarnessPassed: true, sourceHash: "fresh", commands: []}, "fresh")).not.toThrow();
  expect(() => validateGateSummary({coveragePassed: true, fullHarnessPassed: true, sourceHash: "old", commands: []}, "fresh")).toThrow();
  expect(() => validateGateSummary({coveragePassed: true, fullHarnessPassed: false, sourceHash: "fresh", commands: []}, "fresh")).toThrow();
  expect(() => validateLiveBunOutput("1 pass\n0 fail\n")).not.toThrow();
  expect(() => validateLiveBunOutput("1 pass\n1 skip\n0 fail\n")).toThrow();
  expect(() => validateFullCargoOutput("running 1 test\ntest x ... ok\ntest result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out;\n")).toThrow();
});
test("live source-bound mixed tests, coverage, full harness, independent QA and budgets", () => {
  expect(validateFinalEvidence(root).pass).toBe(true);
}, allowance);
