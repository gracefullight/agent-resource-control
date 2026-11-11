import { readFileSync, lstatSync, realpathSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { arch } from "node:os";
import { hash, inputIdentity, sourceInventory } from "../../coverage/source";
import { account } from "../../coverage/accounting";
import { BASELINE } from "../../coverage/config";
import { equal, requireValue, canonicalBytes } from "./exec-fail-open-inputs";
const coverageBase = ".arc-verify/rust-coverage/baseline";
export function nativeCollection(root: string, runId: unknown) {
  requireValue(typeof runId === "string" && /^[A-Za-z0-9][A-Za-z0-9-]*$/.test(runId), "unsafe native run identity");
  const base = `${coverageBase}/runs/${runId}`;
  const bytes = (name: string) => canonicalBytes(root, `${base}/${name}`);
  const load = (name: string) => JSON.parse(bytes(name).toString());
  const identity = inputIdentity(root), inventory = sourceInventory(root);
  const report = load("summary.json"), receipt = load("receipt.json");
  requireValue(report.schemaVersion === 1 && report.threshold === 80 && report.pass === true &&
    report.acceptedBaseline === BASELINE && receipt.acceptedBaseline === BASELINE &&
    report.sourceHash === identity.sha256 && receipt.sourceHash === identity.sha256 &&
    report.runId === runId && receipt.runId === runId && report.host === `${arch() === "arm64" ? "aarch64" : arch() === "x64" ? "x86_64" : arch()}-apple-darwin` &&
    receipt.schemaVersion === 1 && receipt.pass === true && receipt.exit === 0, "native coverage receipt/source mismatch");
  requireValue(equal(report.inputs, identity) && equal(load("inputs.json"), identity) &&
    equal(load("source-inventory.json"), inventory), "native coverage inputs/inventory stale");
  const raw = bytes("raw.lcov"), llvm = bytes("raw.json");
  requireValue(hash(raw) === report.rawSha256 && hash(llvm) === report.llvmSha256, "native coverage report digest mismatch");
  const recomputed = account(raw.toString(), inventory, root, undefined, JSON.parse(llvm.toString()));
  requireValue(recomputed.pass && equal(recomputed.crates, report.crates) && equal(recomputed.files, report.files), "native crate/file accounting differs");
  const profiles = load("profiles.json"), target = resolve(root, "target/arc-coverage", runId);
  requireValue(profiles.target === target && realpathSync(target) === resolve(realpathSync(root), "target/arc-coverage", runId), "unsafe native profile target");
  requireValue(Array.isArray(profiles.profiles) && profiles.profiles.length > 0 &&
    profiles.profiles.every((p: any) => typeof p.path === "string" && /^arc-[0-9]+-[A-Za-z0-9_]+\.profraw$/.test(p.path)) &&
    new Set(profiles.profiles.map((p: any) => p.path)).size === profiles.profiles.length &&
    equal(readdirSync(target).filter(p => p.endsWith(".profraw")).sort(), profiles.profiles.map((p: any) => p.path).sort()) &&
    Array.isArray(profiles.merge) && equal([...profiles.merge].sort(), profiles.profiles.map((p: any) => resolve(target, p.path)).sort()), "native profile merge mismatch");
  for (const p of profiles.profiles) {
    const profile = canonicalBytes(root, `target/arc-coverage/${runId}/${p.path}`);
    requireValue(profile.length > 0 && profile.length === p.size && hash(profile) === p.sha256, "native profile digest mismatch");
  }
  requireValue(Array.isArray(profiles.childProof) && equal(profiles.childProof.map((c: any) => c.binary), ["arcctl", "arc-daemon"]) &&
    equal(report.childProof, profiles.childProof) && Array.isArray(receipt.receipts), "missing native child profiles");
  for (const child of profiles.childProof) requireValue(Number.isSafeInteger(child.pid) && child.pid > 0 && Array.isArray(child.profiles) &&
    child.profiles.length > 0 && child.profiles.every((p: string) => p.startsWith(`arc-${child.pid}-`) &&
      profiles.merge.includes(resolve(target, p))) && receipt.receipts.some((r: any) => r.pid === child.pid && r.exit === child.exit &&
      r.argv?.[0] === resolve(target, "debug", child.binary)), "native child PID proof mismatch");
  bytes("uncovered.md");
  return { report, profiles };
}
