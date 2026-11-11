import { expect, test } from "bun:test";
import { readFileSync, symlinkSync } from "node:fs";
import { resolve } from "node:path";
import { gitBytes } from "../coverage/hygiene";
import { hash, walk } from "../../coverage/source";
import { coverageFixture } from "./managed-exec-coverage-fixture";
import { directory, validateFinalEvidence } from "./managed-exec-evidence";

function snapshot(f: ReturnType<typeof coverageFixture>) {
  return { files: Object.fromEntries(walk(resolve(f.dir, directory)).map(p => [p, hash(readFileSync(p))])),
    index: gitBytes(f.dir, ["ls-files", "--stage", "-z", "--", directory]).toString() };
}
test("reviewed measurements remain bound when latest advances to a different passing collection", () => {
  const f = coverageFixture(), before = snapshot(f);
  expect(f.load(`${f.reviewed.directory}/summary.json`).crates[0].covered).toBe(8);
  expect(f.load(`${f.later.directory}/summary.json`).crates[0].covered).toBe(9);
  expect(validateFinalEvidence(f.dir).pass).toBe(true);
  f.advance();
  expect(validateFinalEvidence(f.dir).pass).toBe(true);
  expect(snapshot(f)).toEqual(before);
});

test("both collection bindings reject accounting, bytes, inventories, source, identity and profile tampering", () => {
  const f = coverageFixture(); f.advance();
  const rejectJSON = (path: string, change: (value: any) => void, rebind = false) => {
    const original = readFileSync(resolve(f.dir, path)), summaryPath = `${directory}/final-summary.json`;
    const oldSummary = readFileSync(resolve(f.dir, summaryPath));
    const value = f.load(path); change(value); f.save(path, value);
    if (rebind) { const summary = f.load(summaryPath); summary.coverage.summary = f.ref(path); f.save(summaryPath, summary); f.stage(); }
    try { expect(() => validateFinalEvidence(f.dir)).toThrow(); }
    finally { f.put(path, original); f.put(summaryPath, oldSummary); f.stage(); }
  };
  for (const pointer of [f.reviewed, f.later]) {
    const base = pointer.directory;
    for (const change of [
      (r: any) => r.crates[0].covered++, (r: any) => r.files[0].covered++,
      (r: any) => r.files.pop(), (r: any) => r.crates.pop(), (r: any) => r.runId = "other-unit",
      (r: any) => r.sourceHash = "0".repeat(64), (r: any) => r.host = "x86_64-unknown-linux",
      (r: any) => r.inputs.files = {}, (r: any) => r.pass = false,
    ]) rejectJSON(`${base}/summary.json`, change, pointer === f.reviewed);
    rejectJSON(`${base}/receipt.json`, r => r.runId = "other-unit");
    rejectJSON(`${base}/receipt.json`, r => r.exit = 1);
    rejectJSON(`${base}/inputs.json`, r => r.sha256 = "0".repeat(64));
    rejectJSON(`${base}/source-inventory.json`, r => r.pop());
    for (const change of [
      (p: any) => p.merge.pop(), (p: any) => p.childProof[0].pid++,
      (p: any) => p.childProof.pop(), (p: any) => p.profiles[0].path = "../escape.profraw",
      (p: any) => p.profiles.push(p.profiles[0]), (p: any) => p.target = resolve(f.dir, "target/elsewhere"),
    ]) rejectJSON(`${base}/profiles.json`, change);
    const profiles = f.load(`${base}/profiles.json`);
    for (const path of [`${base}/raw.lcov`, `${base}/raw.json`, `target/arc-coverage/${pointer.runId}/${profiles.profiles[0].path}`]) {
      const bytes = readFileSync(resolve(f.dir, path)); f.put(path, Buffer.concat([bytes, Buffer.from("\n")]));
      try { expect(() => validateFinalEvidence(f.dir)).toThrow(); } finally { f.put(path, bytes); }
    }
  }
});

test("another valid passing collection cannot replace the independently reviewed run", () => {
  const f = coverageFixture(); f.advance();
  const path = `${directory}/final-summary.json`, summary = f.load(path), report = f.load(`${f.later.directory}/summary.json`);
  const profiles = f.load(`${f.later.directory}/profiles.json`);
  summary.coverage = { ...summary.coverage, runId: f.later.runId, summary: f.ref(`${f.later.directory}/summary.json`),
    crates: report.crates, childProof: profiles.childProof, profilesChecked: profiles.profiles.length };
  f.save(path, summary); f.stage();
  expect(() => validateFinalEvidence(f.dir)).toThrow("coverage not bound to independent review");
});

test("unsafe summary and pointer paths, changed recorded fields and below-floor latest are rejected", () => {
  const f = coverageFixture(), path = `${directory}/final-summary.json`, original = readFileSync(resolve(f.dir, path));
  for (const change of [
    (s: any) => s.coverage.summary.sha256 = "0".repeat(64),
    (s: any) => s.coverage.summary.path = `${f.reviewed.directory}/../reviewed-unit/summary.json`,
    (s: any) => s.coverage.summary.path = resolve(f.dir, f.reviewed.directory, "summary.json"),
    (s: any) => s.coverage.summary.path = `${f.later.directory}/summary.json`,
    (s: any) => s.coverage.runId = "later-unit", (s: any) => s.coverage.sourceHash = "0".repeat(64),
    (s: any) => s.coverage.crates[0].covered++, (s: any) => s.coverage.profilesChecked++,
    (s: any) => s.coverage.childProof[0].pid++,
  ]) {
    const summary = f.load(path); change(summary); f.save(path, summary); f.stage();
    try { expect(() => validateFinalEvidence(f.dir)).toThrow(); } finally { f.put(path, original); f.stage(); }
  }
  for (const change of [
    (p: any) => p.directory += "/../later-unit", (p: any) => p.runId = "other-unit",
    (p: any) => p.sourceHash = "0".repeat(64), (p: any) => p.pass = false,
  ]) { const pointer = { ...f.later }; change(pointer); f.save(f.latestPath, pointer); expect(() => validateFinalEvidence(f.dir)).toThrow(); }
  f.advance();
  const target = resolve(f.dir, ".arc-verify/rust-coverage/baseline/runs/linked-unit");
  symlinkSync(resolve(f.dir, f.later.directory), target);
  f.save(f.latestPath, { ...f.later, runId: "linked-unit", directory: ".arc-verify/rust-coverage/baseline/runs/linked-unit" });
  expect(() => validateFinalEvidence(f.dir)).toThrow();
  f.advance();
  f.collection(f.later.runId, 7);
  expect(f.load(`${f.later.directory}/summary.json`).crates[0]).toMatchObject({ covered: 7, total: 10, pass: false });
  expect(() => validateFinalEvidence(f.dir)).toThrow();
});
