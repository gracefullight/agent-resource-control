import { readFileSync, lstatSync } from "node:fs";
import { resolve, relative } from "node:path";
import { gitBytes } from "../coverage/hygiene";
import { checkCurrentPreservation, rustItems, MIGRATION, MIGRATION_SHA256 } from "../coverage/current-preservation";
import { trafficSchemaReference } from "../coverage/schema-adaptations";
import { analyzeSource, hash, tokens, walk } from "../coverage/source";
import { baseline, baselineTree } from "./exec-fail-open-history";
import { requireValue } from "./exec-fail-open-inputs";
const encoded = (s: string) => JSON.stringify(tokens(s).map(t => t.text));
const base = (root: string, path: string) => gitBytes(root, ["show", `${baseline}:${path}`]).toString();
export const routes = [
  { path: "crates/arc-daemon/tests/managed_exec/retries.rs", from: "let mut c = child_command(&f, true);", to: "let mut c = super::historical::before_register(child_command(&f, true), mode);" },
  { path: "crates/arc-daemon/tests/managed_exec/cli.rs", from: 'f.report(); let out = command(&f).args(["exec", "--", "/usr/bin/true"])', to: 'f.report(); let out = super::historical::after_shutdown(command(&f)).args(["exec", "--", "/usr/bin/true"])' },
];
function reverse(path: string, source: string) {
  let ts = tokens(source).map(t => t.text);
  for (const recipe of routes.filter(r => r.path === path)) {
    const from = tokens(recipe.to).map(t => t.text), to = tokens(recipe.from).map(t => t.text);
    const hits = ts.map((_, i) => JSON.stringify(ts.slice(i, i + from.length)) === JSON.stringify(from) ? i : -1).filter(i => i >= 0);
    requireValue(hits.length === 1, `missing/ambiguous fixed adapter: ${path}`);
    ts.splice(hits[0], from.length, ...to);
  }
  // Token reconstruction is only for comparison. Literal bytes stay opaque.
  return ts.join(" ");
}
function contains(expected: string[], actual: string[], path: string) {
  let at = 0;
  for (const item of actual) if (item === expected[at]) at++;
  requireValue(at === expected.length, `accepted test/helper/registry changed: ${path}`);
}
function schema3Reference(path: string, original: string) {
  if (path === "crates/arc-daemon/tests/managed_exec/scenarios.rs") {
    // Preserve the full historical assertion with only the truthful new
    // status and scheduling labels adapted to Traffic.
    for (const [before, after] of [
      [String.raw`State: starting\nRegistered (Unix ms): 100`, String.raw`State: registered\nRegistered (Unix ms): 100`],
      [String.raw`Scheduling: not implemented\n`, String.raw`Scheduling: see job status <JOB_ID> for details\n`],
    ]) {
      requireValue(original.split(before).length === 2, `status baseline mismatch: ${path}`);
      original = original.replace(before, after);
    }
    return original;
  }
  if (["crates/arc-storage/src/migration/tests/acceptance.rs", "crates/arc-storage/tests/nfr010.rs",
    "crates/arc-storage/tests/managed_exec_migration.rs", "crates/arc-daemon/src/database_failure_tests.rs",
    "crates/arc-daemon/src/db_worker/tests.rs", "crates/arc-daemon/src/server_tests.rs",
    "crates/arc-daemon/tests/support/runtime.rs", "crates/arc-daemon/tests/resident/lifecycle.rs",
    "crates/arc-daemon/tests/resident/schema.rs", "crates/arc-daemon/tests/startup.rs"].includes(path))
    return trafficSchemaReference(path, original);
  if (path !== "crates/arc-storage/src/jobs/tests/migration.rs") return original;
  // Only the applied-version and history-count expectations change when the
  // third migration is added; the rest of the historical test stays pinned.
  for (const [before, after] of [
    ["assert_eq!(assess_schema(&c).unwrap().applied_version, Some(2));", "assert_eq!(assess_schema(&c).unwrap().applied_version, Some(3));"],
    ['c.query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r\n            .get::<_, i64>(0))\n            .unwrap(),\n        2',
      'c.query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r\n            .get::<_, i64>(0))\n            .unwrap(),\n        3'],
  ]) {
    requireValue(original.split(before).length === 2, `schema 3 baseline mismatch: ${path}`);
    original = original.replace(before, after);
  }
  return original;
}
const trafficManifests = new Set(["crates/arc-daemon/Cargo.toml", "crates/arc-storage/Cargo.toml", "crates/arc-ipc/Cargo.toml"]);
export function checkHistoricalManifest(path: string, original: string, current: string) {
  if (current === original) return;
  const addition = "arc-traffic.workspace = true\n";
  if (!trafficManifests.has(path)) throw Error(`EXEC_FAIL_OPEN: manifest/migration changed: ${path}`);
  const start = current.indexOf("[dependencies]\n"), at = current.indexOf(addition);
  const nextSection = current.indexOf("\n[", start + 1);
  requireValue(start >= 0 && at > start && current[at - 1] === "\n" && (nextSection < 0 || at < nextSection) &&
    current.indexOf(addition, at + addition.length) < 0 && current.slice(0, at) + current.slice(at + addition.length) === original,
    `manifest/migration changed: ${path}`);
}
export function checkFailOpenPreservation(root: string) {
  const old = checkCurrentPreservation(root);
  requireValue(gitBytes(root, ["rev-parse", `${baseline}^{tree}`]).toString().trim() === baselineTree, "fixed baseline changed");
  const paths = gitBytes(root, ["ls-tree", "-r", "--name-only", baseline, "--", "crates"]).toString().trim().split("\n");
  let checked = 0;
  for (const path of paths) {
    const original = schema3Reference(path, base(root, path)), current = readFileSync(resolve(root, path), "utf8");
    if (path.endsWith("Cargo.toml")) { checkHistoricalManifest(path, original, current); continue; }
    if (path.endsWith(".sql")) {
      requireValue(current === original, `manifest/migration changed: ${path}`); continue;
    }
    if (!path.endsWith(".rs") && !path.endsWith(".py")) continue;
    const normalized = routes.some(r => r.path === path) ? reverse(path, current) : current;
    if (path.includes("/tests/")) {
      // Original Python controllers are kept byte-for-byte; new callers reuse them.
      if (path.endsWith(".py")) requireValue(current === original, `native helper changed: ${path}`);
      else contains(rustItems(original), rustItems(normalized), path);
      checked++; continue;
    }
    const expected = analyzeSource(original), actual = analyzeSource(current);
    const spans = (s: string, info: ReturnType<typeof analyzeSource>) => info.spans.map(span => encoded(s.split("\n").slice(span.start - 1, span.end).join("\n")));
    contains(spans(original, expected), spans(current, actual), path);
    // Existing module declarations (including test roots and cfg attributes)
    // stay in order while production function bodies may change.
    const modules = (s: string) => rustItems(s).filter(item => /^\[.*"mod"/.test(item) && !JSON.parse(item).includes("fn"));
    contains(modules(original), modules(current), path);
  }
  const all = walk(resolve(root, "crates")).filter(p => p.endsWith(".rs"));
  const callerNames = ["super::historical::before_register", "super::historical::after_shutdown"];
  for (let i = 0; i < routes.length; i++) {
    const callers = all.flatMap(p => Array.from(readFileSync(p, "utf8").matchAll(new RegExp(callerNames[i].replaceAll("::", "\\s*::\\s*") + "\\s*\\(", "g"))).map(() => relative(root, p)));
    requireValue(JSON.stringify(callers) === JSON.stringify([routes[i].path]), "broadened historical routing");
  }
  // These new helpers are deliberately frozen independently of the older
  // preservation baseline; their routing and instrumentation rules may not drift.
  const pinned: Record<string, string> = {
    ".arc-verify/exec-fail-open/contract.json": "474fbf2004483dfd56d3858c689dd77674c3dab008d34f8fa28b121dee524ff3",
    ".arc-verify/exec-fail-open/contract.md": "bb25c8603508ef555e786a4c0acc72b43f28e260d24f5b7c0400309d5375d6ed",
    ".arc-verify/exec-fail-open/design-contract.md": "1eb777d89b34d450628d5c707e424ea10be241fb675235676f3e327c803689db",
    "crates/arc-daemon/tests/managed_exec/historical.rs": "3ad844276c8ed77abc7e688f717bb0050be53a26e2bf09e56cdc0ef7b8fbca19",
    "scripts/harness/exec-fail-open-history.ts": "7b0c1cf9cdbe5859fd1835715a84f2d479819f7dc57859e29a8a2eebcca0f868",
    "crates/arc-daemon/tests/managed_exec/fail_open.rs": "5ddc6c705aa6f68f60f126473990073d66e1cc64ec78bc636a798db5605385b4",
    // Includes the bounded nonblocking read fallback for Darwin's EINVAL race.
    "crates/arc-daemon/tests/managed_exec/fault.rs": "5a1021d0dc180798c3fd4d9ba1fb417f0cc345e66b9cb7843eaa74575c5085c1",
    "crates/arc-daemon/tests/managed_exec/fallback_descriptors.rs": "b6ae1eb6bc486a5e2838efb9c24618a50108ddbce6f123c4cdbf1741107167a6",
    "crates/arc-daemon/tests/managed_exec/fallback_terminal.rs": "a3cc34a8c878d1ffe30c42dc757acac58f8634da38a3c61aa817bb5475f062b8",
    "crates/arc-daemon/tests/managed_exec/fallback_pipeline.rs": "788038622ea39f8ffb6bce41f618aa942cb3d5dc3ff630c9f99f48e42b834dde",
    "crates/arc-daemon/tests/managed_exec/fallback_sigstate.rs": "1e90f5157b6d252b73cf0f4bc5d32d33895b95cafb56fb8ffdfc684b672bb7b7",
    "crates/arc-cli/src/exec/transport/tests.rs": "83757a11e06321cd40be4f877a7c6ba8fba0543612317db4037cff97a73940f3",
    "crates/arc-daemon/tests/managed_exec/fallback_environment.rs": "f778416771edac2fcc692f570b92480cb5ea908e25af88b69a21e749e131fd72",
  };
  for (const [path, sha] of Object.entries(pinned)) requireValue(hash(readFileSync(resolve(root, path))) === sha, `historical helper changed: ${path}`);
  const adapter = readFileSync(resolve(root, "crates/arc-daemon/tests/managed_exec/historical.rs"), "utf8");
  const before = adapter.slice(adapter.indexOf("pub fn before_register"), adapter.indexOf("pub fn after_shutdown"));
  requireValue(encoded(before) === encoded('pub fn before_register(command: Command, mode: &str) -> Command { if mode == "before-register" { route(command, "before-register") } else { command } }'), "before-register route broadened");
  requireValue(encoded(adapter.slice(adapter.indexOf("pub fn after_shutdown"), adapter.indexOf("pub fn negative_control_binary"))) === encoded('pub fn after_shutdown(command: Command) -> Command { route(command, "after-shutdown") }'), "shutdown route changed");
  const registry = readFileSync(resolve(root, "crates/arc-daemon/tests/managed_exec.rs"), "utf8");
  for (const name of ["historical", "fault", "fail_open", "fallback_terminal", "fallback_pipeline", "fallback_descriptors", "fallback_sigstate", "fallback_environment"]) {
    requireValue(rustItems(registry).includes(encoded(`#[path = "managed_exec/${name}.rs"] mod ${name};`)), `live registry changed: ${name}`);
  }
  const migration = resolve(root, MIGRATION);
  requireValue(hash(readFileSync(migration)) === MIGRATION_SHA256 && (lstatSync(migration).mode & 0o777) === 0o644, "migration identity changed");
  return { baselineHead: baseline, baselineTree, checked, previousGuard: old.pass, passed: true };
}
