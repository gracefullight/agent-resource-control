import { test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve, join } from "node:path";
import { root, evidence, metadata, writeJSON, timeout, sync, sha, environment } from "./test-support";
import { acceptedCorpus, materialize, verifyCorpus, measuredRun } from "./benchmark";

test("one original and three serial unskipped full runs preserve accepted corpus and meet prefix budget", async () => {
  const receipt: any = { ...metadata("timing"), startedAtUtc: new Date().toISOString(), runs: [], status: "running",
    command: { argv: ["bun", "test", "scripts/harness/performance.test.ts"], cwd: root, environment: environment() },
    systemUtilities: Object.fromEntries(["wc", "awk", "xargs"].map(name => { const path = Bun.which(name)!; return [name, { path, sha256: sha(readFileSync(path)), implementation: "host system utility; OS identified in host" }]; })),
    gitConfiguration: Object.fromEntries(["core.quotepath", "core.filemode", "core.excludesfile"].map(key => { const p = Bun.spawnSync(["git", "config", "--get", key], { cwd: root }); return [key, { value: p.stdout.toString().trim() || null, exit: p.exitCode }]; })),
    schedule: "original then candidate-1/2/3, serial real coverage; no suites started concurrently by this runner",
    observer: "external stdout/stderr drains; markers timestamped when complete marker bytes are received; prefix includes output delivery and Check 5 boundary audit",
    benchmarkCorrection: "Earlier benchmark incorrectly required runtime latest pointer immutability after Check 5. Fresh snapshots are audited before execution and at Check 5, and complete post-run deviations are retained with the collector pointer bound to fresh evidence.",
    boundaryNotes: "startup ends at Check 1 label; Checks 1–4 end at Check 5 label; Check 5 ends at Coverage thresholds met; finalization ends after exit and stream drain. processStartToCheck5 is the conservative acceptance measure.",
  };
  const save = () => writeJSON(resolve(evidence, "timing.json"), receipt);
  try {
    const corpus = acceptedCorpus();
    receipt.benchmarkCorpus = { entries: corpus.entries.length, blobBytes: corpus.blobBytes,
      gitTreeManifestSha256: corpus.gitTreeManifestSha256, pathModeContentManifestSha256: corpus.pathModeContentManifestSha256 };
    expect(corpus.entries.length).toBe(4044);
    expect(corpus.gitTreeManifestSha256).toBe("cc9e627ec4cc9f2ab8fa41b8be66e6ca285f64cf48729600d7aac587da955344");
    expect(corpus.pathModeContentManifestSha256).toBe("5a4cc80b455f45901c8c70825972150937f2d6c22402f706562247a1b72733b3");
    const initialSnapshot = materialize(); receipt.snapshots = [initialSnapshot]; receipt.initialCorpus = verifyCorpus(initialSnapshot.cwd, corpus); save();
    // A mutation probe establishes that the corpus guard actually rejects changes.
    const { writeFileSync, readFileSync } = await import("node:fs");
    const path = join(initialSnapshot.cwd, "README.md"), bytes = readFileSync(path);
    writeFileSync(path, Buffer.concat([bytes, Buffer.from("\n")]));
    expect(() => verifyCorpus(initialSnapshot.cwd, corpus)).toThrow("accepted corpus entry changed");
    writeFileSync(path, bytes);
    for (const label of ["original", "candidate-1", "candidate-2", "candidate-3"]) {
      const snapshot = label === "original" ? initialSnapshot : materialize();
      if (label !== "original") receipt.snapshots.push(snapshot);
      console.log(`Measuring ${label}: ${snapshot.scratch}`);
      receipt.runs.push(await measuredRun(label, snapshot, corpus)); save();
    }
    for (const run of receipt.runs) {
      expect(run.corpusPreserved).toBe(true);
      expect(run.gatePassed).toBe(true); expect(run.coverage.latest.pass).toBe(true);
      expect(run.coverage.crates.length).toBe(6);
      for (const crate of run.coverage.crates) expect(crate.covered * 100).toBeGreaterThanOrEqual(crate.total * 80);
      if (run.label !== "original") { expect(run.prefixPassed).toBe(true); expect(run.durationsSeconds.checks1to4).toBeLessThan(20); }
    }
    receipt.status = "passed"; receipt.exit = 0;
  } catch (error) { receipt.status = "failed"; receipt.exit = 1; receipt.error = String(error); throw error; }
  finally { receipt.endedAtUtc = new Date().toISOString(); save(); }
}, timeout * 4);
