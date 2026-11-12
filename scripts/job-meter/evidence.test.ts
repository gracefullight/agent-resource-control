import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dir, "../..");
const evidence = resolve(root, ".arc-verify/job-meter");
const native = JSON.parse(readFileSync(resolve(evidence, "native.json"), "utf8"));
const sha = (bytes: Buffer | string) => createHash("sha256").update(bytes).digest("hex");
const close = (a: number, b: number) => expect(Math.abs(a - b)).toBeLessThanOrEqual(1e-9);
const rawSeconds = (v: { seconds: number; microseconds: number }) => {
  expect(Number.isInteger(v.seconds) && v.seconds >= 0).toBe(true);
  expect(Number.isInteger(v.microseconds) && v.microseconds >= 0 && v.microseconds < 1e6).toBe(true);
  return v.seconds + v.microseconds / 1e6;
};

test("native calibration arithmetic, identities and explicit waited scope", () => {
  expect(native.schemaVersion).toBe(1);
  expect(native.baseline.acceptedHead).toBe("18091eeb1ac0232dfdda4a937113fa3bd149b220");
  const c = native.calibration;
  const r = c.reference;
  const actual = c.surfaces.sqliteReport;
  const user = rawSeconds(r.raw.self_user) + rawSeconds(r.raw.children_user);
  const system = rawSeconds(r.raw.self_system) + rawSeconds(r.raw.children_system);
  expect(r.user_microseconds).toBe(r.raw.self_user.seconds * 1e6 + r.raw.self_user.microseconds + r.raw.children_user.seconds * 1e6 + r.raw.children_user.microseconds);
  expect(r.system_microseconds).toBe(r.raw.self_system.seconds * 1e6 + r.raw.self_system.microseconds + r.raw.children_system.seconds * 1e6 + r.raw.children_system.microseconds);
  close(user + system, r.total_core_seconds);
  close(user, c.referenceUserCoreSeconds);
  close(system, c.referenceSystemCoreSeconds);
  close(actual.cpu_core_seconds.user, c.recordedUserCoreSeconds);
  close(actual.cpu_core_seconds.system, c.recordedSystemCoreSeconds);
  close(actual.cpu_core_seconds.user + actual.cpu_core_seconds.system, c.recordedTotalCoreSeconds);
  const error = Math.abs(c.recordedTotalCoreSeconds - user - system);
  const allowed = Math.max(0.1, 0.05 * (user + system));
  close(error, c.absoluteErrorCoreSeconds);
  close(allowed, c.allowedErrorCoreSeconds);
  expect(error).toBeLessThanOrEqual(allowed);
  expect(c.withinTolerance && c.scopeMatched && c.shortLivedDescendantsReaped).toBe(true);
  expect(c.workerCount).toBe(2);
  expect(r.worker_count).toBe(2);
  expect(r.requested_seconds).toBe(10);
  expect(c.elapsedSeconds).toBeGreaterThanOrEqual(9.5);
  expect(c.elapsedSeconds).toBeLessThan(15);
  expect(r.elapsed_seconds).toBeGreaterThanOrEqual(9.5);
  expect(r.elapsed_seconds).toBeLessThan(15);
  expect(actual.child.pid).toBe(r.root.pid);
  expect(actual.child.start_token).toBe(r.root.start_token);
  expect(actual.child.boot_id).toBe(r.boot_id.toLowerCase());
  expect(actual.scope).toBe("native_waited_descendants");
  expect(r.scope).toBe(actual.scope);
  expect(actual.completeness).toBe("partial");
  expect(actual.missing_coverage).toContain("unobserved_descendants");
  const pids = new Set([r.root.pid]);
  for (const w of r.workers) {
    expect(w.short_lived_reaped).toBe(4);
    expect(w.short_lived.length).toBe(4);
    for (const child of [w.identity, ...w.short_lived]) {
      expect(pids.has(child.pid)).toBe(false);
      expect(BigInt(child.start_token)).toBeGreaterThan(0n);
      pids.add(child.pid);
    }
  }
  expect(pids.size).toBe(11);
});

test("required cases have current executable receipts and measured observations", () => {
  const groups = [
    [native.matrix, ["scheduled", "lifecycle", "queue_wait", "exit_0", "exit_17", "signals", "stop_continue",
      "nested_scheduled", "nested_lifecycle", "unrelated_pipeline", "short_lived_descendants", "escaped_descendants",
      "interrupted", "duplicate_reports", "conflicting_reports", "identity_mismatch", "daemon_restart", "lost_report_ack", "lost_completion_ack"]],
    [native.compatibility, ["old_protocol_1", "unavailable_reporting", "stalled_reporting", "bounded_fail_open", "single_execution", "snapshot_preservation", "redaction", "legacy_unknown"]],
  ] as const;
  for (const [group, names] of groups) {
    expect(group.allRequiredCasesPassed).toBe(true);
    expect(new Set(group.cases.map((c: { id: string }) => c.id)).size).toBe(group.cases.length);
    for (const id of names) {
      const c = group.cases.find((v: { id: string }) => v.id === id);
      expect(c?.passed).toBe(true);
      expect(c.syntheticOnly).toBe(false);
      expect(c.test.length > 8 && c.inputs && c.observed).toBeTruthy();
      const receipt = native.checks.find((r: { id: string }) => r.id === c.receiptId);
      expect(receipt.exitCode).toBe(0);
      expect(receipt.argv[0]).toBe("cargo");
      expect(receipt.stdout).toContain(`test ${c.test} ... ok`);
      expect(receipt.inputDigestBefore).toBe(native.inputBinding.digest);
      expect(receipt.inputDigestAfter).toBe(native.inputBinding.digest);
    }
  }
});

test("raw read-only SQLite, CLI and registered production desktop agree", () => {
  expect(native.persistence.sqliteCliDesktopAgree && native.persistence.retention90DaysVerified).toBe(true);
  for (const s of [native.calibration.surfaces, native.calibration.afterRestart]) {
    expect(s.cli).toEqual(s.desktop.data);
    expect(s.sqliteReport).toEqual(s.cli.accounting.report);
    expect(JSON.parse(s.sqliteReadOnly.report_json)).toEqual(s.sqliteReport);
    expect(s.sqliteReport.job_id).toBe(s.jobId);
    expect(s.cli.job_id).toBe(s.jobId);
    expect(s.desktopCommand).toContain("production registration; real authenticated daemon transport");
  }
  const r = native.persistence.retention;
  expect(r.controlledClock.retentionMs).toBe(90 * 24 * 60 * 60 * 1000);
  expect(r.controlledClock.boundaryMs - r.controlledClock.terminalAtMs).toBe(r.controlledClock.retentionMs);
  expect(r.before.accounting.state).toBe("recorded");
  expect(r.after.accounting.state).toBe("expired");
  expect(r.remainingPayloads).toBe(0);
  expect(r.remainingJobRows).toBe(1);
  expect(r.lateDeliveryRejected && r.backwardClockCannotResurrect).toBe(true);
});

test("final input digests, immutable migrations and evidence limits", () => {
  const input = native.inputBinding;
  expect(input.files.length).toBeGreaterThan(100);
  for (const f of input.files) {
    const bytes = readFileSync(resolve(root, f.path));
    expect(bytes.length).toBe(f.bytes);
    expect(sha(bytes)).toBe(f.sha256);
  }
  expect(sha(input.files.map((f: { path: string; sha256: string }) => `${f.path}\0${f.sha256}\n`).join(""))).toBe(input.digest);
  for (const f of native.migrationDigests) expect(sha(readFileSync(resolve(root, f.path)))).toBe(f.sha256);
  for (const f of native.integrity.newSourceAndTests) {
    const text = readFileSync(resolve(root, f), "utf8");
    expect(text.split("\n").length - Number(text.endsWith("\n"))).toBeLessThan(500);
  }
  expect(readFileSync(import.meta.path, "utf8").split("\n").length).toBeLessThan(200);
  const allowed = ["contract.md", "experience.md", "value.md", "native.json", "gates.json", "qa.json", "final.md"];
  const files = readdirSync(evidence);
  expect(files.length).toBeLessThanOrEqual(7);
  let bytes = 0;
  for (const file of files) {
    expect(allowed).toContain(file);
    const info = statSync(resolve(evidence, file));
    expect(info.isFile()).toBe(true);
    bytes += info.size;
  }
  expect(bytes).toBeLessThanOrEqual(5_000_000);
});
