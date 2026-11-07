import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { root, qa, sha, bytes, fingerprint, git, lines } from './audit-lib.mjs';

// Evidence writer for the two assigned Node audits only. Never runs Cargo gates
// or modifies dependency deliverables or product inputs.
for (const [name, file] of [
  ['structure', 'structure-and-mapping.test.mjs'],
  ['preservation', 'preservation-and-receipts.test.mjs'],
]) {
  const before = fingerprint().value;
  const argv = ['--test', `${qa}/${file}`];
  const inputPaths = [`${qa}/audit-lib.mjs`, `${qa}/${file}`, `${qa}/record-audits.mjs`];
  const inputs = inputPaths.map(path => ({ path, sha256: sha(bytes(path)) }));
  const start = new Date();
  const chunks = [];
  const child = spawn(process.execPath, argv, { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
  child.stdout.on('data', chunk => chunks.push(chunk));
  child.stderr.on('data', chunk => chunks.push(chunk));
  let timedOut = false;
  const timer = setTimeout(() => { timedOut = true; child.kill('SIGTERM'); }, 120000);
  const outcome = await new Promise((accept, reject) => {
    child.once('error', reject);
    child.once('close', (code, signal) => accept({ code, signal }));
  });
  clearTimeout(timer);
  const output = Buffer.concat(chunks);
  const records = output.toString('utf8').match(/[^\n]*\n|[^\n]+$/g) || [];
  const parts = [];
  for (let i = 0; i < records.length; i += 400) {
    const data = Buffer.from(records.slice(i, i + 400).join(''));
    const path = `${qa}/${name}.part-${String(parts.length + 1).padStart(2, '0')}.txt`;
    writeFileSync(resolve(root, path), data);
    parts.push({ path, sha256: sha(data), ...lines(data) });
  }
  const receipt = {
    command: ['node', ...argv], executable: process.execPath, cwd: root,
    startedAt: start.toISOString(), finishedAt: new Date().toISOString(),
    exitCode: outcome.code, signal: outcome.signal, timedOut, pid: child.pid,
    processReaped: true, sourceFingerprintBefore: before, sourceFingerprintAfter: fingerprint().value,
    inspectedHead: git('rev-parse', 'HEAD').toString().trim(), inputs,
    outputBytes: output.length, outputSha256: sha(output), outputParts: parts,
    outputStreams: 'stdout/stderr retained in observed stream-event order',
  };
  writeFileSync(resolve(root, `${qa}/${name}.json`), JSON.stringify(receipt, null, 2) + '\n');
  console.log(JSON.stringify({ name, exitCode: outcome.code, sourceFingerprint: before,
    tail: output.toString().slice(-2800) }));
  if (outcome.code !== 0) process.exitCode = 1;
}
