import test from 'node:test';
import assert from 'node:assert/strict';
import {
  base, impl, ipc, resident, support, modules, resulting, bytes, text, json,
  oldText, oldBytes, git, revision, sha, lines, evidenceFiles, inventories,
  independentMapping, tokenValues, stripRanges, parse, fingerprint,
} from './audit-lib.mjs';

test('all resulting source and evidence files meet both line-count definitions', () => {
  for (const path of [...resulting, ...evidenceFiles()]) {
    const count = lines(bytes(path));
    assert(count.newlines <= 500 && count.logical <= 500, `${path}: ${JSON.stringify(count)}`);
  }
  assert.equal(lines(oldBytes(resident)).newlines, 901);
  assert.equal(lines(oldBytes(ipc)).newlines, 551);
});

test('native modules are explicit, private and exhaustively wired', () => {
  const wiring = Object.entries(modules).filter(([p]) => p.includes('arc-daemon'))
    .map(([path, name]) => `#[path = "${path.replace('crates/arc-daemon/tests/', '')}"]\nmod ${name};`);
  for (const statement of wiring) assert(text(resident).includes(statement));
  assert(text(ipc).endsWith('#[cfg(test)]\nmod tests;\n'));
  const entry = text('crates/arc-ipc/src/tests/mod.rs');
  assert.match(entry, /^use super::\*;\n\nmod dispatch;\nmod framing;/);
  for (const path of Object.keys(modules).filter(p => !p.endsWith('/mod.rs'))) {
    assert(text(path).startsWith('use super::*;\n'), path);
    assert(!/\bpub\b|\binclude!/.test(text(path)), path);
  }
  const tracked = git('ls-files', '--cached', '--', ...resulting).toString().trim().split('\n');
  assert.deepEqual([...tracked].sort(), [...resulting].sort());
  for (const path of resulting) {
    assert.equal(git('rev-parse', `:${path}`).toString().trim(), git('hash-object', path).toString().trim(), path);
  }
});

test('derive each test, helper, assertion and attribute directly from accepted Git bytes', () => {
  const mapped = independentMapping();
  assert.equal(mapped.length, 51);
  assert.equal(mapped.filter(x => x.original.kind === 'test').length, 24);
  assert.equal(mapped.filter(x => x.original.kind === 'helper').length, 27);
  assert.equal(mapped.reduce((n, x) => n + x.original.assertions.length, 0), 137);
  const { old, current } = inventories();
  assert.equal(old.flatMap(p => p.attributes).length, 28);
  const inherited = current.flatMap(p => p.attributes).filter(a => !a.text.startsWith('#[path'));
  inherited.push(...parse(text(ipc), ipc, text(ipc).indexOf('#[cfg(test)]')).attributes);
  assert.deepEqual(inherited.map(a => a.text).sort(), old.flatMap(p => p.attributes).map(a => a.text).sort());
  const mac = mapped.filter(x => x.original.attributes.some(a => a.text.includes('target_os')));
  assert.equal(mac.length, 1);
  assert.equal(mac[0].destination.qualified, 'identity::native_default_and_cli_ignore_unset_changed_and_unwritable_tmpdir');
  assert.equal(mac[0].destination.attributes[0].text, '#[cfg(target_os = "macos")]');
});

test('implementer mapping is checked against independently derived ranges and assertion arguments', () => {
  const actual = independentMapping(), delivered = json(`${impl}/mapping.json`);
  assert.equal(delivered.baselineRevision, revision);
  assert.equal(delivered.sourceFingerprint, fingerprint().value);
  assert.equal(delivered.entries.length, actual.length);
  const consumed = new Set();
  for (const { original: old, destination: now } of actual) {
    const records = delivered.entries.filter(e => e.original.path === old.path &&
      e.original.name === old.name && e.original.range[0] === old.line);
    assert.equal(records.length, 1, old.qualified);
    const e = records[0]; consumed.add(e.originalId);
    assert.equal(e.originalId, `${old.path}:${old.line}:${old.name}`);
    for (const [record, derived] of [[e.original, old], [e.destination, now]]) {
      assert.equal(record.path, derived.path);
      assert.equal(record.qualifiedName, derived.qualified);
      assert.deepEqual(record.range, [derived.line, derived.endLine]);
      const source = record === e.original ? oldText(derived.path) : text(derived.path);
      assert.deepEqual(record.attributes, derived.attributes.map(a => ({
        line: a.line, text: source.split('\n')[a.line - 1].trim(),
      })));
      assert.deepEqual(record.assertionLines, derived.assertions.map(a => a.line));
      assert.equal(record.rawBodySha256, sha(derived.body));
    }
    assert.deepEqual(e.comparison.assertions, old.assertions.map((a, n) => ({
      originalLine: a.line, destinationLine: now.assertions[n].line, macro: a.macro, tokensEqual: true,
    })));
  }
  assert.equal(consumed.size, delivered.entries.length);
  const { old } = inventories();
  const oldAttrs = old.flatMap(p => p.attributes);
  assert.equal(delivered.attributes.length, oldAttrs.length);
  for (const a of oldAttrs) {
    const rows = delivered.attributes.filter(e => e.original.path === a.path && e.original.line === a.line);
    assert.equal(rows.length, 1);
    const originalLine = oldText(a.path).split('\n')[a.line - 1].trim();
    assert.equal(rows[0].original.text, originalLine);
    const dest = rows[0].destination;
    assert.equal(text(dest.path).split('\n')[dest.line - 1].trim(), originalLine);
  }
});

test('all residual statements and helper declaration order survive module extraction', () => {
  const { old, current, boundary } = inventories();
  const removedTests = old[0].functions.filter(f => f.kind === 'test' && f.name !== 'fixture_serialization_probe');
  const withoutWiring = text(resident).replace(/#\[path = "resident\/(coordination|identity|lifecycle|provisioning)\.rs"\]\nmod \1;\n/g, '');
  // Only blank separator lines left at extraction sites may collapse.
  const separators = s => s.replace(/\n{3,}/g, '\n\n').trimEnd();
  assert.equal(separators(stripRanges(old[0].source, removedTests)), separators(withoutWiring));
  for (const parsed of current.filter(p => p.functions.some(f => f.path in modules))) {
    const path = parsed.functions[0].path;
    const residue = stripRanges(parsed.source, parsed.functions.filter(f =>
      !parsed.functions.some(p => p.start < f.start && p.end > f.end)));
    const expected = path.endsWith('/framing.rs') ? 'use super::*;\nuse std::io::Cursor;' :
      path.endsWith('/mod.rs') ? 'use super::*;\nmod dispatch;\nmod framing;' : 'use super::*;';
    assert.deepEqual(tokenValues(residue), tokenValues(expected), path);
  }
  const inline = oldText(ipc).slice(boundary);
  const functions = old[1].functions.map(f => ({ start: f.start - boundary, end: f.end - boundary }));
  assert.deepEqual(tokenValues(stripRanges(inline, functions)),
    tokenValues('#[cfg(test)] mod tests { use super::*; use std::io::Cursor; }'));
  assert.equal(text(support), oldText(support));
});

test('independent parser preserves tricky literals, comments and nested assertion ownership', () => {
  const example = 'fn outer() { /* { /* } */ } */ let x = br#"} {"#; fn inner() { assert_eq!("a b", "a b"); } }';
  const p = parse(example, 'lexer-self-check');
  assert.equal(p.functions.length, 2);
  assert.equal(p.functions[0].assertions.length, 0);
  assert.equal(p.functions[1].assertions.length, 1);
  assert.notDeepEqual(tokenValues('assert_eq!("a b", "a b")'), tokenValues('assert_eq!("ab", "ab")'));
});
