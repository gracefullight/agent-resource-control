import { test, expect } from "bun:test";
import { readFileSync, writeFileSync, cpSync, mkdirSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { withHistoricalCheckout } from "../coverage/historical";
import { root } from "./exec-fail-open-inputs";
import { baseline } from "./exec-fail-open-history";
import { checkFailOpenPreservation } from "./exec-fail-open-preservation";
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
