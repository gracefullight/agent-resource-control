import { expect, test } from "bun:test";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { account } from "../../coverage/accounting";
import { BASELINE, CRATES, HISTORICAL_CRATES } from "../../coverage/config";
import { assertionMacroBindings, checkPermitUniqueIndexes, checkProtectedToolingContent, FEATURE_BASELINE, MIGRATION } from "./current-preservation";
import { fixture } from "../../coverage/fixtures";
import { withHistoricalCheckout } from "./historical";
import { checkHistoricalPreservation } from "./preservation";
import { historicalSourceInventory, sourceInventory } from "../../coverage/source";
import { validateFloors } from "../harness/exec-fail-open-gates";
import { gitBytes } from "./hygiene";

test("the current seven-crate gate rejects missing and below-floor Traffic coverage", () => {
  const f = fixture(), traffic = f.inventory.find(file => file.crate === "arc-traffic")!;
  expect([...CRATES]).toEqual([...HISTORICAL_CRATES, "arc-traffic"]);
  expect(account(f.lcov, f.inventory, f.root).crates.at(-1)).toMatchObject({ crate: "arc-traffic", covered: 4, total: 5, pass: true });
  expect(() => account(f.inventory.filter(file => file !== traffic).map(file => f.record(file.path)).join(""), f.inventory, f.root)).toThrow("MISSING_CRATE: arc-traffic");
  const below = f.inventory.map(file => f.record(file.path, file === traffic ? [1, 1, 1, 0, 0] : [1, 1, 1, 1, 1])).join("");
  const result = account(below, f.inventory, f.root);
  expect(result.pass).toBe(false);
  expect(result.crates.at(-1)).toMatchObject({ crate: "arc-traffic", covered: 3, total: 5, pass: false });
});

test("historical fixture floors use six crates while live floors require seven", () => {
  const historical = HISTORICAL_CRATES.map(crate => ({ crate, covered: 80, total: 100 }));
  const current = [...historical, { crate: "arc-traffic", covered: 80, total: 100 }];
  expect(() => validateFloors(historical, "historical-fixture")).not.toThrow();
  expect(() => validateFloors(current)).not.toThrow();
  expect(() => validateFloors(historical)).toThrow("missing");
  expect(() => validateFloors(current, "historical-fixture")).toThrow("missing");
  expect(() => validateFloors([...historical, { crate: "arc-traffic", covered: 79, total: 100 }])).toThrow("below 80");
  expect(() => validateFloors(historical.map((c, i) => i === 0 ? { ...c, covered: 79 } : c), "historical-fixture")).toThrow("below 80");
  expect(() => validateFloors(historical.slice(1), "historical-fixture")).toThrow("missing");
});

test("fixed current adaptations retain all other protected tooling bytes", () => {
  const root = resolve(import.meta.dir, "../../..");
  for (const path of ["scripts/coverage/config.ts", "scripts/coverage/source.ts", "scripts/coverage/preservation.ts",
    "scripts/coverage/accounting.test.ts", ".arc-verify/rust-coverage/contract/accounting.json",
    "scripts/coverage/accounting.ts", "scripts/coverage/commands.ts", "scripts/harness/validate.sh"]) {
    const bytes = readFileSync(resolve(root, path));
    expect(checkProtectedToolingContent(root, path, bytes)).toHaveLength(64);
    expect(() => checkProtectedToolingContent(root, path, Buffer.concat([bytes, Buffer.from("\n// altered\n")]))).toThrow("protected bytes changed");
  }
});

test("forward migrations preserve the original permit uniqueness constraints", () => {
  const root = resolve(import.meta.dir, "../../..");
  const baseline = gitBytes(root, ["show", `${FEATURE_BASELINE}:${MIGRATION}`]).toString();
  const migrations = ["0001_phase0.sql", "0002_managed_exec.sql", "0003_traffic.sql"]
    .map(path => readFileSync(resolve(root, "crates/arc-storage/migrations", path), "utf8"));
  expect(() => checkPermitUniqueIndexes(baseline, migrations)).not.toThrow();
  // Keep the same column names and foreign key while moving UNIQUE from job_id
  // to state: the old columns-only comparison could not distinguish this.
  const altered = `DROP TABLE permits;
    CREATE TABLE permits (id TEXT PRIMARY KEY, job_id TEXT NOT NULL REFERENCES jobs(id),
      state TEXT NOT NULL UNIQUE, cpu_cores_est REAL, memory_bytes_est INTEGER,
      reserved_at INTEGER NOT NULL, released_at INTEGER, cpu_millis_est INTEGER, generation INTEGER);`;
  expect(() => checkPermitUniqueIndexes(baseline, [...migrations, altered])).toThrow("permits unique index changed or removed");
});

test("new test modules allow ordinary inner cfg while still rejecting assertion bindings", () => {
  expect(assertionMacroBindings('#![cfg(target_os = "macos")]\nuse super::*;\n', true)).toBe("[]");
  expect(assertionMacroBindings('#![cfg(target_os = "macos")]\n')).not.toBe("[]");
  expect(assertionMacroBindings("#![macro_use] extern crate fixture;", true)).not.toBe("[]");
  expect(assertionMacroBindings("use unrelated::*;", true)).not.toBe("[]");
  expect(assertionMacroBindings("macro_rules! assert { ($x:expr) => { () } }", true)).not.toBe("[]");
  const cleanupBuffer = "thread_local! { static UNWIND_ERRORS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) }; }";
  expect(assertionMacroBindings(cleanupBuffer, true)).toBe("[]");
  expect(assertionMacroBindings(cleanupBuffer)).not.toBe("[]");
  expect(assertionMacroBindings(cleanupBuffer.replace("UNWIND_ERRORS", "assert"), true)).not.toBe("[]");
  expect(assertionMacroBindings("use std::panic::assert;", true)).not.toBe("[]");
  expect(assertionMacroBindings("use std::assert;", true)).not.toBe("[]");
});

test("historical six-crate checks stay bound to the immutable baseline", () => {
  const root = resolve(import.meta.dir, "../../..");
  withHistoricalCheckout(root, BASELINE, historical => {
    expect(new Set(historicalSourceInventory(historical).map(file => file.crate))).toEqual(new Set(HISTORICAL_CRATES));
    expect(() => sourceInventory(historical)).toThrow();
    expect(checkHistoricalPreservation(historical).pass).toBe(true);
    const path = resolve(historical, "crates/arc-core/src/lib.rs");
    const original = readFileSync(path, "utf8");
    writeFileSync(path, original.replace("pub ", ""));
    expect(() => checkHistoricalPreservation(historical)).toThrow("PRESERVATION: existing Rust tokens changed or removed");
  });
}, 7200000);
