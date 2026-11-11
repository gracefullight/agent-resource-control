import { test, expect } from "bun:test";
import { mkdirSync, mkdtempSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { hash } from "./source";
import { validateQAIndependentHandoff } from "./traffic-bun-qa";

test("independent QA authenticates its manifest, receipt, output and preserved files", () => {
  const base = resolve(import.meta.dir, "../../target/traffic-bun");
  mkdirSync(base, {recursive: true});
  const root = mkdtempSync(resolve(base, "qa-fixture-")), dir = "target/traffic-bun/qa";
  mkdirSync(resolve(root, dir), {recursive: true});
  const write = (name: string, content: string) => {
    const path = `${dir}/${name}`;
    writeFileSync(resolve(root, path), content);
    return {path, sha256: hash(Buffer.from(content))};
  };
  const stdout = write("stdout.log", "passed"), stderr = write("stderr.log", "");
  const receipt = write("receipt.json", JSON.stringify({output: {stdout, stderr}}));
  const files = [receipt, stdout, stderr].map((original, i) => ({original,
    preserved: write(`copy-${i}`, readFileSync(resolve(root, original.path)).toString())}));
  const sourceHash = "a".repeat(64);
  const manifest = {schemaVersion: 1, actorId: "independent-reviewer", sourceHash, policyReceipt: receipt, files};
  const manifestRef = () => write("manifest.json", JSON.stringify(manifest));
  const qa = {sourceHash, handoff: {installed: true, preservedManifest: manifestRef(), helper: write("handoff.ts", "helper")},
    independentRuns: {freshSnapshotPolicy: {passed: 1, failed: 0, ignored: 0, receipt}}};
  const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
  expect(() => validateQAIndependentHandoff(root, qa)).not.toThrow();
  for (const mutate of [
    (v: any) => v.handoff.installed = false,
    (v: any) => delete v.independentRuns.freshSnapshotPolicy.receipt,
    (v: any) => v.independentRuns.freshSnapshotPolicy.receipt.sha256 = "0".repeat(64),
    (v: any) => delete v.handoff.preservedManifest,
    (v: any) => v.handoff.helper.sha256 = "0".repeat(64),
    (v: any) => v.independentRuns.freshSnapshotPolicy.ignored = 1,
  ]) {
    const bad = clone(qa); mutate(bad);
    expect(() => validateQAIndependentHandoff(root, bad)).toThrow();
  }
  const checkManifest = (bad: any) => {
    const changed = clone(qa);
    changed.handoff.preservedManifest = write("manifest.json", JSON.stringify(bad));
    expect(() => validateQAIndependentHandoff(root, changed)).toThrow();
  };
  checkManifest({});
  checkManifest({...manifest, files: []});
  checkManifest({...manifest, sourceHash: "b".repeat(64)});
  checkManifest({...manifest, policyReceipt: stdout});
  checkManifest({...manifest, files: files.slice(0, 2)});
  checkManifest({...manifest, files: [...files, files[0]]});
  const malformed = clone(qa); malformed.handoff.preservedManifest = write("manifest.json", "{");
  expect(() => validateQAIndependentHandoff(root, malformed)).toThrow();
  qa.handoff.preservedManifest = manifestRef();
  writeFileSync(resolve(root, files[0].preserved.path), "altered");
  expect(() => validateQAIndependentHandoff(root, qa)).toThrow();
  writeFileSync(resolve(root, files[0].preserved.path), readFileSync(resolve(root, receipt.path)));
  unlinkSync(resolve(root, files[1].preserved.path));
  expect(() => validateQAIndependentHandoff(root, qa)).toThrow();
  writeFileSync(resolve(root, files[1].preserved.path), readFileSync(resolve(root, stdout.path)));
  unlinkSync(resolve(root, qa.handoff.helper.path));
  expect(() => validateQAIndependentHandoff(root, qa)).toThrow();
});
