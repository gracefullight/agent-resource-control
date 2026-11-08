import { readFileSync, lstatSync, realpathSync } from "node:fs";
import { resolve, relative, basename } from "node:path";
import { checkHarnessStructure, gitBytes, indexEntries } from "./hygiene";
import { withHistoricalCheckout } from "./historical";
import { analyzeSource, hash, sourceInventory, tokens, walk } from "./source";
import { schemaReference } from "./schema-adaptations";

export const FEATURE_BASELINE = "19d7a507044cbf83918f58cd41056905c275c8d4";
export const MIGRATION = "crates/arc-storage/migrations/0001_phase0.sql";
export const MIGRATION_SHA256 = "046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384";
const textTokens = (text: string) => tokens(text).map(t => t.text);
const encode = (text: string) => JSON.stringify(textTokens(text));
const original = (root: string, path: string) => gitBytes(root, ["show", `${FEATURE_BASELINE}:${path}`]).toString();

// Item boundaries include attributes and the complete body, preventing inserted
// returns, changed helper inputs, ignored tests or cfg guards around old cases.
// New standalone items may be added; existing items must remain byte-independent
// token matches in their original order. Strings remain opaque lexer tokens.
export function rustItems(source: string): string[] {
  const ts = tokens(source), items: string[] = [];
  let start = 0, depth = 0;
  for (let i = 0; i < ts.length; i++) {
    const t = ts[i].text;
    if (["{", "(", "["].includes(t)) depth++;
    if (["}", ")", "]"].includes(t)) depth--;
    if (depth < 0) throw Error("REGRESSION: unbalanced item");
    if (depth === 0 && (t === ";" || t === "}")) {
      if (t === "}" && ts[i + 1]?.text === ";") i++;
      items.push(JSON.stringify(ts.slice(start, i + 1).map(t => t.text))); start = i + 1;
    }
  }
  if (depth || start !== ts.length) throw Error("REGRESSION: incomplete item");
  return items;
}
function containsItems(expected: string[], actual: string[], path: string) {
  let cursor = 0;
  for (const item of actual) if (item === expected[cursor]) cursor++;
  if (cursor !== expected.length) throw Error(`REGRESSION: existing test/helper item changed or unreachable: ${path}`);
}
// This is a conservative change guard, not Rust macro/name resolution. Pin
// unresolved macro constructs and wildcard imports at every nesting level to
// immutable baseline tokens. A new ordinary literal-expression macro is safe:
// its empty matcher and literal expansion cannot generate assertion bindings.
// Other new macros need a separately reviewed guard extension, not a name list.
function assertionMacroBindings(source: string) {
  const raw = textTokens(source), ts: string[] = [], bindings: string[] = [];
  // Raw identifiers name the same macro: r#assert_eq must not evade the scan.
  for (let i = 0; i < raw.length; i++) {
    if (raw[i] === "r" && raw[i + 1] === "#" && /^[A-Za-z_][A-Za-z_0-9]*$/.test(raw[i + 2] ?? "")) i += 2;
    ts.push(raw[i]);
  }
  const names = new Set(["assert", "assert_eq", "assert_ne", "debug_assert", "debug_assert_eq", "debug_assert_ne", "panic", "unreachable"]);
  const pairs: Record<string, string> = { "[": "]", "(": ")", "{": "}" };
  const endOf = (start: number): number => {
    const stack: string[] = [];
    for (let i = start; i < ts.length; i++) {
      if (pairs[ts[i]]) stack.push(pairs[ts[i]]);
      else if (["]", ")", "}"].includes(ts[i]) && stack.pop() !== ts[i]) throw Error("REGRESSION: unbalanced macro binding");
      if (!stack.length) return i;
    }
    throw Error("REGRESSION: incomplete macro binding");
  };
  const moduleBodies = new Set<number>(), scopes: number[] = [];
  for (let i = 0; i < ts.length; i++) if (ts[i] === "mod" && ts[i + 2] === "{") moduleBodies.add(i + 2);
  for (let i = 0; i < ts.length; i++) {
    if (ts[i] === "}") scopes.pop();
    // Unknown item expansions (including include! of non-.rs files and
    // external factories) may introduce any binding. Pin them without trying
    // to expand Rust. Ordinary macro expressions inside function bodies remain
    // editable; they cannot change the lexical scope of preserved module tests.
    if (ts[i] !== "macro_rules" && ts[i + 1] === "!" && pairs[ts[i + 2]] &&
      scopes.every(body => moduleBodies.has(body)))
      bindings.push(JSON.stringify(ts.slice(i, endOf(i + 2) + 1)));
    if (ts[i] === "#" && ts[i + 1] === "[") {
      const end = endOf(i + 1), attr = ts.slice(i, end + 1);
      if (attr.includes("macro_use")) bindings.push(JSON.stringify(attr));
    }
    if (ts[i] === "macro_rules" && ts[i + 1] === "!") {
      // A generated `$name` is not an ordinary identifier. Keep the complete
      // factory rather than assuming its invocation cannot bind an assertion.
      let body = i + 3;
      while (body < ts.length && !pairs[ts[body]]) body++;
      const end = endOf(body), item = ts.slice(i, end + 1);
      const expansion = ts.slice(body + 1, end);
      if (expansion.at(-1) === ";") expansion.pop();
      const literal = expansion.length >= 7 && expansion[0] === "(" && expansion[1] === ")" &&
        expansion[2] === "=" && expansion[3] === ">" && expansion[4] === "{" && expansion.at(-1) === "}" &&
        /^(?:[0-9][A-Za-z_0-9.]*|true|false|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)')$/.test(expansion.slice(5, -1).join(""));
      if (names.has(ts[i + 2]) || !literal) bindings.push(JSON.stringify(item));
    }
    if (ts[i] === "macro") {
      let body = i + 2;
      while (body < ts.length && !pairs[ts[body]]) body++;
      bindings.push(JSON.stringify(ts.slice(i, endOf(body) + 1)));
    }
    if (ts[i] === "use") {
      const end = ts.indexOf(";", i), item = ts.slice(i, end + 1);
      if (item.includes("*") || item.some(t => names.has(t))) bindings.push(JSON.stringify(item));
    }
    if (ts[i] === "{") scopes.push(i);
  }
  return JSON.stringify(bindings);
}
// Preserve the contexts that make old tests reachable, independently of mutable
// production bodies. No cfg evaluation is needed: cfg_attr and nested conditions
// must keep their immutable tokens, including on crate roots and external mods.
function regressionReachability(source: string) {
  const ts = tokens(source), close = new Map<number, number>(), open = new Map<number, number>();
  const stack: number[] = [], braces: number[] = [];
  const pairs: Record<string, string> = { "{": "}", "[": "]", "(": ")" };
  for (let i = 0; i < ts.length; i++) {
    if (pairs[ts[i].text]) { stack.push(i); if (ts[i].text === "{") braces.push(i); }
    else if (["}", "]", ")"].includes(ts[i].text)) {
      const start = stack.pop();
      if (start === undefined || pairs[ts[start].text] !== ts[i].text) throw Error("REGRESSION: unbalanced scope");
      close.set(start, i); open.set(i, start);
    }
  }
  if (stack.length) throw Error("REGRESSION: incomplete scope");
  const texts = (start: number, end: number) => ts.slice(start, end).map(t => t.text);
  const innerAttrs = (start: number) => {
    const attrs: string[][] = [];
    while (ts[start]?.text === "#" && ts[start + 1]?.text === "!" && ts[start + 2]?.text === "[") {
      const end = close.get(start + 2)!;
      attrs.push(texts(start, end + 1)); start = end + 1;
    }
    return attrs;
  };
  const modules: { start: number; body: number; header: string[]; inner: string[][] }[] = [];
  for (let i = 0; i < ts.length; i++) {
    if (ts[i].text !== "mod" || ![";", "{"].includes(ts[i + 2]?.text)) continue;
    let start = i;
    // Include restricted visibility and all outer attributes in the header.
    if (ts[start - 1]?.text === ")" && ts[(open.get(start - 1) ?? 0) - 1]?.text === "pub")
      start = open.get(start - 1)! - 1;
    else if (ts[start - 1]?.text === "pub") start--;
    while (ts[start - 1]?.text === "]") {
      const attr = open.get(start - 1)!;
      if (ts[attr - 1]?.text !== "#") break;
      start = attr - 1;
    }
    const body = ts[i + 2].text === "{" ? i + 2 : -1;
    modules.push({ start, body, header: texts(start, i + 3), inner: body < 0 ? [] : innerAttrs(body + 1) });
  }
  const byBody = new Map(modules.filter(m => m.body >= 0).map(m => [m.body, m]));
  const context = (at: number) => braces.filter(b => b < at && close.get(b)! > at).map(b => {
    const mod = byBody.get(b);
    // A function, macro or anonymous block cannot replace a module ancestor.
    return mod ? { header: mod.header, inner: mod.inner } : "block";
  });
  const info = analyzeSource(source), lines = source.split("\n");
  return {
    crateAttrs: JSON.stringify(innerAttrs(0)),
    modules: modules.map(m => JSON.stringify({ context: context(m.start), header: m.header, inner: m.inner })),
    spans: info.spans.map(s => JSON.stringify({
      context: context(ts.findIndex(t => t.line === s.start)),
      span: encode(lines.slice(s.start - 1, s.end).join("\n")),
    })),
  };
}
const cache = new Map<string, ReturnType<typeof regressionReferences>>();
function regressionReferences(root: string) {
  return withHistoricalCheckout(root, FEATURE_BASELINE, dir => {
    const inventory = sourceInventory(dir);
    return walk(resolve(dir, "crates")).filter(p => p.endsWith(".rs")).map(file => {
      const path = relative(dir, file);
      const source = schemaReference(path, readFileSync(file, "utf8"));
      const testOnly = inventory.find(f => f.path === path)?.testOnly ?? /\/tests\//.test(path);
      const info = analyzeSource(source);
      const spans = info.spans.map(s => encode(source.split("\n").slice(s.start - 1, s.end).join("\n")));
      const declarations = rustItems(source).filter(item => info.modules.some(m => m.testOnly &&
        JSON.parse(item).includes("mod") && JSON.parse(item).includes(m.name)));
      return { path, source, testOnly, items: testOnly ? rustItems(source) : [], spans, declarations,
        reachability: regressionReachability(source) };
    });
  });
}
// Fixed, task-5-only adaptation recipes. Compare all other bytes against the
// immutable feature baseline; these are not a moving checkout snapshot.
function toolingReference(root: string, path: string) {
  let text = original(root, path);
  if (path === "scripts/coverage/run.ts") text = text.replace("timeout=120000", "timeout=3600000")
    .replace('"TEST_FAILURE",coverageEnv,0,120000', '"TEST_FAILURE",coverageEnv,0,3600000');
  if (path === "scripts/coverage/commands.ts") text = text.replace("timeout=120000", "timeout=3600000");
  if (path === "scripts/coverage/acceptance.ts") text = text.replace("900000", "3600000");
  if (["scripts/coverage/harness.test.ts", "scripts/coverage/coverage-cli.test.ts"].includes(path)) text = text.replace("960000", "3600000");
  if (path === "scripts/coverage/clean-checkout.test.ts") text = text
    .replace('import { root }', 'import { checkPreservation } from "./preservation";\nimport { root }')
    .replace(', checkExactPreservation }', ' }').replaceAll("checkExactPreservation(", "checkPreservation(")
    .replace("900000", "3600000").replace("1860000", "7260000");
  if (path === "scripts/coverage/preservation.ts") text = text.replace("export function checkPreservation(", "export function checkHistoricalPreservation(") +
    '\n// Live feature checks preserve regression semantics rather than production bytes.\nexport { checkCurrentPreservation as checkPreservation } from "./current-preservation";\n';
  if (path === "scripts/coverage/preservation.test.ts") text = text.replaceAll("checkPreservation", "checkHistoricalPreservation")
    .replace('import { BASELINE }', 'import { withHistoricalCheckout } from "./historical";\nimport { BASELINE }')
    .replace('const result=checkHistoricalPreservation(resolve(import.meta.dir,"../.."));',
      'const result=withHistoricalCheckout(resolve(import.meta.dir,"../.."), BASELINE, checkHistoricalPreservation);');
  if (path === "scripts/coverage/hygiene-preservation.test.ts") text = text.replaceAll("checkPreservation", "checkHistoricalPreservation")
    .replace('test("protected bytes/modes', 'import { withHistoricalCheckout } from "./historical";\n\ntest("protected bytes/modes')
    .replace('expect(checkExactPreservation(root))', 'expect(withHistoricalCheckout(root, ACCEPTED_HEAD, checkExactPreservation))');
  return text;
}
const compactHarness = (text: string) => text.split("\n").filter((l, i) => i === 0 || l.trim() && !l.trimStart().startsWith("#")).join("\n") + "\n";
const protectedTooling = [
  "scripts/coverage/accounting.ts", "scripts/coverage/source.ts", "scripts/coverage/gate.ts",
  "scripts/coverage/run.ts", "scripts/coverage/config.ts", "scripts/coverage/fixtures.ts",
  "scripts/coverage/commands.ts", "scripts/coverage/acceptance.ts", "scripts/coverage/clean-checkout.test.ts",
  "scripts/coverage/harness.test.ts", "scripts/coverage/coverage-cli.test.ts", "scripts/coverage/preservation.ts",
  "scripts/coverage/preservation.test.ts", "scripts/coverage/hygiene-preservation.test.ts",
  "scripts/coverage/fixture-runner.ts", "scripts/coverage/accounting.test.ts", "scripts/coverage/gate.test.ts",
  "scripts/coverage/evidence.test.ts", ".arc-verify/rust-coverage/contract/accounting.json",
  "scripts/harness/validate.sh", "scripts/harness/equivalence.test.ts", "scripts/harness/fixtures.ts",
  "scripts/harness/test-support.ts", "scripts/harness/coverage-verification.test.ts",
];
function checkTestAdmission(root: string) {
  const manifests = gitBytes(root, ["ls-tree", "-r", "--name-only", FEATURE_BASELINE, "--", "crates"])
    .toString().trim().split("\n").filter(p => p.endsWith("/Cargo.toml"));
  for (const path of manifests) {
    const current: any = Bun.TOML.parse(readFileSync(resolve(root, path), "utf8"));
    const prior: any = Bun.TOML.parse(original(root, path));
    if (current.package?.autotests === false || current.package?.autolib === false || current.package?.autobins === false)
      throw Error(`REGRESSION: automatic test target admission disabled: ${path}`);
    if ((current.lib?.path ?? "src/lib.rs") !== (prior.lib?.path ?? "src/lib.rs"))
      throw Error(`REGRESSION: existing library test target redirected: ${path}`);
    for (const target of prior.bin ?? []) if (!(current.bin ?? []).some((t: any) => t.name === target.name && t.path === target.path))
      throw Error(`REGRESSION: existing binary test target redirected: ${path}`);
    const base = path.slice(0, -"Cargo.toml".length);
    const oldCases = gitBytes(root, ["ls-tree", "-r", "--name-only", FEATURE_BASELINE, "--", base + "tests"])
      .toString().trim().split("\n").filter(p => /^tests\/[^/]+\.rs$/.test(p.slice(base.length)));
    const existingBinaries = (prior.bin ?? [{ name: prior.package.name }]).map((t: any) => t.name);
    const protectedTargets = [current.lib, ...(current.bin ?? []).filter((t: any) => existingBinaries.includes(t.name)),
      ...(current.test ?? []).filter((t: any) => oldCases.some(p => basename(p, ".rs") === t.name))].filter(Boolean);
    for (const target of protectedTargets) {
      if (target.test === false || target.harness === false || target["required-features"]?.length)
        throw Error(`REGRESSION: existing target tests disabled or gated: ${path}`);
    }
    for (const target of current.test ?? []) {
      const old = oldCases.find(p => basename(p, ".rs") === target.name);
      if (old && (target.path ?? `tests/${target.name}.rs`) !== old.slice(base.length))
        throw Error(`REGRESSION: existing integration test target redirected: ${path}`);
    }
  }
}
export function checkCurrentPreservation(root: string) {
  // Native macOS temp roots may enter through /var while module identities use
  // /private/var. Use one root identity; retain every source and assertion check.
  root = realpathSync(root);
  checkHarnessStructure(root);
  checkTestAdmission(root);
  for (const path of [MIGRATION, ...protectedTooling]) {
    const file = resolve(root, path), stat = lstatSync(file);
    if (!stat.isFile() || stat.isSymbolicLink()) throw Error(`PRESERVATION: regular file required: ${path}`);
    const content = readFileSync(file);
    const expected = path === MIGRATION ? MIGRATION_SHA256 : hash(path === "scripts/harness/validate.sh" ? compactHarness(toolingReference(root, path)) : toolingReference(root, path));
    const actual = hash(content);
    if (actual !== expected) throw Error(`PRESERVATION: protected bytes changed: ${path}`);
    if (path === MIGRATION) {
      const [entry, ...extra] = indexEntries(root, path);
      if (!entry || extra.length || entry.mode !== "100644" || hash(gitBytes(root, ["show", `:${path}`])) !== expected)
        throw Error(`PRESERVATION: index migration bytes/mode changed: ${path}`);
      if (stat.mode & 0o111) throw Error("PRESERVATION: migration executable mode changed");
    }
  }
  let refs = cache.get(root);
  if (!refs) { refs = regressionReferences(root); cache.set(root, refs); }
  const inventory = sourceInventory(root);
  for (const ref of refs) {
    const source = readFileSync(resolve(root, ref.path), "utf8");
    const reachability = regressionReachability(source);
    if (ref.reachability.crateAttrs !== reachability.crateAttrs)
      throw Error(`REGRESSION: crate/module inner attributes changed: ${ref.path}`);
    containsItems(ref.reachability.modules, reachability.modules, ref.path);
    containsItems(ref.reachability.spans, reachability.spans, ref.path);
    const current = inventory.find(f => f.path === ref.path);
    if (ref.testOnly && current && !current.testOnly) throw Error(`REGRESSION: test/helper reachability changed: ${ref.path}`);
    const items = rustItems(source);
    containsItems(ref.items, items, ref.path);
    containsItems(ref.declarations, items, ref.path);
    const info = analyzeSource(source);
    const spans = info.spans.map(s => encode(source.split("\n").slice(s.start - 1, s.end).join("\n")));
    containsItems(ref.spans, spans, ref.path);
  }
  const files = walk(resolve(root, "crates")).filter(p => /\.(rs|sql)$/.test(p))
    .concat(walk(resolve(root, "scripts/coverage")), walk(resolve(root, "scripts/harness")));
  const references = new Map(refs.map(ref => [ref.path, ref.source]));
  for (const file of files) {
    if (file.endsWith(".rs")) {
      const path = relative(root, file), source = readFileSync(file, "utf8");
      if (assertionMacroBindings(source) !== assertionMacroBindings(references.get(path) ?? ""))
        throw Error(`REGRESSION: assertion macro binding/import changed: ${path}`);
    }
    if (readFileSync(file, "utf8").split(/\n/).length - Number(readFileSync(file, "utf8").endsWith("\n")) > 500)
      throw Error(`PRESERVATION: source exceeds 500 lines: ${relative(root, file)}`);
  }
  return { baselineHead: FEATURE_BASELINE, existingRustFiles: refs.length, pass: true };
}
