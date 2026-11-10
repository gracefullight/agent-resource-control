import { isAbsolute, resolve, sep } from "node:path";
import { existsSync, lstatSync, readFileSync, realpathSync, statSync } from "node:fs";
import { boundBytes, equal, requireTraffic } from "./traffic-receipts";
import { hash } from "./source";

type Ref = { path: string; sha256: string };
type Event = { scenario: "bypass" | "scheduled"; event: string; job: number; at: number;
  pid?: number; cliPid?: number; cwd?: string; target?: string; exit?: number; ms?: number; observation?: Ref };
export const acceptanceTest = "two_independent_cargo_builds_measure_traffic";
const separated = (a: string, b: string) => a !== b && !a.startsWith(b + sep) && !b.startsWith(a + sep);

function directoryIdentity(path: string) {
  const stat = statSync(path);
  requireTraffic(stat.isDirectory() && realpathSync(path) === path, "build directory missing or noncanonical");
  return `${stat.dev}:${stat.ino}`;
}
function regular(path: string) {
  const stat = lstatSync(path);
  requireTraffic(stat.isFile() && !stat.isSymbolicLink() && realpathSync(path) === path, "Cargo project or target artifact missing");
}
function cargoProject(cwd: string, target: string) {
  for (const path of [resolve(cwd, "Cargo.toml"), resolve(cwd, "build.rs"), resolve(target, "CACHEDIR.TAG")]) regular(path);
  const source = ["src/main.rs", "src/lib.rs"].map(path => resolve(cwd, path)).find(existsSync);
  requireTraffic(source, "Cargo source target missing");
  regular(source);
  requireTraffic(statSync(resolve(target, "debug/build")).isDirectory(), "Cargo build artifacts missing");
  const manifest: any = Bun.TOML.parse(readFileSync(resolve(cwd, "Cargo.toml"), "utf8"));
  requireTraffic(typeof manifest?.package?.name === "string" && manifest.package.name.length > 0 &&
    manifest.package.build !== false, "working directory is not a Cargo build-script project");
  return manifest.package.name;
}

// With --nocapture libtest can put output after a test-name prefix, or print
// output before its completed test line. Bind every event to the named test
// and an unfiltered harness summary in either format.
function assertAcceptanceTest(stdout: string) {
  const lines = stdout.split(/\r?\n/);
  const start = lines.findIndex(line => line.startsWith(`test ${acceptanceTest} ...`));
  requireTraffic(start >= 0 && lines.filter(line => line.startsWith(`test ${acceptanceTest} ...`)).length === 1,
    "executed CLI acceptance test identity missing");
  const first = lines.findIndex(line => line.includes("TRAFFIC_EVENT "));
  const last = lines.findLastIndex(line => line.includes("TRAFFIC_EVENT "));
  const running = lines.findLastIndex((line, i) => i < Math.min(start, first) && /^running [1-9]\d* tests?$/.test(line));
  const end = lines.findIndex((line, i) => i >= last && /^(?:ok|test result:)/.test(line));
  const prefixed = first >= start && last >= first && end > last && lines[end] === "ok" &&
    lines.slice(start + 1, end).every(line => !/^test .+ \.\.\./.test(line));
  // When output precedes its test identity, a one-test harness prevents another
  // concurrently executing test from supplying the event stream.
  const completedLine = first > running && last < start && lines[start] === `test ${acceptanceTest} ... ok` &&
    lines[running] === "running 1 test";
  const finish = prefixed ? end : start;
  const summary = lines.findIndex((line, i) => i > finish && /^test result:/.test(line));
  const verdicts = [...stdout.matchAll(/^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;/gm)];
  requireTraffic(running >= 0 && lines.slice(running + 1, start).every(line => !/^test result:/.test(line)) &&
    first >= 0 && last >= first && (prefixed || completedLine) && summary > finish &&
    /^test result: ok\. [1-9]\d* passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;/.test(lines[summary]) &&
    (!completedLine || /^test result: ok\. 1 passed;/.test(lines[summary])) &&
    verdicts.length > 0 && verdicts.some(v => Number(v[1]) > 0) &&
    verdicts.every(v => Number(v[2]) === 0 && Number(v[3]) === 0 && Number(v[4]) === 0 && Number(v[5]) === 0),
    "CLI acceptance test did not execute successfully without filtering");
}

type Window = { started: string; ended: string };
function freshBytes(root: string, ref: Ref | undefined, window: Window) {
  const bytes = boundBytes(root, ref, "target/traffic");
  const modified = statSync(resolve(root, ref!.path)).mtimeMs;
  requireTraffic(modified >= Date.parse(window.started) && modified <= Date.parse(window.ended), "stale measurement observation");
  return bytes;
}
function observation(root: string, event: Event, kind: string, window: Window) {
  const data = JSON.parse(freshBytes(root, event.observation, window).toString());
  requireTraffic(data?.schemaVersion === 1 && data.kind === kind && data.scenario === event.scenario && data.job === event.job,
    `invalid ${kind} observation`);
  return data;
}

// Paths and hashes name raw files written by the executed CLI test. This is a
// correlation check, not proof against a test intentionally fabricating data.
export function measurementFromOutput(root: string, stdout: string, window: Window) {
  requireTraffic(Number.isFinite(Date.parse(window?.started)) && Number.isFinite(Date.parse(window?.ended)) &&
    Date.parse(window.ended) >= Date.parse(window.started), "invalid measurement command window");
  assertAcceptanceTest(stdout);
  const marker = "TRAFFIC_EVENT ";
  const events: Event[] = stdout.split(/\r?\n/).filter(line => line.includes(marker)).map(line => {
    const event = JSON.parse(line.slice(line.indexOf(marker) + marker.length));
    requireTraffic(event && ["bypass", "scheduled"].includes(event.scenario) &&
      ["start", "invocation", "end", "queued", "notice", "release", "grant", "wait", "completion", "remote", "unknown"].includes(event.event) &&
      [0, 1].includes(event.job) && Number.isSafeInteger(event.at) && event.at >= 0, "invalid measurement event");
    return event;
  });
  const refs = events.filter(e => e.observation).map(e => e.observation!.path);
  requireTraffic(new Set(refs).size === refs.length, "reused measurement observation");
  const counts = ["bypass", "scheduled"].map(scenario => {
    const rows = events.filter(e => e.scenario === scenario);
    requireTraffic(rows.length > 0 && rows.every((e, i) => i === 0 || e.at >= rows[i - 1].at), "measurement event ordering differs");
    const one = (kind: string, job: number) => {
      const matches = rows.filter(e => e.event === kind && e.job === job);
      requireTraffic(matches.length === 1, `missing or duplicate ${scenario} ${kind} ${job}`);
      return matches[0];
    };
    const starts = [one("start", 0), one("start", 1)], ends = [one("end", 0), one("end", 1)];
    const invocations = [one("invocation", 0), one("invocation", 1)], completions = [one("completion", 0), one("completion", 1)];
    for (const i of [0, 1]) {
      const s = starts[i], e = ends[i], marker = invocations[i], outcome = completions[i];
      requireTraffic(Number.isSafeInteger(s.pid) && s.pid! > 0 && typeof s.cwd === "string" && isAbsolute(s.cwd) && resolve(s.cwd) === s.cwd &&
        typeof s.target === "string" && isAbsolute(s.target) && resolve(s.target) === s.target && separated(s.cwd, s.target) &&
        s.at <= marker.at && marker.at <= e.at && e.at < outcome.at && e.exit === 0 && outcome.exit === 0, "invalid build observation");
      cargoProject(s.cwd, s.target);
      // The launch shim records every exec attempt before replacing itself with
      // Cargo. This stream is independent of build.rs and catches cache hits.
      const launches = freshBytes(root, s.observation, window).toString().split(/\r?\n/).filter(Boolean);
      requireTraffic(launches.length === 1, "Cargo launch record missing or repeated");
      const launch = JSON.parse(launches[0]);
      requireTraffic(launch?.schemaVersion === 1 && launch.kind === "cargo-spawn" && launch.scenario === scenario && launch.job === i &&
        launch.pid === s.pid && launch.cwd === s.cwd && launch.target === s.target && equal(launch.argv, ["build"]) &&
        typeof launch.executable === "string" && isAbsolute(launch.executable) && realpathSync(launch.executable) === launch.executable &&
        statSync(launch.executable).isFile() && (statSync(launch.executable).mode & 0o111) !== 0 &&
        launch.executableSha256 === hash(readFileSync(launch.executable)), "Cargo launch record differs");
      // Build scripts append one JSON line per invocation. Two identical lines
      // are still two executions, and cannot be hidden by a summary counter.
      const bytes = freshBytes(root, marker.observation, window).toString();
      const lines = bytes.split(/\r?\n/).filter(Boolean);
      requireTraffic(lines.length === 1, "build invocation marker missing or repeated");
      const invocation = JSON.parse(lines[0]);
      requireTraffic(invocation?.schemaVersion === 1 && invocation.scenario === scenario && invocation.job === i &&
        invocation.cargoPid === s.pid && Number.isSafeInteger(invocation.buildPid) && invocation.buildPid > 0 &&
        invocation.buildPid !== s.pid && invocation.cwd === s.cwd && invocation.target === s.target,
        "build invocation marker differs from process observation");
    }
    requireTraffic(starts[0].pid !== starts[1].pid && separated(starts[0].cwd!, starts[1].cwd!) &&
      separated(starts[0].target!, starts[1].target!) && starts.every(s => starts.every(t => separated(s.cwd!, t.target!))) &&
      directoryIdentity(starts[0].cwd!) !== directoryIdentity(starts[1].cwd!) &&
      directoryIdentity(starts[0].target!) !== directoryIdentity(starts[1].target!), "working or target directories are not independent");
    const changes = starts.map(s => ({ at: s.at, delta: 1 })).concat(ends.map(e => ({ at: e.at, delta: -1 })))
      .sort((a, b) => a.at - b.at || a.delta - b.delta);
    let active = 0, peak = 0;
    for (const change of changes) { active += change.delta; peak = Math.max(peak, active); requireTraffic(active >= 0, "invalid build intervals"); }
    requireTraffic(active === 0, "unfinished builds");
    if (scenario === "bypass") {
      requireTraffic(peak === 2 && rows.every(e => !["queued", "notice", "release", "grant", "wait", "remote", "unknown"].includes(e.event)), "bypassed builds did not overlap");
      return { peakConcurrentBuilds: peak, invocations: [1, 1] };
    }
    const queued = one("queued", 1), notice = one("notice", 1), release = one("release", 0), grant = one("grant", 1), wait = one("wait", 1);
    const q = observation(root, queued, "queue", window), w = observation(root, wait, "admission", window), released = observation(root, release, "release", window);
    const granted = observation(root, grant, "grant", window);
    const cliOutput = freshBytes(root, notice.observation, window).toString();
    requireTraffic(Number.isSafeInteger(notice.cliPid) && notice.cliPid! > 0 && notice.cliPid === q.clientPid &&
      /\bqueued\b/i.test(cliOutput), "second CLI queued notice missing or unbound");
    const outcomes = completions.map(e => observation(root, e, "completion", window));
    const jobId = (value: unknown) => typeof value === "string" && /^[a-f0-9]{32}$/.test(value);
    requireTraffic(jobId(q.jobId) && jobId(released.jobId) && q.jobId !== released.jobId && q.state === "QUEUED" &&
      w.jobId === q.jobId && released.state === "RELEASED" && released.committed === true && released.activePermitCount === 0 &&
      granted.jobId === q.jobId && granted.state === "RESERVED" && granted.committed === true && granted.reservedAtMs === w.reservedAtMs &&
      outcomes[0].jobId === released.jobId && outcomes[1].jobId === q.jobId &&
      outcomes.every((o, i) => o.state === "COMPLETED" && o.outcomeKind === "EXITED" && o.exitCode === 0 && o.childPid === starts[i].pid) &&
      Number.isSafeInteger(q.queueAtMs) && q.queueAtMs >= 0 && w.queueAtMs === q.queueAtMs &&
      Number.isSafeInteger(w.reservedAtMs) && w.reservedAtMs > w.queueAtMs && wait.ms === w.reservedAtMs - w.queueAtMs,
      "queue, permit or completion observation differs");
    const bypassObservations = [];
    for (const [event, classification] of [["remote", "REMOTE"], ["unknown", "UNKNOWN"]]) {
      const observed = one(event, 1), bypass = observation(root, observed, "bypass", window);
      requireTraffic(bypass.classification === classification && jobId(bypass.jobId) &&
        bypass.jobId !== q.jobId && bypass.jobId !== released.jobId && bypass.state === "RUNNING" &&
        Number.isSafeInteger(bypass.childPid) && bypass.childPid > 0 && bypass.permitCount === 0 &&
        bypass.activePermitJobId === released.jobId && bypass.activePermitCount === 1 &&
        bypass.queuedJobId === q.jobId && bypass.queuedState === "QUEUED" &&
        queued.at < observed.at && observed.at < ends[0].at, `${classification} bypass while queue full not established`);
      bypassObservations.push(bypass);
    }
    requireTraffic(bypassObservations[0].jobId !== bypassObservations[1].jobId && bypassObservations[0].childPid !== bypassObservations[1].childPid,
      "bypass observations must identify distinct jobs and processes");
    requireTraffic(rows.filter(e => ["queued", "notice", "release", "grant", "wait", "remote", "unknown"].includes(e.event)).length === 7 &&
      peak === 1 && starts[0].at <= queued.at && queued.at <= notice.at && notice.at < ends[0].at &&
      ends[0].at <= release.at && release.at < grant.at && grant.at < starts[1].at &&
      starts[1].at <= wait.at && wait.at <= completions[1].at,
      "scheduled queue or release ordering differs");
    return { peakConcurrentBuilds: peak, invocations: [1, 1], queuedBeforeFirstRelease: true, secondStartedAfterRelease: true,
      queueWaitRecorded: true, completionOutcomesRecorded: true, queueWaitMs: wait.ms, completionExitCodes: [0, 0] };
  });
  requireTraffic(events.length === 23, "unexpected measurement events");
  return { platform: "macOS", independentDirectoriesVerified: true, bypass: counts[0], scheduled: counts[1] };
}
function recordedValues(output: string) {
  const rows: any[] = [];
  for (const line of output.split(/\r?\n/)) {
    // A concise human-readable line with labeled values is also supported.
    const scenario = line.match(/\b(bypass(?:ed)?|scheduled)\b/i)?.[1].toLowerCase();
    const peak = line.match(/\bpeak[\s_-]*(?:concurrent[\s_-]*)?builds\s*[:=]\s*(\d+)/i);
    const invocations = line.match(/\binvocations\s*[:=]\s*\[?\s*(\d+)\s*[,/]\s*(\d+)\s*\]?/i);
    const wait = line.match(/\bqueue[\s_-]*wait(?:[\s_-]*ms)?\s*[:=]\s*(\d+(?:\.\d+)?)/i);
    const outcomes = line.match(/\bcompletion(?:[\s_-]*exit[\s_-]*codes|[\s_-]*outcomes)?\s*[:=]\s*\[?\s*(-?\d+)\s*[,/]\s*(-?\d+)\s*\]?/i);
    if (scenario && peak && invocations) rows.push({ scenario: scenario.startsWith("bypass") ? "bypass" : scenario,
      peakConcurrentBuilds: Number(peak[1]), invocations: [Number(invocations[1]), Number(invocations[2])],
      queueWaitMs: wait ? Number(wait[1]) : undefined, completionExitCodes: outcomes ? [Number(outcomes[1]), Number(outcomes[2])] : undefined });
    const start = line.indexOf("{"), end = line.lastIndexOf("}");
    if (start < 0 || end <= start) continue;
    try {
      const value = JSON.parse(line.slice(start, end + 1));
      if (value?.bypass && value?.scheduled) rows.push({ scenario: "bypass", ...value.bypass }, { scenario: "scheduled", ...value.scheduled });
      if (["bypass", "scheduled"].includes(value?.scenario)) rows.push(value);
    } catch { /* Other output is not a measurement record. */ }
  }
  return rows;
}

export function validateMeasurement(root: string, summary: any, stdout: string, window: Window, stderr = "") {
  // Raw logs may contain individual records or a combined JSON observation;
  // the older event stream is also accepted. No event count or spawn shim is
  // required. The independent reviewer still checks how observations arose.
  const running = [...stdout.matchAll(/^running (\d+) tests?$/gm)];
  const results = [...stdout.matchAll(/^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;/gm)];
  requireTraffic(running.length > 0 && running.length === results.length && /^test .+ \.\.\./m.test(stdout) &&
    running.every((count, i) => Number(count[1]) === Number(results[i][1]) &&
      results[i].slice(2).every(value => Number(value) === 0)) &&
    results.some(result => Number(result[1]) > 0), "CLI acceptance test did not execute successfully without filtering");
  const bypass = summary?.bypass, scheduled = summary?.scheduled;
  requireTraffic(summary?.platform === "macOS" && summary.independentDirectoriesVerified === true &&
    bypass?.peakConcurrentBuilds === 2 && equal(bypass.invocations, [1, 1]) &&
    scheduled?.peakConcurrentBuilds === 1 && equal(scheduled.invocations, [1, 1]) &&
    scheduled.queuedBeforeFirstRelease === true && scheduled.secondStartedAfterRelease === true &&
    scheduled.queueWaitRecorded === true && scheduled.completionOutcomesRecorded === true,
    "measurement summary incomplete or differs from the fixed contract");
  const extra = summary.observations ? boundBytes(root, summary.observations, "target").toString() : "";
  const rows = recordedValues(`${stdout}\n${stderr}\n${extra}`);
  const validOutcomes = (outcomes: any) => Array.isArray(outcomes) && outcomes.length === 2 && outcomes.every(value =>
    Number.isSafeInteger(value) || (typeof value === "string" && value.trim().length > 0) ||
    (value && typeof value === "object" && [value.outcomeKind, value.state, value.status].some(v => typeof v === "string" && v.length > 0)));
  const matching = (values: any[], scenario: string, expected: any) => values.some(row => {
    if (row.scenario !== scenario || row.peakConcurrentBuilds !== expected.peakConcurrentBuilds || !equal(row.invocations, expected.invocations)) return false;
    if (scenario !== "scheduled") return true;
    const wait = row.queueWaitMs ?? row.waitMs, outcomes = row.completionExitCodes ?? row.completionOutcomes;
    const expectedOutcomes = expected.completionExitCodes ?? expected.completionOutcomes;
    return Number.isFinite(wait) && wait >= 0 && validOutcomes(outcomes) &&
      (expected.queueWaitMs === undefined || expected.queueWaitMs === wait) &&
      (expectedOutcomes === undefined || equal(expectedOutcomes, outcomes));
  });
  if (!(matching(rows, "bypass", bypass) && matching(rows, "scheduled", scheduled))) {
    requireTraffic(stdout.includes("TRAFFIC_EVENT "), "recorded measurement values missing or inconsistent");
    const derived = measurementFromOutput(root, stdout, window);
    const events = [{ scenario: "bypass", ...derived.bypass }, { scenario: "scheduled", ...derived.scheduled }];
    requireTraffic(matching(events, "bypass", bypass) && matching(events, "scheduled", scheduled), "recorded measurement values missing or inconsistent");
  }
  return { platform: summary.platform, independentDirectoriesVerified: summary.independentDirectoriesVerified, bypass, scheduled };
}
