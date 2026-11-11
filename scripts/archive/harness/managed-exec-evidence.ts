import { existsSync, lstatSync, readFileSync, realpathSync, readdirSync } from "node:fs";
import { resolve, relative } from "node:path";
import { platform, release, arch, hostname } from "node:os";
import { hash, walk, inputIdentity, sourceInventory } from "../../coverage/source";
import { gitBytes, indexEntries, prohibitedReasons } from "../coverage/hygiene";
import { account } from "../../coverage/accounting";
import { BASELINE } from "../../coverage/config";
import { FEATURE_BASELINE } from "../coverage/current-preservation";
import { validateCargoExecution } from "./managed-exec-test-output";

export const directory = ".arc-verify/managed-exec";
export const requiredCommands = [
  ["cargo", "fmt", "--all", "--", "--check"],
  ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
  ["cargo", "test", "--workspace"], ["bun", "run", "test:coverage"], ["./scripts/harness/validate.sh"],
  ...["scripts/harness/managed-exec.test.ts", "scripts/harness/equivalence.test.ts",
    "scripts/harness/coverage-verification.test.ts", "scripts/coverage/clean-checkout.test.ts",
    "scripts/coverage/coverage-cli.test.ts", "scripts/coverage/harness.test.ts",
    "scripts/coverage/evidence.test.ts"].map(path => ["bun", "test", path]),
];
export const nativeHost = () => ({ platform: platform(), release: release(), arch: arch(), hostname: hostname() });
const equal = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
function requireValue(condition: unknown, diagnostic: string): asserts condition {
  if (!condition) throw Error(`MANAGED_EXEC_EVIDENCE: ${diagnostic}`);
}
const digest = (s: unknown) => typeof s === "string" && /^[a-f0-9]{64}$/.test(s);
export function managedInputs(root: string) {
  const paths = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "mise.toml", "package.json", ".gitignore",
    ".arc-verify/rust-coverage/contract/accounting.json", `${directory}/contract.json`, `${directory}/decision.md`,
    "docs/rust-coverage.md", ...[".cargo/config", ".cargo/config.toml", "build.rs"].filter(p => existsSync(resolve(root, p))),
    ...["crates", "scripts/coverage", "scripts/harness"].flatMap(p => walk(resolve(root, p)).map(f => relative(root, f)))];
  const files = Object.fromEntries([...new Set(paths)].sort().map(path => {
    const stat = lstatSync(resolve(root, path));
    requireValue(stat.isFile() && !stat.isSymbolicLink(), `nonregular input: ${path}`);
    return [path, { sha256: hash(readFileSync(resolve(root, path))), mode: stat.mode & 0o777 }];
  }));
  return { sha256: hash(JSON.stringify(files)), files };
}
export function managedGit(root: string) {
  const scopes = ["crates", "scripts/coverage", "scripts/harness", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
    "mise.toml", "package.json", ".gitignore", ".cargo", "build.rs", "docs/rust-coverage.md"];
  return { head: gitBytes(root, ["rev-parse", "HEAD"]).toString().trim(),
    // Against fixed feature baseline, independent of staging, so staging final
    // evidence does not invalidate already captured product command receipts.
    trackedDiffSha256: hash(gitBytes(root, ["diff", "--binary", FEATURE_BASELINE, "--", ...scopes])),
    untrackedSource: gitBytes(root, ["ls-files", "--others", "--exclude-standard", "-z", "--", ...scopes])
      .toString().split("\0").filter(Boolean).sort() };
}
function evidenceBytes(root: string, path: string) {
  requireValue(typeof path === "string" && path.startsWith(directory + "/") && !path.split("/").some(p => p === ".." || p === "."), "unsafe evidence path");
  const file = resolve(root, path), stat = lstatSync(file);
  requireValue(stat.isFile() && !stat.isSymbolicLink(), `nonregular artifact: ${path}`);
  requireValue(realpathSync(file) === resolve(realpathSync(root), path), "artifact path traverses symlink");
  const bytes = readFileSync(file);
  requireValue(prohibitedReasons(path, bytes).length === 0, `prohibited artifact: ${path}`);
  return bytes;
}
function boundJSON(root: string, ref: any) {
  requireValue(ref && digest(ref.sha256), "missing artifact digest");
  const bytes = evidenceBytes(root, ref.path);
  requireValue(hash(bytes) === ref.sha256, `artifact digest mismatch: ${ref.path}`);
  return JSON.parse(bytes.toString());
}
const coverageBase = ".arc-verify/rust-coverage/baseline";
function canonicalBytes(root: string, path: string) {
  requireValue(typeof path === "string" && !path.includes("\\") && !path.split("/").some(p => !p || p === ".." || p === "."), "unsafe native artifact path");
  const file = resolve(root, path);
  requireValue(lstatSync(file).isFile() && !lstatSync(file).isSymbolicLink() &&
    realpathSync(file) === resolve(realpathSync(root), path), `noncanonical native artifact: ${path}`);
  return readFileSync(file);
}
function nativeCollection(root: string, runId: unknown) {
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
export function validateCommandReceipt(receipt: any, sourceHash: string, host: ReturnType<typeof nativeHost>, actorId = "backend") {
  requireValue(receipt.schemaVersion === 1 && receipt.sourceHash === sourceHash, "stale command source");
  requireValue(equal(receipt.host, host) && host.platform === "darwin", "command must bind native macOS host");
  requireValue(Array.isArray(receipt.argv) && receipt.argv.length && receipt.argv.every((s: unknown) => typeof s === "string"), "missing exact argv");
  requireValue(receipt.cwd === ".", "receipt cwd must be workspace-relative dot");
  requireValue(Number.isInteger(receipt.exit) && ["passed", "failed"].includes(receipt.status) &&
    (receipt.status === "passed") === (receipt.exit === 0), "inconsistent command outcome");
  requireValue(Number.isFinite(Date.parse(receipt.started)) && Number.isFinite(Date.parse(receipt.ended)) &&
    Date.parse(receipt.ended) >= Date.parse(receipt.started), "invalid command times");
  requireValue(digest(receipt.stdoutSha256) && digest(receipt.stderrSha256), "missing output digests");
  requireValue(receipt.timeoutAllowanceMs >= 3600000, "command allowance below framework per-check timeout");
  requireValue(receipt.environment === undefined && receipt.stdout === undefined && receipt.stderr === undefined,
    "compact receipts must omit raw environment/output");
  requireValue(receipt.skipFlags === false && receipt.actorId === actorId, "invalid implementation receipt owner or skips");
}
export function validateFinalEvidence(root: string) {
  const summary = JSON.parse(evidenceBytes(root, `${directory}/final-summary.json`).toString());
  const source = managedInputs(root), host = nativeHost();
  requireValue(summary.schemaVersion === 1 && summary.baseline === FEATURE_BASELINE, "wrong summary contract");
  requireValue(summary.nativeMacos === true && summary.sourceBound === true && summary.independentReviewPassed === true,
    "final acceptance flags absent");
  requireValue(host.platform === "darwin" && equal(summary.host, host), "final acceptance requires current native macOS host");
  validateSourceBinding(summary, source, managedGit(root));
  requireValue(Array.isArray(summary.receipts) && summary.receipts.length > 0, "command receipts absent");
  const receipts = summary.receipts.map((ref: any) => {
    const receipt = boundJSON(root, ref); validateCommandReceipt(receipt, source.sha256, host); return receipt;
  });
  for (const argv of requiredCommands) requireValue(receipts.some((r: any) => r.exit === 0 && equal(r.argv, argv)), `missing successful command: ${argv.join(" ")}`);
  const coverage = receipts.findLast((r: any) => equal(r.argv, ["bun", "test", "scripts/coverage/evidence.test.ts"]) && r.exit === 0);
  const collectors = receipts.filter((r: any) => equal(r.argv, ["bun", "run", "test:coverage"]) || equal(r.argv, ["./scripts/harness/validate.sh"]) ||
    ["scripts/coverage/clean-checkout.test.ts", "scripts/coverage/coverage-cli.test.ts", "scripts/coverage/harness.test.ts"].some(p => equal(r.argv, ["bun", "test", p])));
  requireValue(collectors.every((r: any) => Date.parse(r.ended) <= Date.parse(coverage.started)), "live coverage check predates final collection");
  requireValue(summary.coverage && Array.isArray(summary.coverage.crates), "coverage accounting absent");
  // Independently read current workspace member manifests rather than accepting
  // an arbitrary list supplied by summary; the native evidence gate checks maps.
  const workspace = readFileSync(resolve(root, "Cargo.toml"), "utf8");
  const members = [...workspace.matchAll(/"(crates\/[^"*]+)"/g)].map(m => m[1]);
  const crates = [...new Set(members)].map(path => readFileSync(resolve(root, path, "Cargo.toml"), "utf8").match(/^name\s*=\s*"([^"]+)"/m)?.[1]).sort();
  requireValue(crates.length >= 6 && equal(summary.coverage.crates.map((c: any) => c.crate).sort(), crates), "missing workspace crate accounting");
  for (const c of summary.coverage.crates) requireValue(Number.isSafeInteger(c.total) && c.total > 0 && Number.isSafeInteger(c.covered) && c.covered >= 0 && c.covered <= c.total &&
    BigInt(c.covered) * 100n >= BigInt(c.total) * 80n, `crate below 80%: ${c.crate}`);
  const recorded = summary.coverage;
  requireValue(recorded.summary?.path === `${coverageBase}/runs/${recorded.runId}/summary.json` && digest(recorded.summary.sha256), "unsafe recorded coverage summary binding");
  requireValue(hash(canonicalBytes(root, recorded.summary.path)) === recorded.summary.sha256, "recorded coverage summary digest mismatch");
  const { report, profiles } = nativeCollection(root, recorded.runId);
  requireValue(recorded.sourceHash === report.sourceHash && equal(recorded.crates, report.crates) &&
    recorded.profilesChecked === profiles.profiles.length && equal(recorded.childProof, profiles.childProof) &&
    recorded.independentlyRecomputed === true, "recorded native coverage measurements differ");
  // The saved latest digest records what was observed at acceptance. Collectors
  // may advance that pointer without rewriting the reviewed measurements.
  requireValue(recorded.latest?.path === `${coverageBase}/latest.json` && digest(recorded.latest.sha256), "missing historical latest observation");
  const latest = JSON.parse(canonicalBytes(root, `${coverageBase}/latest.json`).toString());
  requireValue(latest.pass === true && latest.sourceHash === inputIdentity(root).sha256 &&
    latest.directory === `${coverageBase}/runs/${latest.runId}`, "live native coverage pointer stale or invalid");
  nativeCollection(root, latest.runId);
  const qa = boundJSON(root, summary.review);
  requireValue(equal(qa.coverage, recorded), "coverage not bound to independent review");
  validateIndependentReview(qa, source.sha256, host, summary.receipts, summary.qaReport, root);
  for (const [key, name, terms] of [["qaReport", "qa.md", ["macOS", "protocol", "coverage", "reconciliation"]],
    ["finalReport", "final.md", ["lifecycle", "desktop", "native Windows", "Phase 1 scheduling"]]] as const) {
    const ref = summary[key], bytes = evidenceBytes(root, `${directory}/${name}`);
    requireValue(ref?.path === `${directory}/${name}` && hash(bytes) === ref.sha256, `unbound ${name}`);
    requireValue(bytes.length >= (name === "qa.md" ? 1000 : 800) && terms.every(term => bytes.toString().includes(term)), `incomplete ${name}`);
    if (name === "qa.md") requireValue(equal(qa.report, ref), "QA report not bound to independent review");
  }
  const required = ["contract.json", "decision.md", "preservation.md", "final-summary.json", "qa.md", "final.md"].map(p => `${directory}/${p}`)
    .concat(summary.receipts.map((r: any) => r.path), [summary.review.path]);
  const trackedFiles = validateTrackedEvidence(root, required);
  return { sourceHash: source.sha256, trackedFiles, pass: true };
}

export function validateIndependentReview(qa: any, sourceHash: string, host: ReturnType<typeof nativeHost>, refs: any[], report: any, root = ".") {
  requireValue(qa.schemaVersion === 1 && qa.actorId === "reviewer" && qa.skill === "oma-qa" && qa.status === "passed" &&
    qa.sourceHash === sourceHash && equal(qa.host, host), "independent review not bound to final sources/native host");
  requireValue(equal(qa.reviewedReceipts, refs) && Array.isArray(qa.commands) && qa.commands.length > 0, "review did not bind command receipts/reproduction");
  for (const command of qa.commands) {
    validateCommandReceipt(command, sourceHash, host, "reviewer");
    requireValue(command.exit === 0, "independent test command failed");
    validateCargoExecution(command, root);
  }
  requireValue(qa.commands.some((r: any) => equal(r.argv, ["cargo", "test", "--workspace"])),
    "missing unfiltered independent cargo test --workspace execution");
  requireValue(equal(qa.report, report), "QA report not bound to independent review");
}
export function validateTrackedEvidence(root: string, required: string[]) {
  const entries = indexEntries(root, directory);
  requireValue(entries.length > 0 && entries.length < 150, "tracked evidence count must be between 1 and 149");
  requireValue(required.every(path => entries.some(e => e.path === path)), "required evidence is not tracked");
  for (const entry of entries) {
    const bytes = evidenceBytes(root, entry.path);
    requireValue(entry.mode === "100644" && bytes.equals(gitBytes(root, ["show", `:${entry.path}`])), `unstaged or nonregular evidence: ${entry.path}`);
  }
  return entries.length;
}

export function validateSourceBinding(summary: any, source: ReturnType<typeof managedInputs>, git: ReturnType<typeof managedGit>) {
  requireValue(equal(summary.inputs, source) && equal(summary.git, git), "current input/git hashes differ");
}
