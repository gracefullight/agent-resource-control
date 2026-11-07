import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, statSync, readdirSync, existsSync } from 'node:fs';
import { resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

export const root = resolve(fileURLToPath(new URL('../../../', import.meta.url)));
export const base = '.arc-verify/module-conformance/baseline';
export const impl = '.arc-verify/module-conformance/implementation';
export const qa = '.arc-verify/module-conformance/qa';
export const revision = 'a905653d557f4c5246a1f656042cb8d29e46c052';
export const resident = 'crates/arc-daemon/tests/resident.rs';
export const ipc = 'crates/arc-ipc/src/lib.rs';
export const support = 'crates/arc-daemon/tests/support/mod.rs';
export const modules = {
  'crates/arc-daemon/tests/resident/coordination.rs': 'coordination',
  'crates/arc-daemon/tests/resident/identity.rs': 'identity',
  'crates/arc-daemon/tests/resident/lifecycle.rs': 'lifecycle',
  'crates/arc-daemon/tests/resident/provisioning.rs': 'provisioning',
  'crates/arc-ipc/src/tests/mod.rs': 'tests',
  'crates/arc-ipc/src/tests/framing.rs': 'tests::framing',
  'crates/arc-ipc/src/tests/dispatch.rs': 'tests::dispatch',
};
export const resulting = [resident, ipc, ...Object.keys(modules)];
export const bytes = path => readFileSync(resolve(root, path));
export const text = path => bytes(path).toString('utf8');
export const json = path => JSON.parse(text(path));
export const sha = data => createHash('sha256').update(data).digest('hex');
export const git = (...args) => execFileSync('git', args, { cwd: root, maxBuffer: 16 * 1024 * 1024 });
export const oldBytes = path => git('show', `${revision}:${path}`);
export const oldText = path => oldBytes(path).toString('utf8');
export const lineAt = (source, offset) => source.slice(0, offset).split('\n').length;
export function lines(data) {
  const s = data.toString();
  return { newlines: (s.match(/\n/g) || []).length,
    logical: s ? s.split('\n').length - Number(s.endsWith('\n')) : 0 };
}
export function evidenceFiles(dir = '.arc-verify/module-conformance') {
  return readdirSync(resolve(root, dir), { withFileTypes: true }).flatMap(entry => {
    const path = `${dir}/${entry.name}`;
    assert(!entry.isSymbolicLink(), `unexpected evidence symlink: ${path}`);
    return entry.isDirectory() ? evidenceFiles(path) : [path];
  });
}

// Independent scoped Rust lexer. String/comment content never controls delimiter
// matching; literal bytes are kept intact. Not a general Rust syntax parser.
export function lex(source) {
  const out = [];
  let i = 0;
  const emit = end => { out.push({ value: source.slice(i, end), start: i, end }); i = end; };
  while (i < source.length) {
    if (/\s/.test(source[i])) { i++; continue; }
    if (source.startsWith('//', i)) {
      const e = source.indexOf('\n', i); i = e < 0 ? source.length : e; continue;
    }
    if (source.startsWith('/*', i)) {
      let depth = 1; i += 2;
      while (depth && i < source.length) {
        if (source.startsWith('/*', i)) { depth++; i += 2; }
        else if (source.startsWith('*/', i)) { depth--; i += 2; }
        else i++;
      }
      assert.equal(depth, 0, 'unclosed comment'); continue;
    }
    const raw = /^(?:br|r)(#*)"/.exec(source.slice(i));
    if (raw) {
      const stop = '"' + raw[1];
      const end = source.indexOf(stop, i + raw[0].length);
      assert(end >= 0, 'unclosed raw string'); emit(end + stop.length); continue;
    }
    const prefix = /^(?:b)?"/.exec(source.slice(i));
    if (prefix) {
      let end = i + prefix[0].length;
      while (end < source.length && source[end] !== '"') end += source[end] === '\\' ? 2 : 1;
      assert(end < source.length, 'unclosed string'); emit(end + 1); continue;
    }
    const character = /^(?:b)?'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'/u.exec(source.slice(i));
    if (character) { emit(i + character[0].length); continue; }
    const word = /^[A-Za-z_][A-Za-z_0-9]*|^[0-9][A-Za-z_0-9.]*/.exec(source.slice(i));
    emit(i + (word ? word[0].length : 1));
  }
  return out;
}
export const tokenValues = source => lex(source).map(t => t.value);
export function parse(source, path, minimum = 0) {
  const tokens = lex(source), pairs = new Map(), stack = [];
  const opens = new Map([['(', ')'], ['[', ']'], ['{', '}']]);
  for (let n = 0; n < tokens.length; n++) {
    const v = tokens[n].value;
    if (opens.has(v)) stack.push(n);
    else if ([')', ']', '}'].includes(v)) {
      const start = stack.pop();
      assert.equal(opens.get(tokens[start]?.value), v, `unbalanced ${path}`);
      pairs.set(start, n); pairs.set(n, start);
    }
  }
  assert.equal(stack.length, 0);
  const attributes = [], impls = [], functions = [];
  tokens.forEach((t, n) => {
    if (t.value === '#' && tokens[n + 1]?.value === '[' && t.start >= minimum) {
      const end = tokens[pairs.get(n + 1)].end;
      attributes.push({ path, start: t.start, end, line: lineAt(source, t.start),
        text: source.slice(t.start, end) });
    }
    if (t.value === 'impl') {
      const opening = tokens.findIndex((v, k) => k > n && v.value === '{');
      impls.push({ start: t.start, end: tokens[pairs.get(opening)].end,
        owner: source.slice(t.end, tokens[opening].start).trim() });
    }
  });
  tokens.forEach((t, n) => {
    if (t.value !== 'fn' || t.start < minimum) return;
    const name = tokens[n + 1].value;
    const opening = tokens.findIndex((v, k) => k > n && v.value === '{');
    const end = tokens[pairs.get(opening)].end;
    let startIndex = n;
    if (tokens[n - 1]?.value === 'pub') startIndex--;
    const functionStart = tokens[startIndex].start;
    while (tokens[startIndex - 1]?.value === ']') {
      const bracket = pairs.get(startIndex - 1);
      if (tokens[bracket - 1]?.value !== '#') break;
      startIndex = bracket - 1;
    }
    const start = tokens[startIndex].start;
    const attrs = attributes.filter(a => a.start >= start && a.end <= functionStart);
    const enclosing = impls.filter(x => x.start < t.start && x.end > end)
      .sort((a, b) => a.end - a.start - (b.end - b.start))[0];
    let qualified = path === support ? 'support::' : modules[path] ? modules[path] + '::' : path === ipc ? 'tests::' : '';
    if (enclosing) {
      const bits = enclosing.owner.split(' for ');
      qualified += bits.length === 2 ? `<${bits[1]} as ${bits[0]}>::` : bits[0] + '::';
    }
    const parent = functions.filter(f => f.start < start && f.end > end)
      .sort((a, b) => a.end - a.start - (b.end - b.start))[0];
    qualified = parent ? parent.qualified + '::' + name : qualified + name;
    functions.push({ path, name, qualified, owner: enclosing?.owner || null,
      kind: attrs.some(a => a.text === '#[test]') ? 'test' : 'helper',
      start, end, fnStart: functionStart, bodyStart: tokens[opening].start,
      line: lineAt(source, functionStart), endLine: lineAt(source, end - 1),
      raw: source.slice(start, end), body: source.slice(tokens[opening].start, end),
      attributes: attrs.map(a => ({ line: a.line, text: a.text })), assertions: [] });
  });
  tokens.forEach((t, n) => {
    if (t.start < minimum || !/^(?:assert(?:_eq|_ne)?|debug_assert(?:_eq|_ne)?|panic|unreachable)$/.test(t.value)
      || tokens[n + 1]?.value !== '!') return;
    const owner = functions.filter(f => f.start <= t.start && f.end > t.end)
      .sort((a, b) => a.end - a.start - (b.end - b.start))[0];
    assert(owner, `assertion without function ${path}:${lineAt(source, t.start)}`);
    const end = tokens[pairs.get(n + 2)]?.end;
    assert(end, `unbalanced macro ${t.value}`);
    owner.assertions.push({ macro: t.value, line: lineAt(source, t.start),
      tokens: tokens.slice(n, pairs.get(n + 2) + 1).map(v => v.value) });
  });
  return { source, functions, attributes };
}
export function inventories() {
  const boundary = oldText(ipc).indexOf('#[cfg(test)]');
  assert(boundary > 0);
  const old = [parse(oldText(resident), resident), parse(oldText(ipc), ipc, boundary), parse(oldText(support), support)];
  const current = [resident, support, ...Object.keys(modules)].map(path => parse(text(path), path));
  return { old, current, boundary };
}
export function independentMapping() {
  const { old, current } = inventories();
  const before = old.flatMap(p => p.functions), after = current.flatMap(p => p.functions);
  const used = new Set();
  const mapping = before.map(original => {
    const group = path => path === support ? 'support' : path.includes('arc-ipc') ? 'ipc' : 'resident';
    const matches = after.filter(f => f.name === original.name && f.owner === original.owner && group(f.path) === group(original.path));
    assert.equal(matches.length, 1, `unique destination: ${original.qualified}`);
    const destination = matches[0];
    assert(!used.has(destination)); used.add(destination);
    const unindent = s => s.replace(/\n {4}/g, '\n');
    assert.equal(original.path === ipc ? unindent(original.raw) : original.raw, destination.raw,
      `raw function, comments, literals and signatures: ${original.qualified}`);
    assert.deepEqual(original.attributes.map(a => a.text), destination.attributes.map(a => a.text));
    assert.deepEqual(original.assertions.map(a => [a.macro, a.tokens]), destination.assertions.map(a => [a.macro, a.tokens]));
    return { original, destination };
  });
  assert.equal(used.size, after.length, 'no added/unmapped function');
  return mapping;
}
export function fingerprint() {
  const seeds = json(`${base}/fingerprint-files.json`).map(x => x.path);
  const listed = git('ls-files', '--cached', '--others', '--exclude-standard', '--', 'crates', 'scripts/harness', '.cargo').toString().trim().split('\n').filter(Boolean);
  const rows = [...new Set([...seeds, ...listed])].sort().map(path => {
    if (!existsSync(resolve(root, path))) return { missing: true, path };
    const b = bytes(path);
    // Alphabetical object keys reproduce the documented Python serialization.
    return { bytes: b.length, mode: '0o' + (statSync(resolve(root, path)).mode & 0o777).toString(8), path, sha256: sha(b) };
  });
  return { value: sha(JSON.stringify(rows)), rows };
}
export function receipt(name, folder = impl) {
  const value = json(`${folder}/${name}.json`);
  const parts = value.outputParts.map((part, i) => {
    assert.equal(part.path, `${name}.part-${String(i + 1).padStart(2, '0')}.txt`);
    const data = bytes(`${folder}/${part.path}`);
    assert.equal(sha(data), part.sha256, part.path);
    assert.equal(lines(data).newlines, part.lines);
    assert(lines(data).logical <= 500);
    return data;
  });
  const data = Buffer.concat(parts);
  assert.equal(sha(data), value.outputSha256, name);
  assert.equal(data.length, value.outputBytes, name);
  return { ...value, output: data.toString('utf8') };
}
export function stripRanges(source, ranges) {
  let result = source;
  for (const { start, end } of [...ranges].sort((a, b) => b.start - a.start)) {
    result = result.slice(0, start) + result.slice(end);
  }
  return result;
}
export const relativePath = path => relative(root, path);
