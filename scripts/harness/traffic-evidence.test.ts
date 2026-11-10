import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { evidenceBudget, validateCollector, validateFinalTrafficEvidence, validateFullCargoOutput, validateLatestCollection, validateLiveBunOutput, validateNestedCoverage } from "../coverage/traffic-evidence";
import { acceptanceTest, measurementFromOutput, validateMeasurement } from "../coverage/traffic-measurement";
import { boundJSON, capture, captureCoverage, sourceContext, validateReceipt } from "../coverage/traffic-receipts";
import { gitBytes } from "../coverage/hygiene";
import { hash } from "../coverage/source";
import { command } from "../coverage/commands";
import { nativeOutput } from "../coverage/native-output";

const root = resolve(import.meta.dir, "../..");
function fixtureEvents() {
  const directory = realpathSync(mkdtempSync(resolve(tmpdir(), "traffic-measurement-")));
  for (const job of [0, 1]) {
    const project = resolve(directory, `project-${job}`);
    mkdirSync(project);
    writeFileSync(resolve(project, "Cargo.toml"), `[package]\nname = "fixture-${job}"\nversion = "0.1.0"\n`);
    writeFileSync(resolve(project, "build.rs"), "fn main() {}\n");
    mkdirSync(resolve(project, "src"));
    writeFileSync(resolve(project, "src/main.rs"), "fn main() {}\n");
    for (const scenario of ["bypass", "scheduled"]) {
      const target = resolve(directory, `${scenario}-target-${job}`);
      mkdirSync(resolve(target, "debug/build"), { recursive: true });
      writeFileSync(resolve(target, "CACHEDIR.TAG"), "fixture only\n");
    }
  }
  mkdirSync(resolve(directory, "target/traffic/measurements"), { recursive: true });
  const ref = (name: string, value: any) => {
    const path = `target/traffic/measurements/${name}.json`;
    const bytes = Buffer.from(JSON.stringify(value) + "\n");
    writeFileSync(resolve(directory, path), bytes);
    return { path, sha256: hash(bytes) };
  };
  const rawRef = (name: string, bytes: Buffer) => {
    const path = `target/traffic/measurements/${name}.log`;
    writeFileSync(resolve(directory, path), bytes);
    return { path, sha256: hash(bytes) };
  };
  const executable = resolve(directory, "cargo-fixture");
  writeFileSync(executable, "fixture executable\n"); chmodSync(executable, 0o755);
  const executableSha256 = hash(readFileSync(executable));
  const base = (scenario: string, event: string, job: number, at: number, extra: any = {}) => ({ scenario, event, job, at, ...extra });
  const start = (scenario: string, job: number, at: number) => base(scenario, "start", job, at,
    { pid: 100 + job, cwd: resolve(directory, `project-${job}`), target: resolve(directory, `${scenario}-target-${job}`),
      observation: ref(`${scenario}-spawn-${job}`, { schemaVersion: 1, kind: "cargo-spawn", scenario, job, pid: 100 + job,
        cwd: resolve(directory, `project-${job}`), target: resolve(directory, `${scenario}-target-${job}`), argv: ["build"], executable, executableSha256 }) });
  const invoke = (scenario: string, job: number, at: number) => base(scenario, "invocation", job, at, { observation: ref(`${scenario}-invoke-${job}`,
    { schemaVersion: 1, scenario, job, cargoPid: 100 + job, buildPid: 200 + job, cwd: resolve(directory, `project-${job}`), target: resolve(directory, `${scenario}-target-${job}`) }) });
  const observe = (event: string, job: number, at: number, kind: string, data: any, extra = {}) => base("scheduled", event, job, at,
    { observation: ref(`${event}-${job}`, { schemaVersion: 1, kind, scenario: "scheduled", job, ...data }), ...extra });
  const ids = ["a".repeat(32), "b".repeat(32)];
  const events = [start("bypass", 0, 10), invoke("bypass", 0, 11), start("bypass", 1, 12),
    invoke("bypass", 1, 13), base("bypass", "end", 0, 20, { exit: 0 }), base("bypass", "end", 1, 21, { exit: 0 }),
    base("bypass", "completion", 0, 22, { exit: 0 }), base("bypass", "completion", 1, 23, { exit: 0 }),
    start("scheduled", 0, 30), invoke("scheduled", 0, 31), observe("queued", 1, 32, "queue", { jobId: ids[1], state: "QUEUED", queueAtMs: 1000, clientPid: 300 }),
    base("scheduled", "notice", 1, 33, { cliPid: 300, observation: rawRef("notice", Buffer.from("Queued — waiting to start\n")) }),
    observe("remote", 1, 34, "bypass", { classification: "REMOTE", jobId: "c".repeat(32), state: "RUNNING", childPid: 400,
      permitCount: 0, activePermitJobId: ids[0], activePermitCount: 1, queuedJobId: ids[1], queuedState: "QUEUED" }),
    observe("unknown", 1, 35, "bypass", { classification: "UNKNOWN", jobId: "d".repeat(32), state: "RUNNING", childPid: 401,
      permitCount: 0, activePermitJobId: ids[0], activePermitCount: 1, queuedJobId: ids[1], queuedState: "QUEUED" }),
    base("scheduled", "end", 0, 40, { exit: 0 }), observe("release", 0, 41, "release", { jobId: ids[0], state: "RELEASED", committed: true, activePermitCount: 0 }),
    observe("completion", 0, 42, "completion", { jobId: ids[0], state: "COMPLETED", outcomeKind: "EXITED", exitCode: 0, childPid: 100 }, { exit: 0 }),
    observe("grant", 1, 43, "grant", { jobId: ids[1], state: "RESERVED", committed: true, reservedAtMs: 1013 }), start("scheduled", 1, 44),
    invoke("scheduled", 1, 45), observe("wait", 1, 46, "admission", { jobId: ids[1], queueAtMs: 1000, reservedAtMs: 1013 }, { ms: 13 }),
    base("scheduled", "end", 1, 50, { exit: 0 }), observe("completion", 1, 51, "completion", { jobId: ids[1], state: "COMPLETED", outcomeKind: "EXITED", exitCode: 0, childPid: 101 }, { exit: 0 })];
  return { directory, events };
}
function fixtureOutput(events: any[]) {
  return `running 1 test\ntest ${acceptanceTest} ... ` + events.map(e => `TRAFFIC_EVENT ${JSON.stringify(e)}`).join("\n") +
    "\nok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1s\n";
}
test("fixture measurement is derived from native events and rejects missing or altered observations", () => {
  const { directory, events } = fixtureEvents(), raw = fixtureOutput(events);
  const window = { started: new Date(Date.now() - 60_000).toISOString(), ended: new Date(Date.now() + 60_000).toISOString() };
  const summary = measurementFromOutput(directory, raw, window);
  expect(summary.bypass).toMatchObject({ peakConcurrentBuilds: 2, invocations: [1, 1] });
  expect(summary.scheduled).toMatchObject({ peakConcurrentBuilds: 1, queueWaitMs: 13, completionExitCodes: [0, 0] });
  expect(validateMeasurement(directory, summary, raw, window)).toEqual(summary);
  const outputBeforeName = "running 1 test\n" + events.map(e => `TRAFFIC_EVENT ${JSON.stringify(e)}`).join("\n") +
    `\ntest ${acceptanceTest} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1s\n`;
  expect(validateMeasurement(directory, summary, outputBeforeName, window)).toEqual(summary);
  expect(() => validateMeasurement(directory, summary, outputBeforeName.replace("running 1 test", "running 2 tests"), window)).toThrow("did not execute");
  expect(() => validateMeasurement(directory, { ...summary, bypass: { ...summary.bypass, peakConcurrentBuilds: 1 } }, raw, window)).toThrow();
  expect(() => measurementFromOutput(directory, fixtureOutput(events.filter(e => e.event !== "queued")), window)).toThrow();
  expect(() => measurementFromOutput(directory, fixtureOutput(events.filter(e => e.event !== "notice")), window)).toThrow("notice");
  const noNotice = structuredClone(events), noNoticeEvent = noNotice.find((e: any) => e.event === "notice");
  const noticePath = resolve(directory, noNoticeEvent.observation.path), noticeBytes = readFileSync(noticePath), noNoticeBytes = Buffer.from("still starting\n");
  writeFileSync(noticePath, noNoticeBytes); noNoticeEvent.observation.sha256 = hash(noNoticeBytes);
  expect(() => measurementFromOutput(directory, fixtureOutput(noNotice), window)).toThrow("queued notice missing");
  writeFileSync(noticePath, noticeBytes);
  const late = structuredClone(events), lateNotice = late.find((e: any) => e.event === "notice");
  late.splice(late.indexOf(lateNotice), 1); lateNotice.at = 44; late.splice(late.findIndex((e: any) => e.event === "invocation" && e.scenario === "scheduled" && e.job === 1), 0, lateNotice);
  expect(() => measurementFromOutput(directory, fixtureOutput(late), window)).toThrow("ordering");
  expect(() => measurementFromOutput(directory, fixtureOutput(events.filter(e => e.event !== "grant")), window)).toThrow("grant");
  expect(() => measurementFromOutput(directory, fixtureOutput(events.filter(e => e.event !== "remote")), window)).toThrow("remote");
  expect(() => measurementFromOutput(directory, fixtureOutput(events.filter(e => e.event !== "unknown")), window)).toThrow("unknown");
  for (const [kind, patch, message] of [
    ["release", { committed: false }, "queue, permit or completion"],
    ["grant", { committed: false }, "queue, permit or completion"],
    ["remote", { permitCount: 1 }, "REMOTE bypass"],
    ["unknown", { queuedState: "RUNNING" }, "UNKNOWN bypass"],
  ] as const) {
    const changed = structuredClone(events), row = changed.find((e: any) => e.event === kind);
    const path = resolve(directory, row.observation.path), original = readFileSync(path);
    const edited = Buffer.from(JSON.stringify({ ...JSON.parse(original.toString()), ...patch }) + "\n");
    writeFileSync(path, edited); row.observation.sha256 = hash(edited);
    expect(() => measurementFromOutput(directory, fixtureOutput(changed), window)).toThrow(message);
    writeFileSync(path, original);
  }
  const bad = structuredClone(events); bad.find((e: any) => e.scenario === "scheduled" && e.event === "start" && e.job === 1)!.at = 39;
  expect(() => measurementFromOutput(directory, fixtureOutput(bad), window)).toThrow();
  const shared = structuredClone(events), starts = shared.filter((e: any) => e.scenario === "bypass" && e.event === "start");
  starts[1].cwd = starts[0].cwd;
  expect(() => measurementFromOutput(directory, fixtureOutput(shared), window)).toThrow();
  expect(() => validateMeasurement(directory, summary, raw.replace("0 ignored", "1 ignored"), window)).toThrow();
  expect(() => validateMeasurement(directory, summary, raw.replace(`test ${acceptanceTest} ...`, "no test identity"), window)).toThrow("did not execute");
  expect(() => validateMeasurement(directory, summary, raw.replace("running 1 test\n", ""), window)).toThrow("did not execute");
  expect(() => validateMeasurement(directory, summary, raw.replace(/^test result:.*$/m, ""), window)).toThrow("did not execute");
  const stale = { started: new Date(Date.now() - 120_000).toISOString(), ended: new Date(Date.now() - 60_000).toISOString() };
  expect(() => measurementFromOutput(directory, raw, stale)).toThrow("stale measurement observation");
  const eventOnly = structuredClone(events); eventOnly.forEach((e: any) => delete e.observation);
  expect(() => measurementFromOutput(directory, fixtureOutput(eventOnly), window)).toThrow("missing artifact hash");
  const unrelated = structuredClone(events), unrelatedStart = unrelated.find((e: any) => e.event === "start");
  unrelatedStart.target = resolve(root, "scripts/coverage");
  expect(() => measurementFromOutput(directory, fixtureOutput(unrelated), window)).toThrow();
  const invoke: any = events.find(e => e.event === "invocation")!, marker = resolve(directory, invoke.observation.path);
  const markerBytes = readFileSync(marker);
  writeFileSync(marker, "fabricated marker\n");
  expect(() => measurementFromOutput(directory, raw, window)).toThrow("altered artifact");
  const repeated = Buffer.concat([markerBytes, markerBytes]);
  writeFileSync(marker, repeated);
  const duplicate = structuredClone(events), duplicateInvoke: any = duplicate.find((e: any) => e.event === "invocation");
  duplicateInvoke.observation.sha256 = hash(repeated);
  expect(() => measurementFromOutput(directory, fixtureOutput(duplicate), window)).toThrow("marker missing or repeated");
  writeFileSync(marker, markerBytes);
  const cached = structuredClone(events), cachedStart = cached.find((e: any) => e.scenario === "scheduled" && e.event === "start" && e.job === 1);
  const launchPath = resolve(directory, cachedStart.observation.path), launch = readFileSync(launchPath);
  const secondLaunch = JSON.parse(launch.toString()); secondLaunch.pid = 999;
  const cachedBytes = Buffer.concat([launch, Buffer.from(JSON.stringify(secondLaunch) + "\n")]);
  writeFileSync(launchPath, cachedBytes); cachedStart.observation.sha256 = hash(cachedBytes);
  expect(() => measurementFromOutput(directory, fixtureOutput(cached), window)).toThrow("Cargo launch record missing or repeated");
  writeFileSync(launchPath, launch);
  const fabricatedQueue = structuredClone(events), queued: any = fabricatedQueue.find((e: any) => e.event === "queued");
  const queuePath = resolve(directory, queued.observation.path), queue = JSON.parse(readFileSync(queuePath, "utf8"));
  queue.state = "RUNNING";
  const queueBytes = Buffer.from(JSON.stringify(queue) + "\n");
  writeFileSync(queuePath, queueBytes); queued.observation.sha256 = hash(queueBytes);
  expect(() => measurementFromOutput(directory, fixtureOutput(fabricatedQueue), window)).toThrow("queue, permit or completion observation differs");
});

test("fixture full collectors reject filtered Cargo and Bun results", () => {
  const cargo = "running 1 test\ntest example ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.0s\n";
  expect(validateFullCargoOutput(cargo).passed).toBe(1);
  expect(() => validateFullCargoOutput(cargo.replace("0 filtered out", "12 filtered out"))).toThrow("filtered");
  expect(() => validateFullCargoOutput(cargo.replace("test example ... ok", "test example ... ignored").replace("1 passed; 0 failed; 0 ignored", "0 passed; 0 failed; 1 ignored"))).toThrow();
  expect(() => validateLiveBunOutput("2 pass\n0 fail\n12 filtered out\n")).toThrow("fully");
  expect(() => validateLiveBunOutput("2 pass\n0 fail\n12 tests filtered\n")).toThrow("fully");
  expect(() => validateLiveBunOutput("2 pass\n0 fail\n")).not.toThrow();
  expect(() => validateLiveBunOutput("(pass) rejects filtered results\n2 pass\n0 fail\n")).not.toThrow();
});

test("fixture final measurement requires recorded values but accepts multiple formats", () => {
  const output = "running 1 test\ntest two_worktrees ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1s\n";
  const summary = { platform: "macOS", independentDirectoriesVerified: true,
    bypass: { peakConcurrentBuilds: 2, invocations: [1, 1] },
    scheduled: { peakConcurrentBuilds: 1, invocations: [1, 1], queuedBeforeFirstRelease: true,
      secondStartedAfterRelease: true, queueWaitRecorded: true, completionOutcomesRecorded: true } };
  const window = { started: "2026-01-01T00:00:00Z", ended: "2026-01-01T00:00:01Z" };
  const observed = JSON.stringify({ bypass: summary.bypass, scheduled: { ...summary.scheduled, queueWaitMs: 13, completionExitCodes: [0, 0] } });
  const recorded = output + observed + "\n";
  expect(validateMeasurement(root, summary, recorded, window)).toEqual(summary);
  const namedOutcomes = output + JSON.stringify({ bypass: summary.bypass, scheduled: { ...summary.scheduled,
    waitMs: 13, completionOutcomes: [{ outcomeKind: "EXITED", exitCode: 0 }, "exited 0"] } }) + "\n";
  expect(validateMeasurement(root, summary, namedOutcomes, window)).toEqual(summary);
  const labeled = "bypass peak concurrent builds: 2 invocations: 1,1\nscheduled peak concurrent builds: 1 invocations: 1,1 queue wait ms: 13 completion exit codes: 0,0\n";
  expect(validateMeasurement(root, summary, output, window, labeled)).toEqual(summary);
  expect(() => validateMeasurement(root, summary, output, window)).toThrow("recorded measurement values missing");
  expect(() => validateMeasurement(root, summary, recorded.replace('"queueWaitMs":13,', ""), window)).toThrow("recorded measurement values missing");
  expect(() => validateMeasurement(root, summary, recorded.replace(',"completionExitCodes":[0,0]', ""), window)).toThrow("recorded measurement values missing");
  expect(() => validateMeasurement(root, { ...summary, scheduled: { ...summary.scheduled, queueWaitMs: 14 } }, recorded, window)).toThrow("recorded measurement values missing");
  expect(() => validateMeasurement(root, { ...summary, scheduled: { ...summary.scheduled, peakConcurrentBuilds: 2 } }, recorded, window)).toThrow("fixed contract");
  expect(() => validateMeasurement(root, { ...summary, scheduled: { ...summary.scheduled, queueWaitRecorded: false } }, recorded, window)).toThrow("fixed contract");
  expect(() => validateMeasurement(root, summary, recorded.replace("0 filtered out", "1 filtered out"), window)).toThrow("without filtering");
});

test("fixture receipt rejects stale source, skips and altered raw logs", async () => {
  const ref = await capture(root, "validator-fixture", ["bun", "-e", "process.stdout.write('fixture')"]);
  const context = sourceContext(root), receipt = boundJSON(root, ref, "target/traffic");
  expect(validateReceipt(root, receipt, context).stdout).toBe("fixture");
  expect(() => validateReceipt(root, { ...receipt, sourceHash: "0".repeat(64) }, context)).toThrow("stale source");
  expect(() => validateReceipt(root, { ...receipt, skipFlags: true }, context)).toThrow("skipped");
  const alternative = `target/task5-fixture-${crypto.randomUUID()}`;
  mkdirSync(resolve(root, alternative));
  const bytes = Buffer.from("fixture"), empty = Buffer.alloc(0);
  writeFileSync(resolve(root, alternative, "stdout.log"), bytes);
  writeFileSync(resolve(root, alternative, "stderr.log"), empty);
  expect(validateReceipt(root, { ...receipt, output: {
    stdout: { path: `${alternative}/stdout.log`, sha256: hash(bytes) },
    stderr: { path: `${alternative}/stderr.log`, sha256: hash(empty) },
  } }, context).stdout).toBe("fixture");
  writeFileSync(resolve(root, receipt.output.stdout.path), "altered");
  expect(() => validateReceipt(root, receipt, context)).toThrow("altered artifact");
}, 7200000);

test("fixture harness collection cannot be skipped or asserted without native evidence", () => {
  expect(() => validateCollector(root, {}, "SKIP_COVERAGE=1 set — skipping coverage gate")).toThrow("skipped coverage");
  expect(() => validateCollector(root, {}, "All checks passed (0 violations).\n")).toThrow("actual coverage collection missing");
});

test("fixture nested coverage output and latest collection remain bound", () => {
  const dir = realpathSync(mkdtempSync(resolve(tmpdir(), "traffic-nested-")));
  mkdirSync(resolve(dir, "target/traffic/commands/fixture"), { recursive: true });
  const run = "native-one", nativePath = `.arc-verify/rust-coverage/baseline/runs/${run}/receipt.json`;
  mkdirSync(resolve(dir, nativePath, ".."), { recursive: true });
  const stdout = "running 1 test\ntest example ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.0s\n";
  const argv = ["rustup", "run", "1.95.0", "cargo", "llvm-cov", "--workspace", "--all-targets", "--no-report"];
  const rawErr = Buffer.from([0xff, 0x0a]), outputBytes = nativeOutput(argv, dir, Buffer.from(stdout), rawErr)!;
  const test = { argv, exit: 0, pid: 42, cwd: dir, started: "2026-01-01T00:00:01Z", ended: "2026-01-01T00:00:02Z",
    stdout, stderr: rawErr.toString(), output: outputBytes };
  const native = Buffer.from(JSON.stringify({ runId: run, receipts: [test] })); writeFileSync(resolve(dir, nativePath), native);
  const output = `Coverage reports: .arc-verify/rust-coverage/baseline/runs/${run}\n`;
  const binding = captureCoverage(dir, "target/traffic/commands/fixture", output);
  const receipt = { started: "2026-01-01T00:00:00Z", ended: "2026-01-01T00:00:03Z", nestedCoverage: binding,
    output: { stdout: { path: "target/traffic/commands/fixture/stdout.log" }, stderr: { path: "target/traffic/commands/fixture/stderr.log" } } };
  expect(validateNestedCoverage(dir, receipt, output)).toBe(run);
  writeFileSync(resolve(dir, nativePath), JSON.stringify({ runId: run, receipts: [{ ...test, stdout: stdout.replace("example", "altered") }] }));
  expect(() => validateNestedCoverage(dir, receipt, output)).toThrow("nested native receipt altered");
  writeFileSync(resolve(dir, nativePath), native);
  writeFileSync(resolve(dir, binding.stderr.path), "altered nested stderr\n");
  expect(() => validateNestedCoverage(dir, receipt, output)).toThrow("altered artifact");
  writeFileSync(resolve(dir, binding.stderr.path), rawErr);
  writeFileSync(resolve(dir, outputBytes.stderr.path), "altered original stderr\n");
  expect(() => validateNestedCoverage(dir, receipt, output)).toThrow("altered artifact");
  writeFileSync(resolve(dir, outputBytes.stderr.path), rawErr);
  expect(() => validateNestedCoverage(dir, { ...receipt, nestedCoverage: undefined }, output)).toThrow("binding missing");
  expect(() => validateNestedCoverage(dir, { ...receipt, nestedCoverage: { ...binding, stderr: binding.stdout } }, output)).toThrow("containing collection");
  const latest = { runId: run, directory: `.arc-verify/rust-coverage/baseline/runs/${run}`, pass: true, sourceHash: "same" };
  expect(() => validateLatestCollection(latest, run, "same")).not.toThrow();
  expect(() => validateLatestCollection({ ...latest, runId: "later", directory: ".arc-verify/rust-coverage/baseline/runs/later" }, run, "same")).not.toThrow();
  expect(() => validateLatestCollection({ ...latest, sourceHash: "stale" }, run, "same")).toThrow("stale");
});

test("fixture native subprocess streams retain their actual non-UTF-8 bytes", async () => {
  const dir = realpathSync(mkdtempSync(resolve(tmpdir(), "traffic-native-output-"))), bin = resolve(dir, "bin");
  mkdirSync(bin); const rustup = resolve(bin, "rustup");
  writeFileSync(rustup, "#!/bin/sh\nprintf 'cargo output\\n'\nprintf '\\377raw\\n' >&2\n"); chmodSync(rustup, 0o755);
  const argv = ["rustup", "run", "fixture", "cargo", "llvm-cov", "--workspace", "--all-targets", "--no-report"];
  const result = await command(argv, dir, dir, { ...process.env, PATH: `${bin}:/usr/bin:/bin` });
  expect(result.exit).toBe(0);
  const stderr = readFileSync(resolve(dir, result.output!.stderr.path));
  expect(stderr).toEqual(Buffer.from([0xff, 0x72, 0x61, 0x77, 0x0a]));
  expect(hash(stderr)).toBe(result.output!.stderr.sha256);
  expect(result.stderr).toBe(stderr.toString());
  expect(readFileSync(resolve(dir, result.output!.stdout.path)).toString()).toBe("cargo output\n");
}, 7200000);

test("fixture evidence budget includes indexed and untracked files and decimal bytes", () => {
  const dir = mkdtempSync(resolve(tmpdir(), "traffic-evidence-budget-"));
  gitBytes(dir, ["init", "--quiet"]);
  gitBytes(dir, ["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "--quiet", "--allow-empty", "-m", "baseline"]);
  const baseline = gitBytes(dir, ["rev-parse", "HEAD"]).toString().trim();
  mkdirSync(resolve(dir, ".arc-verify/traffic"), { recursive: true });
  writeFileSync(resolve(dir, ".arc-verify/traffic/one.md"), "one");
  gitBytes(dir, ["add", "--", ".arc-verify/traffic/one.md"]);
  writeFileSync(resolve(dir, ".arc-verify/traffic/two.md"), "two");
  expect(evidenceBudget(dir, baseline)).toEqual({ files: 2, indexed: 1, bytes: 6 });
  mkdirSync(resolve(dir, "scripts/coverage"), { recursive: true });
  writeFileSync(resolve(dir, "scripts/coverage/traffic-evidence-contract.md"), "handoff");
  expect(evidenceBudget(dir, baseline)).toEqual({ files: 2, indexed: 1, bytes: 6 });
  writeFileSync(resolve(dir, "scripts/coverage/outside-handoff.md"), "outside");
  expect(evidenceBudget(dir, baseline)).toEqual({ files: 2, indexed: 1, bytes: 6 });
  writeFileSync(resolve(dir, "scripts/coverage/outside-handoff.txt"), "other");
  expect(evidenceBudget(dir, baseline)).toEqual({ files: 2, indexed: 1, bytes: 6 });
  writeFileSync(resolve(dir, "outside-root.md"), "root");
  expect(evidenceBudget(dir, baseline)).toEqual({ files: 2, indexed: 1, bytes: 6 });
  mkdirSync(resolve(dir, "docs"), { recursive: true });
  mkdirSync(resolve(dir, "crates/arc-traffic"), { recursive: true });
  writeFileSync(resolve(dir, "docs/managed-exec.md"), "product guide");
  writeFileSync(resolve(dir, "crates/arc-traffic/README.md"), "crate readme");
  expect(evidenceBudget(dir, baseline)).toEqual({ files: 2, indexed: 1, bytes: 6 });
  writeFileSync(resolve(dir, ".arc-verify/traffic/one.md"), "altered");
  expect(evidenceBudget(dir, baseline)).toEqual({ files: 2, indexed: 1, bytes: 10 });
  writeFileSync(resolve(dir, ".arc-verify/traffic/one.md"), "one");
  for (let i = 0; i < 27; i++) writeFileSync(resolve(dir, `.arc-verify/traffic/${i}.md`), "x");
  expect(evidenceBudget(dir, baseline).files).toBe(29);
  writeFileSync(resolve(dir, ".arc-verify/traffic/another-handoff.md"), "x");
  expect(() => evidenceBudget(dir, baseline)).toThrow("count must be below 30");
  const other = mkdtempSync(resolve(tmpdir(), "traffic-evidence-bytes-"));
  gitBytes(other, ["init", "--quiet"]);
  gitBytes(other, ["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "--quiet", "--allow-empty", "-m", "baseline"]);
  const otherBaseline = gitBytes(other, ["rev-parse", "HEAD"]).toString().trim();
  mkdirSync(resolve(other, ".arc-verify/traffic"), { recursive: true });
  writeFileSync(resolve(other, ".arc-verify/traffic/large.md"), Buffer.alloc(5_000_000));
  mkdirSync(resolve(other, "scripts/coverage"), { recursive: true });
  writeFileSync(resolve(other, "scripts/coverage/outside-handoff.md"), "12345");
  expect(() => evidenceBudget(other, otherBaseline)).toThrow("below 5000000");
}, 7200000);

test("live Traffic evidence is source-bound and complete", () => {
  expect(validateFinalTrafficEvidence(root).pass).toBe(true);
});
