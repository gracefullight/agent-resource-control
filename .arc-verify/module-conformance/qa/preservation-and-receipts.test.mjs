import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, statSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  root, base, impl, revision, resident, ipc, support, resulting, bytes, text,
  json, oldBytes, oldText, sha, git, fingerprint, receipt, independentMapping,
} from './audit-lib.mjs';

const commands = {
  'fmt-final': ['cargo', 'fmt', '--all', '--', '--check'],
  clippy: ['cargo', 'clippy', '--workspace', '--all-targets', '--', '-D', 'warnings'],
  'workspace-test': ['cargo', 'test', '--workspace'],
  harness: ['./scripts/harness/validate.sh'],
  'coordination-handoff': ['cargo', 'test', '-p', 'arc-daemon', '--test', 'resident',
    '--', '--exact', 'coordination::fixture_coordination_is_shared_across_processes'],
};

test('accepted baseline objects and snapshot provenance retain their actual identity', () => {
  assert.equal(git('rev-parse', `${revision}^{tree}`).toString().trim(), 'c72fead1fdc7262d4a297d18e74f89e2bbfd6157');
  assert.equal(git('rev-parse', '5d0f5c5b93edc214bff099f73ca8422f0c669565^{tree}').toString().trim(),
    'a3dfc866c7ac3ae80a7e9f7fac36b799ce2bdacb');
  const baseline = json(`${base}/baseline.json`);
  assert.equal(baseline.acceptedRevision, revision);
  const actual = git('ls-tree', '-r', '--name-only', revision, '--', 'crates', 'scripts/harness',
    'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'mise.toml', '.cargo',
    'docs/architecture/CONSTRAINTS.md', 'docs/harness', '.agents/oma-config.cue').toString().trim().split('\n');
  const saved = json(`${base}/source-files.json`);
  assert.deepEqual(saved.map(x => x.path).sort(), actual.sort());
  for (const item of saved) {
    const data = oldBytes(item.path);
    assert.equal(sha(data), item.sha256, item.path);
    assert.equal(data.length, item.bytes, item.path);
    assert.equal(git('rev-parse', `${revision}:${item.path}`).toString().trim(), item.gitBlob);
  }
});

test('all protected Git bytes and modes remain unchanged, including migration and harness', () => {
  const saved = json(`${base}/source-files.json`);
  let count = 0;
  for (const item of saved.filter(x => ![resident, ipc].includes(x.path))) {
    assert.deepEqual(bytes(item.path), oldBytes(item.path), item.path);
    const mode = git('ls-tree', revision, '--', item.path).toString().split(' ')[0];
    const currentMode = git('ls-files', '--stage', '--', item.path).toString().split(' ')[0];
    assert.equal(currentMode, mode, item.path);
    assert.equal((statSync(resolve(root, item.path)).mode & 0o777).toString(8), mode.slice(-3), item.path);
    count++;
  }
  assert.equal(count, 58);
  const prefix = oldBytes(ipc).subarray(0, oldBytes(ipc).indexOf('#[cfg(test)]'));
  assert.deepEqual(bytes(ipc).subarray(0, prefix.length), prefix);
  assert.equal(bytes(ipc).subarray(prefix.length).toString(), '#[cfg(test)]\nmod tests;\n');
  assert.equal(sha(prefix), json(`${base}/baseline.json`).ipcProductionPrefix.sha256);
  const changed = git('diff', '--name-only', revision, '--', 'crates', 'scripts/harness',
    'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'mise.toml').toString().trim().split('\n');
  assert.deepEqual(changed.sort(), [...resulting].sort());
});

test('source fingerprint is recomputed over actual integrated source, not substituted HEAD', () => {
  const current = fingerprint(), saved = json(`${impl}/fingerprint-files.json`);
  const sortKeys = row => Object.fromEntries(Object.entries(row).sort(([a], [b]) => a.localeCompare(b)));
  assert.deepEqual(current.rows, saved.map(sortKeys));
  assert.equal(current.value, '0b82e06fc1588151af7ae61620539025e5e00038935e4f0fcd54fcda43b8e64a');
  assert.equal(current.value, json(`${impl}/mapping.json`).sourceFingerprint);
  assert.notEqual(current.value, json(`${base}/baseline.json`).fingerprint.value);
  const seeds = json(`${base}/fingerprint-files.json`);
  for (const row of seeds.filter(x => !x.missing)) assert.equal(sha(oldBytes(row.path)), row.sha256, row.path);
  assert.equal(sha(JSON.stringify(seeds.map(sortKeys))), json(`${base}/baseline.json`).fingerprint.value);
  for (const path of resulting) {
    assert.deepEqual(bytes(path), git('show', `5d0f5c5b93edc214bff099f73ca8422f0c669565:${path}`), path);
  }
});

test('exact required receipts contain complete hash-checked output on this source', () => {
  const fp = fingerprint().value, index = json(`${impl}/commands.json`);
  for (const row of index) {
    const r = receipt(row.receipt.replace(/\.json$/, ''));
    assert.deepEqual(r.command, row.command);
    assert.equal(r.exitCode, row.exitCode);
    assert.equal(r.outputSha256, row.outputSha256);
    assert.equal(r.sourceFingerprintAfter, row.sourceFingerprint);
    assert(r.processReaped && !r.timedOut, row.receipt);
  }
  for (const [name, argv] of Object.entries(commands)) {
    const r = receipt(name);
    assert.deepEqual(r.command, argv, name);
    assert.equal(r.acceptedRevision, revision, 'historical execution HEAD scope');
    assert.equal(r.sourceFingerprintBefore, fp, name);
    assert.equal(r.sourceFingerprintAfter, fp, name);
    assert(r.processReaped && !r.timedOut, name);
    assert.equal(r.exitCode, name === 'harness' ? 1 : 0, name);
    assert.equal(r.environmentOverrides.CARGO_NET_OFFLINE, 'true');
    assert.equal(r.environmentOverrides.RUSTUP_AUTO_INSTALL, '0');
    assert(r.environmentRemoved.includes('SKIP_COVERAGE'));
    assert(!('SKIP_COVERAGE' in r.environmentOverrides));
    assert.equal(r.environmentContext.WORKSPACE_ROOT, null);
    assert.equal(r.environmentContext.RUSTUP_TOOLCHAIN, '1.95.0');
    assert.match(r.host, /^macOS-.*arm64.*Mach-O$/);
    assert.equal(r.cwd, json(`${base}/baseline.json`).workspace);
    assert(Date.parse(r.startedAt) <= Date.parse(r.finishedAt));
    assert(r.elapsedSeconds >= 0 && r.elapsedSeconds <= 120);
    const running = json(`${impl}/${name}.running.json`);
    assert.equal(running.pid, r.pid);
    assert.equal(running.sourceFingerprintBefore, fp);
    assert.equal(running.startedAt, r.startedAt);
  }
  for (const dir of [base, impl]) {
    // Freeze dependency deliverables through their original artifact manifests.
    for (const artifact of json(`${dir}/artifacts.json`)) {
      assert.equal(sha(bytes(artifact.path)), artifact.sha256, artifact.path);
    }
  }
  const initialFmt = receipt('fmt');
  assert.equal(initialFmt.exitCode, 1, 'retain failed pre-format attempt');
  assert.notEqual(initialFmt.sourceFingerprintAfter, fp);
  assert.equal(receipt('fmt-final').output, '');
  assert.match(receipt('clippy').output, /Finished `dev` profile/);
});

test('workspace output proves every moved native test and the CLI contracts executed', () => {
  const output = receipt('workspace-test').output;
  for (const { destination } of independentMapping().filter(x => x.original.kind === 'test')) {
    assert(output.includes(`test ${destination.qualified} ... ok`), destination.qualified);
  }
  const statuses = [...output.matchAll(/^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;.*$/gm)];
  assert(statuses.length >= 10);
  assert.equal(statuses.reduce((sum, m) => sum + Number(m[1]), 0), 183);
  assert(statuses.every(m => m[2] === '0' && m[3] === '0'));
  assert(!/^test result: FAILED/m.test(output));
  const cliTests = oldText('crates/arc-cli/tests/binary.rs').matchAll(/#\[test\]\s*fn\s+(\w+)/g);
  for (const [, name] of cliTests) assert(output.includes(`test ${name} ... ok`), name);
  assert(output.includes('identity::native_default_and_cli_ignore_unset_changed_and_unwritable_tmpdir ... ok'));
  assert(output.includes('lifecycle::resident_contract_holds_until_sigterm ... ok'));
  const handoff = receipt('coordination-handoff');
  assert.match(handoff.output, /running 1 test\n/);
  assert.match(handoff.output, /test coordination::fixture_coordination_is_shared_across_processes \.\.\. ok/);
  assert.match(handoff.output, /1 passed; 0 failed; 0 ignored;.*10 filtered out/);
  const mapped = independentMapping().filter(x => x.original.kind === 'test');
  for (const [area, originalPath] of [['resident', resident], ['ipc', ipc]]) {
    const oldNames = [...receipt(`${area}-discovery`, base).output.matchAll(/^(.+): test$/gm)].map(m => m[1]).sort();
    const newNames = [...receipt(`${area}-discovery`).output.matchAll(/^(.+): test$/gm)].map(m => m[1]).sort();
    assert.deepEqual(oldNames, mapped.filter(x => x.original.path === originalPath).map(x => x.original.qualified).sort());
    assert.deepEqual(newNames, mapped.filter(x => x.original.path === originalPath).map(x => x.destination.qualified).sort());
  }
});

test('fixture ordering, exact subprocess discovery, handshake assertions and owned-child reaping persist', () => {
  const source = text(resident), lock = text(support);
  assert.match(source, /impl Drop for Daemon[\s\S]*?self\.0\.kill\(\)[\s\S]*?self\.0\.wait\(\)/);
  assert(source.includes('read -r gate; exec \\"$@\\"'));
  assert(source.indexOf('fs::write(root.join("ready")') < source.lastIndexOf('let _serial = support::serial()'));
  assert(source.lastIndexOf('let _serial = support::serial()') < source.indexOf('fs::write(root.join("acquired")'));
  assert.match(lock, /_file: Option<File>,\s*_thread: MutexGuard/);
  assert.match(lock, /pub fn release_process_lock\(&mut self\) \{\s*self\._file\.take\(\);\s*\}/);
  assert.match(lock, /static THREAD: Mutex<\(\)>/);
  assert.match(lock, /arc-fixtures-\{uid\}\.lock/);
  assert.match(lock, /file\.try_lock\(\)/);
  const coordination = text('crates/arc-daemon/tests/resident/coordination.rs');
  const milestones = [
    'let mut serial = support::serial()', 'let fixture = Fixture::new()',
    'let mut child = Daemon(', '.args(["--exact", "fixture_serialization_probe"])',
    'while !fixture.directory.join("ready").exists()',
    'assert!(!fixture.directory.join("acquired").exists())', 'serial.release_process_lock()',
    'assert!(wait_for_exit(&mut child).success())', 'assert!(fixture.directory.join("acquired").exists())',
  ];
  let position = -1;
  for (const part of milestones) {
    const next = coordination.indexOf(part, position + 1);
    assert(next > position, part); position = next;
  }
  const discovery = receipt('resident-discovery');
  const names = [...discovery.output.matchAll(/^(.+): test$/gm)].map(m => m[1]);
  assert.equal(names.filter(name => name === 'fixture_serialization_probe').length, 1);
  assert.equal(names.length, 11);
  // Raw equality in structure-and-mapping also protects every lexical guard,
  // child and fixture declaration and their reverse drop order in all scenarios.
});

test('unchanged full harness includes tracked modules and retains its actual coverage failure', () => {
  const r = receipt('harness'), clean = r.output.replace(/\x1b\[[0-9;]*m/g, '');
  assert.match(clean, /Mode: all-tracked/);
  assert.match(clean, /Files to check: 2416/);
  for (let n = 1; n <= 5; n++) assert(clean.includes(`Check ${n}/5:`));
  assert(clean.includes('No oversized files found'));
  assert(!/File exceeds 500-line cap:|Grandfathered file grew above/.test(clean));
  assert(!/skipping coverage|No files to validate/.test(clean));
  assert(clean.includes('error: Script not found "test:coverage"'));
  assert(clean.includes('Tests or coverage check failed'));
  assert(clean.includes('Total violations: 1'));
  assert.equal(r.exitCode, 1);
  const tracked = receipt('tracked-modules');
  assert(Date.parse(tracked.finishedAt) < Date.parse(r.startedAt));
  const rows = tracked.output.trim().split('\n');
  assert.equal(rows.length, resulting.length);
  for (const path of resulting) {
    const line = rows.find(v => v.endsWith('\t' + path));
    assert(line, path);
    assert.equal(line.split(' ')[1], git('hash-object', path).toString().trim(), path);
    assert(git('ls-files', '--', path).toString().trim() === path);
  }
  assert.deepEqual(bytes('scripts/harness/validate.sh'), oldBytes('scripts/harness/validate.sh'));
  assert(!existsSync(resolve(root, 'package.json')), 'do not create unrelated Bun coverage configuration');
});
