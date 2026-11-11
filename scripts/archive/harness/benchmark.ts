import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, lstatSync, statSync, readlinkSync, realpathSync, chmodSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { root, acceptedHead, sync, sha, implementation, observe } from "../../harness/test-support";

export type Entry = { path: string; mode: string; oid: string; sha256: string };
export function acceptedCorpus() {
  const manifest = sync(["git", "ls-tree", "-r", "-z", "--full-tree", acceptedHead]);
  const entries = manifest.toString().split("\0").filter(Boolean).map(row => {
    const at = row.indexOf("\t"), [mode, type, oid] = row.slice(0, at).split(" ");
    if (type !== "blob") throw Error(`unexpected accepted object ${row}`);
    return { path: row.slice(at + 1), mode, oid, sha256: "" };
  });
  const blobs = sync(["git", "cat-file", "--batch"], root, Buffer.from(entries.map(e => e.oid).join("\n") + "\n"));
  let offset = 0, blobBytes = 0;
  const records: Buffer[] = [];
  const net = (s: string) => Buffer.concat([Buffer.from(String(Buffer.byteLength(s)) + ":"), Buffer.from(s), Buffer.from(",")]);
  for (const entry of entries) {
    const end = blobs.indexOf(10, offset), [oid, type, size] = blobs.subarray(offset, end).toString().split(" ");
    if (oid !== entry.oid || type !== "blob") throw Error(`invalid blob ${entry.path}`);
    const bytes = blobs.subarray(end + 1, end + 1 + Number(size));
    entry.sha256 = sha(bytes); blobBytes += bytes.length; offset = end + Number(size) + 2;
    records.push(Buffer.concat([...[entry.mode, type, oid, entry.path, entry.sha256].map(net), Buffer.from("\n")]));
  }
  return { entries, blobBytes, gitTreeManifestSha256: sha(manifest), pathModeContentManifestSha256: sha(Buffer.concat(records)) };
}
export function materialize() {
  const scratch = realpathSync(mkdtempSync(join(tmpdir(), "arc-harness-benchmark-"))), cwd = join(scratch, "corpus");
  sync(["git", "clone", "--quiet", "--shared", "--no-checkout", root, cwd]);
  sync(["git", "read-tree", acceptedHead], cwd);
  sync(["git", "checkout-index", "--all"], cwd);
  const candidate = join(scratch, "candidate/scripts/harness/validate.sh");
  mkdirSync(resolve(candidate, ".."), { recursive: true });
  writeFileSync(candidate, readFileSync(resolve(root, "scripts/harness/validate.sh"))); chmodSync(candidate, 0o755);
  return { scratch, cwd, candidate };
}
export function inspectCorpus(cwd: string, corpus: ReturnType<typeof acceptedCorpus>) {
  const expectedTree = sync(["git", "rev-parse", `${acceptedHead}^{tree}`]).toString().trim();
  const tree = sync(["git", "write-tree"], cwd).toString().trim();
  const deviations: any[] = tree === expectedTree ? [] : [{ path: "index", expected: expectedTree, actual: tree }];
  const working: any[] = [];
  const dereferenced: { path: string; state: string; sha256: string | null }[] = [];
  for (const entry of corpus.entries) {
    const path = join(cwd, entry.path), stat = lstatSync(path);
    const mode = stat.isSymbolicLink() ? "120000" : stat.mode & 0o111 ? "100755" : "100644";
    const bytes = stat.isSymbolicLink() ? Buffer.from(readlinkSync(path)) : readFileSync(path);
    const actual = { mode, sha256: sha(bytes) };
    working.push({ path: entry.path, ...actual });
    if (mode !== entry.mode || actual.sha256 !== entry.sha256) deviations.push({ path: entry.path, expected: { mode: entry.mode, sha256: entry.sha256 }, actual });
    let state = "regular-file", digest: string | null = actual.sha256;
    if (stat.isSymbolicLink()) {
      let target;
      try { target = statSync(path); } catch (error: any) { if (error.code !== "ENOENT") throw error; }
      state = !target ? "missing" : target.isFile() ? "link-to-file" : target.isDirectory() ? "link-to-directory" : "other";
      digest = target?.isFile() ? sha(readFileSync(path)) : null;
    }
    dereferenced.push({ path: entry.path, state, sha256: digest });
  }
  return { tree, entries: corpus.entries.length, manifestSha256: corpus.pathModeContentManifestSha256, deviations, workingManifestSha256: sha(JSON.stringify(working)),
    dereferencedSha256: sha(JSON.stringify(dereferenced)), symlinkReferentsSha256: sha(JSON.stringify(dereferenced.filter(e => e.state !== "regular-file"))), states: Object.fromEntries([...new Set(dereferenced.map(e => e.state))].map(s => [s, dereferenced.filter(e => e.state === s).length])) };
}
export function verifyCorpus(cwd: string, corpus: ReturnType<typeof acceptedCorpus>) {
  const result = inspectCorpus(cwd, corpus);
  if (result.deviations.length) throw Error(`accepted corpus entry changed: ${result.deviations[0].path}`);
  return result;
}
export async function measuredRun(label: string, snapshot: ReturnType<typeof materialize>, corpus: ReturnType<typeof acceptedCorpus>) {
  const before = verifyCorpus(snapshot.cwd, corpus), env = { ...process.env, WORKSPACE_ROOT: join(snapshot.scratch, "workspaces") };
  delete env.SKIP_COVERAGE;
  const argv = [label === "original" ? "./scripts/harness/validate.sh" : snapshot.candidate];
  let boundary: ReturnType<typeof inspectCorpus> | null = null;
  const latestRelative = ".arc-verify/rust-coverage/baseline/latest.json";
  const latestPath = join(snapshot.cwd, latestRelative);
  const pointerBefore = sha(readFileSync(latestPath));
  const result = await observe(argv, snapshot.cwd, env, join(snapshot.scratch, label), key => {
    if (key === "check5") boundary = inspectCorpus(snapshot.cwd, corpus);
  });
  const after = inspectCorpus(snapshot.cwd, corpus);
  const scanInputsPreserved = boundary !== null && JSON.stringify(before) === JSON.stringify(boundary);
  const unexplainedDeviations = after.deviations.filter(d => d.path !== latestRelative);
  const pointerAfter = sha(readFileSync(latestPath));
  if (sha(readFileSync(snapshot.candidate)) !== implementation()[0].sha256) throw Error("candidate changed during benchmark");
  let coverage: any = null;
  try {
    const latest = JSON.parse(readFileSync(latestPath, "utf8"));
    const summary = JSON.parse(readFileSync(join(snapshot.cwd, latest.directory, "summary.json"), "utf8"));
    const receipt = JSON.parse(readFileSync(join(snapshot.cwd, latest.directory, "receipt.json"), "utf8"));
    coverage = { latest, latestSha256: pointerAfter,
      summarySha256: sha(readFileSync(join(snapshot.cwd, latest.directory, "summary.json"))),
      receiptSha256: sha(readFileSync(join(snapshot.cwd, latest.directory, "receipt.json"))), runId: summary.runId, crates: summary.crates, host: summary.host, versions: summary.versions,
      sourceHash: summary.sourceHash, rawSha256: summary.rawSha256, llvmSha256: summary.llvmSha256,
      commandExits: receipt.receipts.map((r: any) => ({ argv: r.argv, exit: r.exit })), directory: join(snapshot.cwd, latest.directory) };
  } catch (error) { coverage = { error: String(error) }; }
  const runtimePointerBound = coverage.latest?.pass === true && coverage.latest.runId === coverage.runId &&
    coverage.latest.sourceHash === coverage.sourceHash && coverage.commandExits.length > 0;
  const corpusPreserved = scanInputsPreserved && unexplainedDeviations.length === 0 && before.symlinkReferentsSha256 === after.symlinkReferentsSha256 && runtimePointerBound;
  return { label, ...result.receipt, corpusBefore: before, corpusAtCheck5: boundary, corpusAfter: after,
    scanInputsPreserved, unexplainedDeviations, corpusPreserved,
    runtimePointer: { path: latestRelative, beforeSha256: pointerBefore, afterSha256: pointerAfter,
      changed: pointerBefore !== pointerAfter, explainedBy: "accepted scripts/coverage/run.ts runtime latest-pointer write", bound: runtimePointerBound }, coverage,
    gatePassed: result.receipt.exit === 0 && result.stdout.includes("Coverage thresholds met") && !result.stderr.includes("skipping coverage gate"),
    prefixPassed: result.receipt.durationsSeconds.processStartToCheck5 !== null && result.receipt.durationsSeconds.processStartToCheck5 < 20,
    stdoutSnippet: result.stdout.toString().slice(-2500), stderrSnippet: result.stderr.toString().slice(-2500) };
}
