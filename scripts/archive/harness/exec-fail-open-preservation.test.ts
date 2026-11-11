import { test, expect } from "bun:test";
import { readFileSync, writeFileSync, cpSync, mkdirSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { withHistoricalCheckout } from "../coverage/historical";
import { root } from "./exec-fail-open-inputs";
import { baseline } from "./exec-fail-open-history";
import { checkFailOpenPreservation, checkHistoricalManifest } from "./exec-fail-open-preservation";
import { gitBytes } from "../coverage/hygiene";

test("only the three exact Traffic workspace dependencies adapt historical manifests", () => {
  for (const crate of ["arc-daemon", "arc-storage", "arc-ipc"]) {
    const path = `crates/${crate}/Cargo.toml`;
    const original = gitBytes(root, ["show", `${baseline}:${path}`]).toString();
    const current = readFileSync(resolve(root, path), "utf8");
    expect(() => checkHistoricalManifest(path, original, current)).not.toThrow();
    expect(() => checkHistoricalManifest(path, original, current.replace("version.workspace = true", "version = \"9.0.0\""))).toThrow("manifest/migration changed");
    expect(() => checkHistoricalManifest(path, original, current.replace("arc-traffic.workspace = true", "arc-traffic = { workspace = true, optional = true }"))).toThrow("manifest/migration changed");
    expect(() => checkHistoricalManifest(path, original, current + "arc-traffic.workspace = true\n")).toThrow("manifest/migration changed");
  }
  const path = "crates/arc-core/Cargo.toml", original = gitBytes(root, ["show", `${baseline}:${path}`]).toString();
  expect(() => checkHistoricalManifest(path, original, original.replace("[dependencies]\n", "[dependencies]\narc-traffic.workspace = true\n"))).toThrow("manifest/migration changed");
});
function fixture(run: (dir: string) => void) {
  withHistoricalCheckout(root, baseline, dir => {
    for (const path of ["crates", "scripts/coverage", "scripts/harness", ".arc-verify/rust-coverage/contract/accounting.json", ".arc-verify/exec-fail-open/contract.json", ".arc-verify/exec-fail-open/contract.md", ".arc-verify/exec-fail-open/design-contract.md"]) {
      mkdirSync(dirname(resolve(dir, path)), { recursive: true }); cpSync(resolve(root, path), resolve(dir, path), { recursive: true });
    }
    expect(checkFailOpenPreservation(dir).passed).toBe(true);
    run(dir);
    expect(checkFailOpenPreservation(dir).passed).toBe(true);
  });
}
test("accepted tests, two exact historical recipes, migration, accounting and 500-line bounds remain protected", () => {
  expect(checkFailOpenPreservation(root).passed).toBe(true);
}, 3600000);
test("deletion, disabled assertions, widened historical routes and accounting/migration tampering fail", () => fixture(dir => {
  for (const [path, from, to] of [
    ["crates/arc-daemon/tests/managed_exec/retries.rs", "assert!(!marker.exists());", ""],
    ["crates/arc-daemon/tests/managed_exec/cli.rs", "#[test]", "#[test]\n#[ignore]"],
    ["crates/arc-daemon/tests/managed_exec/retries.rs", 'mode == "before-register"', 'true'],
    ["crates/arc-daemon/tests/managed_exec/historical.rs", 'mode == "before-register"', 'mode != "never"'],
    ["crates/arc-daemon/tests/managed_exec.rs", 'mod fail_open;', '#[cfg(any())]\nmod fail_open;'],
    ["crates/arc-storage/migrations/0001_phase0.sql", "CREATE", "--changed\nCREATE"],
    ["crates/arc-storage/src/migration/tests/acceptance.rs", "original_permit_columns.len() + 2", "original_permit_columns.len() + 0"],
    ["crates/arc-storage/src/migration/tests/acceptance.rs", 'if object.1 != "permits"', 'if true'],
    ["crates/arc-storage/tests/nfr010.rs", "assert_eq!(count, 3);", "assert_eq!(count, 2);"],
    ["crates/arc-storage/src/jobs/tests/migration.rs", "applied_version, Some(3)", "applied_version, Some(2)"],
    ["crates/arc-storage/tests/managed_exec_migration.rs", 'assert_eq!(MIGRATIONS[2].id, "0003_traffic");', ""],
    ["crates/arc-storage/src/jobs/traffic/tests/admission.rs", "use super::*;", "use super::*;\nuse std::assert;"],
    ["scripts/coverage/accounting.ts", "BigInt(total)*80n", "BigInt(total)*1n"],
    ["crates/arc-daemon/tests/managed_exec/fail_open.rs", "child_command(&f, wrapped)", "std::process::Command::new(super::historical::negative_control_binary())"],
    [".arc-verify/exec-fail-open/contract.json", "00773c69a8e5e7fd417fd5cc78d03e222364f609", "19d7a507044cbf83918f58cd41056905c275c8d4"],
    ["scripts/harness/exec-fail-open-history.ts", "00773c69a8e5e7fd417fd5cc78d03e222364f609", "19d7a507044cbf83918f58cd41056905c275c8d4"],
    ["crates/arc-daemon/tests/managed_exec/historical.rs", "Command::new(binary())", 'Command::new("arcctl")'],
  ]) {
    const file = resolve(dir, path), original = readFileSync(file, "utf8"), altered = original.replace(from, to);
    expect(altered).not.toBe(original); writeFileSync(file, altered);
    try { expect(() => checkFailOpenPreservation(dir)).toThrow(); }
    finally {
      writeFileSync(file, original);
      expect(checkFailOpenPreservation(dir).passed).toBe(true);
    }
    console.info(`preservation mutation rejected and restored: ${path}`);
  }
}), 3600000);
test("complete-environment comparison mutation identifies fallback_environment.rs and exact restoration passes", () => fixture(dir => {
  const path = "crates/arc-daemon/tests/managed_exec/fallback_environment.rs";
  const file = resolve(dir, path), original = readFileSync(file);
  const altered = original.toString().replace(
    'assert_eq!(expected, actual, "complete native environment changed");',
    'assert_eq!(expected, expected, "complete native environment changed");',
  );
  expect(altered).not.toBe(original.toString());
  writeFileSync(file, altered);
  try {
    expect(() => checkFailOpenPreservation(dir)).toThrow(`EXEC_FAIL_OPEN: historical helper changed: ${path}`);
  } finally {
    writeFileSync(file, original);
    expect(readFileSync(file).equals(original)).toBe(true);
  }
  console.info(`complete-environment mutation rejected with fixture-specific diagnostic; exact bytes restored: ${path}`);
}), 3600000);
test("terminal receipt mutation is rejected and exact restoration passes", () => fixture(dir => {
  const path = "crates/arc-daemon/tests/managed_exec/fallback_terminal.rs";
  const file = resolve(dir, path), original = readFileSync(file);
  const altered = original.toString().replace('"terminal-2",\n        serde_json::json!({"scenario":"terminal","passed":true,"invocations":0})', '"terminal-2",\n        serde_json::json!({"scenario":"terminal","passed":true,"invocations":1})');
  expect(altered).not.toBe(original.toString());
  writeFileSync(file, altered);
  try { expect(() => checkFailOpenPreservation(dir)).toThrow(`EXEC_FAIL_OPEN: historical helper changed: ${path}`); }
  finally { writeFileSync(file, original); expect(readFileSync(file).equals(original)).toBe(true); }
}), 3600000);
