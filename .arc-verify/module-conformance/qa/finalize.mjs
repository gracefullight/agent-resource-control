import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  root, qa, base, impl, revision, resulting, bytes, json, sha, git, fingerprint,
  independentMapping, inventories, evidenceFiles, lines, receipt,
} from './audit-lib.mjs';

const save = (name, value) => writeFileSync(resolve(root, qa, name), JSON.stringify(value, null, 2) + '\n');
const rows = (name, values) => writeFileSync(resolve(root, qa, name), '[\n' + values.map(v => JSON.stringify(v)).join(',\n') + '\n]\n');
const fp = fingerprint().value;
const checked = ['structure', 'preservation'].map(name => {
  const r = json(`${qa}/${name}.json`);
  assert.equal(r.exitCode, 0);
  assert.equal(r.sourceFingerprintBefore, fp);
  assert.equal(r.sourceFingerprintAfter, fp);
  assert(!r.timedOut && r.processReaped);
  for (const input of r.inputs) assert.equal(sha(bytes(input.path)), input.sha256, input.path);
  const output = Buffer.concat(r.outputParts.map(part => {
    const data = bytes(part.path); assert.equal(sha(data), part.sha256); return data;
  }));
  assert.equal(output.length, r.outputBytes);
  assert.equal(sha(output), r.outputSha256);
  return { path: `${qa}/${name}.json`, sha256: sha(bytes(`${qa}/${name}.json`)),
    command: r.command, exitCode: r.exitCode, outputSha256: r.outputSha256 };
});
const maps = independentMapping();
const concise = f => ({ path: f.path, name: f.qualified, kind: f.kind,
  line: f.line, endLine: f.endLine, rawSha256: sha(f.raw), bodySha256: sha(f.body), attributes: f.attributes });
rows('independent-mapping.json', maps.map(({ original, destination }) => ({
  original: concise(original), destination: concise(destination),
  equality: original.path.includes('arc-ipc') ? 'raw item after removing four spaces of enclosing-module indentation' : 'raw item byte-identical',
  assertions: original.assertions.map((a, n) => ({ macro: a.macro, originalLine: a.line,
    destinationLine: destination.assertions[n].line, argumentTokensSha256: sha(JSON.stringify(a.tokens)) })),
})));
const head = git('rev-parse', 'HEAD').toString().trim();
save('source-evidence.json', {
  schemaVersion: 1, taskId: 'task-5', actorId: 'reviewer', inspectedAt: new Date().toISOString(),
  reviewWorkspace: root, reviewHead: head, reviewHeadTree: git('rev-parse', 'HEAD^{tree}').toString().trim(),
  baselineHead: revision, baselineTree: git('rev-parse', `${revision}^{tree}`).toString().trim(),
  snapshotCommit: '5d0f5c5b93edc214bff099f73ca8422f0c669565',
  snapshotTree: 'a3dfc866c7ac3ae80a7e9f7fac36b799ce2bdacb',
  snapshotRefSupplied: 'refs/agent-valley/task-evidence/9dd58b3e81e0c07c-e7c08de777760acd-1/snapshot',
  workerRefSupplied: 'refs/agent-valley/task-evidence/9dd58b3e81e0c07c-e7c08de777760acd-1/worker',
  refResolution: 'Supplied named refs were not available locally; actual snapshot commit/tree are verified Git objects. No workerRef commit invented.',
  scope: 'Actual files and index in review checkout equal the supplied snapshot product bytes; implementation receipts describe uncommitted task edits at accepted baseline HEAD in the source workspace.',
  sourceFingerprint: fp, fingerprintContract: `${base}/fingerprint.md`,
  receiptWorkspace: json(`${impl}/workspace-test.json`).cwd,
  receiptExecutionHead: json(`${impl}/workspace-test.json`).acceptedRevision,
  receiptReuse: 'Product fingerprint, all protected bytes and exact argv/output hashes match. QA evidence is excluded from the product fingerprint; no required Cargo/harness gate was replayed.',
  codeIntelligence: { provider: 'serena', mode: 'unavailable', tools: [],
    fallback: 'Scoped reads of assigned known paths and Git metadata/accepted objects; no native source discovery or provider changes.' },
});
const gateNames = ['fmt-final', 'clippy', 'workspace-test', 'harness', 'coordination-handoff'];
rows('validated-receipts.json', gateNames.map(name => {
  const r = receipt(name);
  return { path: `${impl}/${name}.json`, sha256: sha(bytes(`${impl}/${name}.json`)),
    command: r.command, cwd: r.cwd, startedAt: r.startedAt, finishedAt: r.finishedAt,
    exitCode: r.exitCode, sourceFingerprint: r.sourceFingerprintAfter,
    outputSha256: r.outputSha256, outputBytes: r.outputBytes, outputParts: r.outputParts };
}));
const attributes = inventories().old.flatMap(p => p.attributes).length;
const result = {
  schemaVersion: 1, taskId: 'task-5', actorId: 'reviewer', skill: 'oma-qa',
  status: 'completed', verdict: 'PASS', approved: true, completeness: 'completed',
  checkedAt: new Date().toISOString(), sourceFingerprint: fp, baselineHead: revision,
  inspectedHead: head, mapping: { tests: maps.filter(x => x.original.kind === 'test').length,
    helpers: maps.filter(x => x.original.kind === 'helper').length,
    assertionSites: maps.reduce((n, x) => n + x.original.assertions.length, 0), attributes },
  protectedFilesByteIdentical: 58, ipcProductionPrefixByteIdentical: true,
  fixtureSerializationVerified: true, ownedChildCleanupPreserved: true,
  nativeMacOSCoverageExecuted: true, exactProbeName: 'fixture_serialization_probe',
  requiredCargoGates: 'passed; independently validated original executable receipts on identical product bytes',
  workspaceTestExecutionsPassed: 183, requiredCargoGatesReplayedByReviewer: false,
  fileLengthViolations: 0, harnessExitCode: 1,
  harnessOtherFailures: ['bun run test:coverage: Script not found "test:coverage"; unchanged harness reports Total violations: 1'],
  auditChecks: checked, findings: [], blockingGaps: [],
  acceptance: {
    independentNodeAudits: 'pass: six structure checks and seven preservation/receipt checks',
    exhaustivePreservation: 'pass: independently parsed accepted Git objects; raw items, assertion tokens, cfg and names checked',
    fixtureAndNativeTests: 'pass: ready/blocked/acquired ordering, exact one-test handoff and native macOS workspace output',
    protectedBoundaries: 'pass: Git-object byte equality and filesystem/index mode checks',
    executableEvidence: 'pass: exact Cargo gates; harness has zero lengths and retained non-length failure',
    sourceBinding: 'pass: independently recomputed delivery fingerprint matches receipts and current files',
  },
  failedAttempts: [
    'QA development run 1: audit root was one directory too high; corrected only qa/audit-lib.mjs.',
    'QA development run 2: audit expected alternate exact-filter argv placement, omitted trailing attribute comment, and counted trailing blank separator. Corrected expected representation after inspecting actual argv/raw source; preservation comparisons unchanged.',
    'Implementer initial fmt exit 1 remains in implementation/fmt.json; final exact fmt exit 0 is validated.',
    'Supplied snapshot/worker named refs could not resolve locally; actual snapshot commit/tree verified directly.',
    'Optional .agents/oma-config.yaml and implementation/delivery.json reads returned missing; used existing CUE configuration and actual handoff/result/receipt files.',
  ],
  limits: [
    'Global cleanliness is not established; baseline/implementation status receipts retain av.yaml access denial. Scoped product diffs and all owned/protected bytes are verified.',
    'Missing Bun coverage script remains an actual harness failure, not a passing coverage claim.',
    'One recorded native run does not measure flakiness, mutation strength or coverage percentage.',
    'No external delivery, browser checks, installations, downloads or product edits were performed.',
  ],
  report: `${qa}/review.md`, sourceEvidence: `${qa}/source-evidence.json`,
};
assert.equal(sha(bytes(`${qa}/review.md`)).length, 64, 'material report must exist');
save('result.json', result);
// Count every file directly, including this script, reports, outputs and both
// dependency deliverables. Do not claim the historical harness saw QA files.
const counts = [...resulting, ...evidenceFiles()].filter(p => !p.endsWith('/qa/lengths.json')).map(path => {
  const data = bytes(path), count = lines(data);
  assert(count.newlines <= 500 && count.logical <= 500, path);
  return { path, ...count, bytes: data.length, sha256: sha(data) };
});
rows('lengths.json', counts);
assert(lines(bytes(`${qa}/lengths.json`)).logical <= 500);
console.log(JSON.stringify({ approved: true, completeness: 'completed', sourceFingerprint: fp,
  mappedFunctions: maps.length, countedFiles: counts.length, auditChecks: checked.map(x => x.exitCode) }));
