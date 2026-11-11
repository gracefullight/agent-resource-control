import { lstatSync, readFileSync, realpathSync, statSync } from "node:fs";
import { dirname, isAbsolute, resolve, sep } from "node:path";
import { boundBytes, equal, requireTraffic } from "./traffic-receipts";
import { hash } from "./source";

export type Window = { started: string; ended: string };
const tools = ["cargo", "bun"] as const;
const separated = (a: string, b: string) => a !== b && !a.startsWith(b + sep) && !b.startsWith(a + sep);
const integer = (v: unknown) => Number.isSafeInteger(v) && (v as number) >= 0;
const jobId = (v: unknown) => typeof v === "string" && /^[a-f0-9]{32}$/.test(v);

function inWindow(value: unknown, window: Window) {
  return integer(value) && (value as number) >= Date.parse(window.started) * 1000 &&
    (value as number) <= Date.parse(window.ended) * 1000 + 999;
}
function processIdentity(value: any, window: Window, boot?: string) {
  requireTraffic(value && Number.isSafeInteger(value.pid) && value.pid > 0 &&
    typeof value.start_token === "string" && /^[1-9][0-9]*$/.test(value.start_token) &&
    inWindow(Number(value.start_token), window) && typeof value.boot_id === "string" &&
    /^[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}$/.test(value.boot_id) &&
    (!boot || value.boot_id === boot), "stale or inconsistent native process identity");
}
function executable(value: any) {
  requireTraffic(value && typeof value.path === "string" && isAbsolute(value.path) &&
    value.path === realpathSync(value.path), "noncanonical test executable");
  const stat = lstatSync(value.path);
  requireTraffic(stat.isFile() && !stat.isSymbolicLink() && (stat.mode & 0o111) !== 0 &&
    hash(readFileSync(value.path)) === value.sha256, "test executable altered");
}
function bytes(root: string, ref: any, window: Window) {
  const data = boundBytes(root, ref, "target/traffic-bun");
  const modified = statSync(resolve(root, ref.path)).mtimeMs;
  // Receipt timestamps have millisecond resolution; filesystem timestamps can be sub-millisecond.
  requireTraffic(modified >= Date.parse(window.started) - 1 && modified <= Date.parse(window.ended) + 1,
    "stale measurement artifact");
  return data;
}
function marker(root: string, ref: any, cwd: string, name: string, pid: number, window: Window) {
  requireTraffic(resolve(root, ref?.path ?? "") === resolve(cwd, name), `incorrect ${name} path`);
  const lines = bytes(root, ref, window).toString().split(/\r?\n/).filter(Boolean);
  requireTraffic(lines.length === 1 && lines[0] === String(pid), `missing or duplicate ${name} marker`);
  return lines.length;
}
function directory(path: unknown, root: string) {
  requireTraffic(typeof path === "string" && isAbsolute(path) && path === resolve(path) &&
    path.startsWith(resolve(root, "target/traffic-bun") + sep) && realpathSync(path) === path,
    "noncanonical measurement directory");
  const stat = lstatSync(path);
  requireTraffic(stat.isDirectory() && !stat.isSymbolicLink(), "invalid measurement directory");
  return `${stat.dev}:${stat.ino}`;
}
export function measurementShim(root: string) {
  const source = readFileSync(resolve(root, "crates/arc-daemon/tests/support/bun_measurement.rs"), "utf8");
  const found = source.match(/const SHIM: &str = r##"([\s\S]*?)"##;/);
  // The retained witness is a single non-waiting SQLite read before exec.
  const expected = [
    "#!/bin/sh",
    "printf '%s\\n' \"$$\" >> \"$MIXED_LAUNCH_LOG\" || exit 125",
    "if [ -n \"${MIXED_FIRST_JOB:-}\" ]; then",
    "  case \"$MIXED_FIRST_JOB\" in *[!0123456789abcdef]*|'') exit 125;; esac",
    "  case \"$MIXED_FIRST_TEST_PID\" in *[!0123456789]*|'') exit 125;; esac",
    "  [ \"${#MIXED_FIRST_JOB}\" -eq 32 ] || exit 125",
    "  if kill -0 \"$MIXED_FIRST_TEST_PID\" 2>/dev/null; then exit 125; fi",
    "  observer=$PPID",
    "  state=$(/usr/bin/sqlite3 -readonly -cmd '.timeout 0' -cmd \".parameter set :first \\\"'$MIXED_FIRST_JOB'\\\"\" -cmd \".parameter set :observer $observer\" \"$MIXED_DATABASE\" \"SELECT p.job_id || ' ' || p.state || ' ' || p.released_at || ' ' || t.job_id || ' ' || q.state || ' ' || t.granted_at_ms || ' ' || t.queued_at_ms FROM permits p JOIN traffic_entries t JOIN job_lifecycle l ON l.job_id=t.job_id JOIN permits q ON q.job_id=t.job_id WHERE p.job_id=:first AND l.observer_pid=:observer AND p.state='RELEASED' AND p.released_at IS NOT NULL AND q.state='ACTIVE' AND t.granted_at_ms >= p.released_at;\") || exit 125",
    "  [ -n \"$state\" ] || exit 125",
    "  printf '%s %s %s\\n' \"$$\" \"$MIXED_FIRST_TEST_PID\" \"$state\" > \"$MIXED_RELEASE_WITNESS\" || exit 125",
    "fi",
    "exec \"$MIXED_REAL_TOOL\" \"$@\"",
    "",
  ].join("\n");
  requireTraffic(found && found[1] === expected, "missing source-bound non-waiting exec witness");
  return found[1];
}

/** Recompute every acceptance value from the controller's observed events and retained markers. */
export function deriveCase(root: string, raw: any, window: Window) {
  requireTraffic(Number.isFinite(Date.parse(window?.started)) && Number.isFinite(Date.parse(window?.ended)) &&
    Date.parse(window.started) <= Date.parse(window.ended), "invalid measurement command window");
  requireTraffic(raw?.schemaVersion === 1 && tools.includes(raw.order) && ["bypass", "scheduled"].includes(raw.mode) &&
    raw.id === `${raw.order}_first-${raw.mode}` && Array.isArray(raw.jobs) && raw.jobs.length === 2 &&
    Array.isArray(raw.events), "invalid measurement case");
  processIdentity(raw.daemon?.identity, window);
  executable(raw.daemon?.executable);
  requireTraffic(raw.daemon.executable.path.endsWith(`${sep}arc-daemon`), "daemon executable differs");
  const ids: string[] = [], invocations: number[] = [], exits: number[] = [], outcomes: string[] = [];
  const seenPids = new Set<number>([raw.daemon.identity.pid]);
  for (let i = 0; i < 2; i++) {
    const job = raw.jobs[i], tool = tools[i], proc = job?.process;
    requireTraffic(job?.tool === tool && proc?.parentVerified === true, "tool or parent observation differs");
    ids.push(directory(job.cwd, root));
    for (const id of [proc.wrapper, proc.tool, proc.test]) processIdentity(id, window, raw.daemon.identity.boot_id);
    const jobPids = new Set<number>([proc.wrapper.pid, proc.tool.pid, proc.test.pid]);
    requireTraffic([...jobPids].every(pid => !seenPids.has(pid)) &&
      Number(raw.daemon.identity.start_token) <= Number(proc.wrapper.start_token), "daemon or job identities overlap");
    for (const pid of jobPids) seenPids.add(pid);
    requireTraffic(raw.mode === "bypass" ? equal(proc.wrapper, proc.tool) : proc.wrapper.pid !== proc.tool.pid,
      "CLI wrapper/tool relation differs");
    requireTraffic(raw.mode !== "scheduled" || proc.wrapper.pid !== proc.test.pid, "CLI wrapper is not independent");
    requireTraffic(tool === "cargo" ? proc.test.pid !== proc.tool.pid : equal(proc.test, proc.tool),
      "test body is not the expected native process");
    requireTraffic(Number(proc.wrapper.start_token) <= Number(proc.tool.start_token) &&
      Number(proc.tool.start_token) <= Number(proc.test.start_token), "native process start order differs");
    executable(proc.toolExecutable); executable(proc.testExecutable); executable(proc.wrapperExecutable);
    requireTraffic(raw.mode === "bypass" ? equal(proc.wrapperExecutable, proc.toolExecutable) :
      proc.wrapperExecutable.path.endsWith(`${sep}arcctl`), "CLI wrapper executable differs");
    requireTraffic(proc.toolExecutable.path.endsWith(`${sep}${tool}`) &&
      (tool !== "bun" || equal(proc.toolExecutable, proc.testExecutable)), "wrong tool executable");
    requireTraffic(tool === "cargo" ? equal(job.argv, ["test", "--offline", "--", "--nocapture"]) : equal(job.argv, ["test"]),
      "native test argv differs");
    for (const file of tool === "cargo" ? ["Cargo.toml", "src/lib.rs", "target/CACHEDIR.TAG"] : ["barrier.test.ts"]) {
      const path = resolve(job.cwd, file), st = lstatSync(path);
      requireTraffic(st.isFile() && !st.isSymbolicLink() && realpathSync(path) === path, "native project artifact missing");
    }
    const sourceName = tool === "cargo" ? "src/lib.rs" : "barrier.test.ts";
    requireTraffic(resolve(root, job.source?.path ?? "") === resolve(job.cwd, sourceName), "test source path differs");
    const source = bytes(root, job.source, window).toString();
    requireTraffic((tool === "cargo" ? source.includes("#[test]") && source.includes("real_cargo_barrier") :
      source.includes("bun:test") && source.includes("real bun barrier")) && source.includes("MIXED_BODY_LOG") && source.includes("MIXED_RELEASE") &&
      statSync(resolve(root, job.source.path)).mtimeMs <= statSync(resolve(root, job.bodies.path)).mtimeMs,
      "native test source or execution order differs");
    if (tool === "cargo") {
      requireTraffic(resolve(root, job.manifest?.path ?? "") === resolve(job.cwd, "Cargo.toml") &&
        bytes(root, job.manifest, window).toString().includes("[workspace]"), "Cargo test manifest differs");
    } else requireTraffic(job.manifest === null, "unexpected Bun manifest");
    invocations.push(marker(root, job.bodies, job.cwd, "bodies", proc.test.pid, window));
    marker(root, job.launches, job.cwd, "launches", proc.tool.pid, window);
    requireTraffic(Math.floor(statSync(resolve(root, job.launches.path)).mtimeMs) >= Math.floor(Number(proc.tool.start_token) / 1000) &&
      Math.floor(statSync(resolve(root, job.bodies.path)).mtimeMs) >= Math.floor(Number(proc.test.start_token) / 1000),
      "process marker precedes native start");
    requireTraffic(resolve(root, job.shim?.path ?? "") === resolve(dirname(job.cwd), "shims", tool) &&
      bytes(root, job.shim, window).toString() === measurementShim(root), "exec script differs from source");
    requireTraffic(resolve(root, job.stdout.path) === resolve(job.cwd, "stdout.log") &&
      resolve(root, job.stderr.path) === resolve(job.cwd, "stderr.log"), "test stream path differs");
    const output = bytes(root, job.stdout, window).toString() + "\n" + bytes(root, job.stderr, window).toString();
    requireTraffic(tool === "cargo" ? /test real_cargo_barrier \.\.\. ok/.test(output) && /test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;/.test(output) :
      /\(pass\) real bun barrier/.test(output) && /^\s*1 pass\s*$/m.test(output) && /^\s*0 fail\s*$/m.test(output) && !/^\s*[1-9]\d* (?:skip|todo)\s*$/m.test(output),
      "native test output is absent, failed or skipped");
    requireTraffic(job.exitCode === 0 && job.outcome === "EXITED:0", "native test did not complete successfully");
    if (raw.mode === "scheduled" && tool !== raw.order) requireTraffic(
      output.includes("STARTED\nCommand started."), "queued CLI did not report its start");
    exits.push(job.exitCode); outcomes.push(job.outcome);
    if (raw.mode === "scheduled") requireTraffic(jobId(job.record?.jobId) && job.record.state === "COMPLETED" &&
      job.record.exitCode === 0 && job.record.outcomeKind === "EXITED" && job.record.class === "TEST", "durable TEST outcome missing");
    else requireTraffic(job.record === null, "unexpected bypass record");
  }
  requireTraffic(ids[0] !== ids[1] && separated(raw.jobs[0].cwd, raw.jobs[1].cwd) &&
    raw.jobs[0].process.test.pid !== raw.jobs[1].process.test.pid, "test directories or processes are not independent");
  const events = raw.events;
  requireTraffic(events.every((e: any, i: number) => e?.seq === i && integer(e.atUs) &&
    (i === 0 || e.atUs >= events[i - 1].atUs) && tools.includes(e.tool)), "invalid observed event order");
  const one = (kind: string, tool: string) => {
    const matches = events.filter((e: any) => e.event === kind && e.tool === tool);
    requireTraffic(matches.length === 1, `missing or repeated ${kind} for ${tool}`);
    return matches[0].seq as number;
  };
  for (const tool of tools) requireTraffic(one("test-start", tool) < one("test-end", tool), "test ended before start");
  const first = raw.order, second = first === "cargo" ? "bun" : "cargo";
  const firstIndex = tools.indexOf(first), secondIndex = tools.indexOf(second);
  requireTraffic(one("test-start", first) < one("test-start", second), "arrival order differs");
  const end = raw.firstEndObservation;
  const secondEnd = raw.secondEndObservation;
  requireTraffic(equal(end?.test, raw.jobs[firstIndex].process.test) && inWindow(end?.observedStoppedAtUnixUs, window) &&
    end.observedStoppedAtUnixUs > Number(end.test.start_token) &&
    equal(secondEnd?.test, raw.jobs[secondIndex].process.test) && inWindow(secondEnd?.observedStoppedAtUnixUs, window) &&
    secondEnd.observedStoppedAtUnixUs > Number(secondEnd.test.start_token) &&
    raw.jobs.every((job: any, i: number) => inWindow(job.wrapperExitObservedAtUnixUs, window) &&
      job.wrapperExitObservedAtUnixUs >= (i === firstIndex ? end : secondEnd).observedStoppedAtUnixUs),
    "first process exit observation is missing or unordered");
  const summary = (peakConcurrentTests: number) => ({ peakConcurrentTests, invocations, exitCodes: exits, completionOutcomes: outcomes,
    independentDirectoriesVerified: true, rawCaseId: raw.id });
  if (raw.mode === "bypass") {
    requireTraffic(events.length === 5 && raw.notice === null && raw.queue === null && raw.permit === null && raw.committedRelease === null &&
      raw.sqliteExecutable === null &&
      raw.overlapObservation?.bothLive === true && equal(raw.overlapObservation.first, raw.jobs[firstIndex].process.test) &&
      equal(raw.overlapObservation.second, raw.jobs[secondIndex].process.test) &&
      inWindow(raw.overlapObservation.observedAtUnixUs, window) &&
      Math.max(Number(end.test.start_token), Number(secondEnd.test.start_token)) <= raw.overlapObservation.observedAtUnixUs &&
      raw.overlapObservation.observedAtUnixUs < Math.min(end.observedStoppedAtUnixUs, secondEnd.observedStoppedAtUnixUs) &&
      one("test-start", second) < one("barrier-release", first) && one("barrier-release", first) < one("test-end", first),
      "bypassed test processes did not overlap");
    return summary(2);
  }
  const queue = raw.queue, permit = raw.permit;
  requireTraffic(events.length === 9 && raw.overlapObservation === null &&
    queue?.state === "QUEUED" && queue.childAbsent === true &&
    queue.firstTestLive === true && queue.activePermits === 1 && queue.firstClass === "TEST" && queue.firstPermitState === "ACTIVE" &&
    jobId(queue.jobId) && queue.jobId === raw.jobs[secondIndex].record.jobId &&
    queue.firstJobId === raw.jobs[firstIndex].record.jobId && queue.firstJobId !== queue.jobId &&
    integer(queue.queuedAtMs) && integer(permit?.releasedAtMs) && integer(permit?.grantedAtMs) &&
    integer(permit?.queueWaitMs) && integer(permit?.secondLaunchAtMs) && inWindow(permit?.releaseObservedAtUnixUs, window) &&
    inWindow(permit?.secondBodyObservedAtUnixUs, window) &&
    permit.secondBodyObservedAtUnixUs >= Number(raw.jobs[secondIndex].process.test.start_token) &&
    permit.releaseState === "RELEASED" && permit.releaseJobId === queue.firstJobId &&
    permit.grantState === "ACTIVE" && permit.grantJobId === queue.jobId &&
    permit.grantedAtMs >= permit.releasedAtMs && permit.grantedAtMs >= queue.queuedAtMs &&
    permit.queueWaitMs === permit.grantedAtMs - queue.queuedAtMs && permit.secondLaunchAtMs >= permit.releasedAtMs &&
    permit.releaseObservedAtUnixUs >= permit.releasedAtMs * 1000 &&
    permit.secondLaunchAtMs === Math.floor(statSync(resolve(root, raw.jobs[secondIndex].launches.path)).mtimeMs),
    "durable queue, permit or wait differs");
  // On macOS start_token is proc_pidinfo's Unix start time in microseconds.
  // releasedAtMs uses the same wall-clock epoch but is truncated to milliseconds.
  // A start in that millisecond is ambiguous, even if the later shim sees the
  // committed RELEASED row. Controller observation time is not a launch time.
  requireTraffic(Math.floor(Number(raw.jobs[secondIndex].process.tool.start_token) / 1000) > permit.releasedAtMs,
    "kernel start is not unambiguously after release");
  executable(raw.sqliteExecutable);
  requireTraffic(raw.sqliteExecutable.path === realpathSync("/usr/bin/sqlite3"), "wrong SQLite witness executable");
  const witnessMs = statSync(resolve(root, raw.committedRelease?.path ?? "")).mtimeMs;
  requireTraffic(resolve(root, raw.committedRelease?.path ?? "") === resolve(raw.jobs[secondIndex].cwd, "committed-release") &&
    bytes(root, raw.committedRelease, window).toString() === `${raw.jobs[secondIndex].process.tool.pid} ${end.test.pid} ${queue.firstJobId} RELEASED ${permit.releasedAtMs} ${queue.jobId} ACTIVE ${permit.grantedAtMs} ${queue.queuedAtMs}\n` &&
    Math.floor(witnessMs) >= permit.releasedAtMs && Math.floor(witnessMs) >= permit.secondLaunchAtMs &&
    Math.floor(witnessMs) <= Math.floor(statSync(resolve(root, raw.jobs[secondIndex].bodies.path)).mtimeMs),
    "single pre-exec committed permit witness differs");
  requireTraffic(resolve(root, raw.notice?.path ?? "") === resolve(raw.jobs[secondIndex].cwd, "queued.log") &&
    bytes(root, raw.notice, window).toString().includes("QUEUED\nWaiting to start."), "visible queue notice missing");
  const sequence = [one("test-start", first), one("queued", second), one("notice", second),
    one("barrier-release", first), one("test-end", first), one("permit-release", first),
    one("grant", second), one("test-start", second), one("test-end", second)];
  requireTraffic(sequence.every((v, i) => v === i), "queue or committed release order differs");
  return { ...summary(1), queuedVisible: true, startedAfterRelease: true, queueWaitMs: permit.queueWaitMs };
}

export function casesFromOutput(stdout: string) {
  const rows = stdout.split(/\r?\n/).filter(line => line.includes("TRAFFIC_BUN_CASE ")).map(line =>
    JSON.parse(line.slice(line.indexOf("TRAFFIC_BUN_CASE ") + "TRAFFIC_BUN_CASE ".length)));
  requireTraffic(rows.length === 4 && new Set(rows.map((v: any) => v.id)).size === 4, "four distinct native cases required");
  return rows;
}

export function deriveOrders(root: string, cases: any[], window: Window) {
  requireTraffic(Array.isArray(cases) && cases.length === 4, "four native cases required");
  const orders: any = {};
  for (const order of tools) {
    orders[`${order}_first`] = {};
    for (const mode of ["bypass", "scheduled"]) {
      const found = cases.filter(raw => raw?.id === `${order}_first-${mode}`);
      requireTraffic(found.length === 1, "missing or duplicate native scenario");
      orders[`${order}_first`][mode] = deriveCase(root, found[0], window);
      requireTraffic(orders[`${order}_first`][mode].peakConcurrentTests === (mode === "bypass" ? 2 : 1), "incorrect peak");
    }
  }
  return orders;
}
