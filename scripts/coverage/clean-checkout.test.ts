import { test, expect } from "bun:test";
import { mkdirSync, mkdtempSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { checkPreservation } from "./preservation";
import { root } from "./acceptance";
import { command, type Receipt } from "./commands";
import { inputIdentity, hash } from "./source";
import { BASELINE, CRATES, EVIDENCE } from "./config";
import { ACCEPTED_HEAD, gitBytes, auditTrackedEvidence, assertHygiene } from "./hygiene";

test("isolated checkout collects fresh native evidence with no inherited run artifacts", async () => {
  // Use the final index, including additions/deletions. Requiring identical input
  // identities fails if a relevant working-file change was not staged. No raw
  // reports, targets, runtime pointers or ignored files are copied from root.
  assertHygiene(auditTrackedEvidence(root));
  checkPreservation(root);
  const source = inputIdentity(root), tree = gitBytes(root, ["write-tree"]).toString().trim();
  const checkout = mkdtempSync(join(tmpdir(), "arc-clean-coverage-"));
  const receiptDir = resolve(root, "target/evidence-hygiene/clean-checkout/runs", crypto.randomUUID());
  mkdirSync(receiptDir, { recursive: true });
  const receipts: Receipt[] = [];
  const state: Record<string, unknown> = { classification: "Fresh", acceptedHead: ACCEPTED_HEAD, accountingBaseline: BASELINE,
    snapshotTree: tree, checkout, sourceHash: source.sha256, started: new Date().toISOString(), pass: false };
  const save = () => writeFileSync(join(receiptDir, "result.json"), JSON.stringify({ ...state, commands: receipts.map(({ stdout, stderr, ...r }) =>
    ({ ...r, stdoutSha256: hash(stdout), stderrSha256: hash(stderr) })) }, null, 2) + "\n");
  try {
    gitBytes(root, ["clone", "--quiet", "--shared", "--no-checkout", root, checkout]);
    gitBytes(checkout, ["read-tree", tree]);
    gitBytes(checkout, ["checkout-index", "--all"]);
    for (const commit of [ACCEPTED_HEAD, BASELINE]) gitBytes(checkout, ["cat-file", "-e", `${commit}^{commit}`]);
    const paths = gitBytes(checkout, ["ls-files", "-z"]).toString().split("\0").filter(Boolean);
    const prior = paths.filter(path => path.startsWith("target/") || path.startsWith(".arc-verify/") &&
      (/(?:^|\/)(runs|profiles)\//.test(path) || /\.(profraw|profdata|lcov)$/.test(path) || /\/(raw|llvm-export)\.json$/.test(path)));
    expect(prior).toEqual([]);
    expect(existsSync(join(checkout, "target"))).toBe(false);
    expect(existsSync(join(checkout, EVIDENCE, "baseline/runs"))).toBe(false);
    expect(existsSync(join(checkout, EVIDENCE, "baseline/latest.json"))).toBe(false);
    assertHygiene(auditTrackedEvidence(checkout));
    expect(inputIdentity(checkout)).toEqual(source);
    checkPreservation(checkout);
    state.precollection = { priorArtifacts: prior, targetAbsent: true, runsAbsent: true, runtimeLatestAbsent: true, sourceIdentityMatches: true };
    save();
    const env = { ...process.env }; delete env.SKIP_COVERAGE;
    for (const argv of [["bun", "run", "test:coverage"], ["bun", "test", "scripts/coverage/evidence.test.ts"]]) {
      console.log(`Clean checkout ${checkout}: ${argv.join(" ")}`);
      const receipt = await command(argv, receiptDir, checkout, env, 3600000);
      receipts.push(receipt); save();
      if (receipt.exit !== 0) throw Error(`CLEAN_CHECKOUT: ${argv.join(" ")} exited ${receipt.exit}\n${receipt.stdout}\n${receipt.stderr}\nReceipts: ${receiptDir}`);
    }
    const latest = JSON.parse(readFileSync(join(checkout, EVIDENCE, "baseline/latest.json"), "utf8"));
    const summary = JSON.parse(readFileSync(resolve(checkout, latest.directory, "summary.json"), "utf8"));
    expect(latest.pass).toBe(true); expect(latest.sourceHash).toBe(source.sha256);
    expect(summary.inputs).toEqual(source); expect(summary.crates.map((c: any) => c.crate)).toEqual([...CRATES]);
    for (const crate of summary.crates) {
      expect(crate.pass).toBe(true); expect(crate.total).toBeGreaterThan(0);
      expect(crate.covered * 100).toBeGreaterThanOrEqual(crate.total * 80);
    }
    expect(summary.rawSha256).toMatch(/^[a-f0-9]{64}$/); expect(summary.llvmSha256).toMatch(/^[a-f0-9]{64}$/);
    expect(inputIdentity(checkout)).toEqual(source); expect(inputIdentity(root)).toEqual(source);
    checkPreservation(checkout);
    state.pass = true; state.runId = latest.runId; state.directory = latest.directory;
    state.host = summary.host; state.crates = summary.crates;
    state.nativeHashes = { lcov: summary.rawSha256, llvmJson: summary.llvmSha256 };
    state.childProof = summary.childProof;
  } catch (error) { state.error = String(error); throw error; }
  finally {
    state.ended = new Date().toISOString(); save();
    console.log(`Fresh checkout and native artifacts retained at ${checkout}; receipts: ${receiptDir}`);
  }
}, 7260000);
