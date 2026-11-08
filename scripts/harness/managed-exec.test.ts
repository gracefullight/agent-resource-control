import { test, expect } from "bun:test";
import { cpSync, mkdirSync, readFileSync, writeFileSync, chmodSync, unlinkSync, symlinkSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { root } from "../coverage/acceptance";
import { withHistoricalCheckout } from "../coverage/historical";
import { checkHistoricalPreservation } from "../coverage/preservation";
import { checkCurrentPreservation, FEATURE_BASELINE, MIGRATION } from "../coverage/current-preservation";
import { BASELINE } from "../coverage/config";
import { gitBytes } from "../coverage/hygiene";
import { schemaReference, schemaAdaptationPaths } from "../coverage/schema-adaptations";

function fixture(run: (dir: string) => void) {
  withHistoricalCheckout(root, FEATURE_BASELINE, dir => {
    for (const path of ["scripts/coverage", "scripts/harness", ".arc-verify/rust-coverage/contract/accounting.json"]) {
      mkdirSync(dirname(resolve(dir, path)), { recursive: true });
      cpSync(resolve(root, path), resolve(dir, path), { recursive: true });
    }
    for (const path of schemaAdaptationPaths) {
      const file = resolve(dir, path);
      writeFileSync(file, schemaReference(path, readFileSync(file, "utf8")));
    }
    run(dir);
  });
}
function reject(dir: string, path: string, change: (source: string) => string, message: string) {
  const file = resolve(dir, path), before = readFileSync(file, "utf8"), after = change(before);
  expect(after).not.toBe(before); writeFileSync(file, after);
  try { expect(() => checkCurrentPreservation(dir)).toThrow(message); }
  finally { writeFileSync(file, before); }
}
test("live checkout preserves migration, regressions, accounting, harness and source bounds", () => {
  expect(checkCurrentPreservation(root)).toMatchObject({ baselineHead: FEATURE_BASELINE, pass: true });
}, 3600000);
test("authorized production change and lifecycle addition pass while historical token checks reject changes", () => {
  fixture(dir => {
    const file = resolve(dir, "crates/arc-core/src/lib.rs");
    writeFileSync(file, readFileSync(file, "utf8").replace('"ARC_DB_PATH"', '"ARC_DB_PATH_AUTHORIZED_FIXTURE"') +
      '\nmod lifecycle_fixture;\n');
    writeFileSync(resolve(dir, "crates/arc-core/src/lifecycle_fixture.rs"),
      'macro_rules! lifecycle_value { () => { 17 }; }\npub fn lifecycle_fixture() -> u8 { lifecycle_value!() }\n' +
      'const NOTE: &str = "macro_rules! assert { unchecked } #[macro_use]";\n');
    expect(checkCurrentPreservation(dir).pass).toBe(true);
  });
  withHistoricalCheckout(root, BASELINE, dir => {
    expect(checkHistoricalPreservation(dir).pass).toBe(true);
    const file = resolve(dir, "crates/arc-core/src/lib.rs");
    writeFileSync(file, readFileSync(file, "utf8").replace('"ARC_DB_PATH"', '"changed"'));
    expect(() => checkHistoricalPreservation(dir)).toThrow("existing Rust tokens changed");
  });
}, 3600000);
test("exact schema-2 adaptations pass and fixed schema-1 witness semantics remain protected", () => {
  fixture(dir => {
    const text = (path: string) => readFileSync(resolve(dir, path), "utf8");
    const acceptance = text("crates/arc-storage/src/migration/tests/acceptance.rs");
    expect(acceptance).toContain("assert_eq!(report.status, SchemaStatus::UpgradeRequired);");
    expect(acceptance).toContain("assert_eq!(stamped.witness, 2);");
    expect(acceptance).toContain("fn fixed_schema1_registry_stamps_legacy_witness_without_upgrading()");
    expect(acceptance).toContain("assert_eq!(stamped.witness, 1);");
    expect(acceptance).toContain("assert_eq!(history(&connection)[0], original_history[0]);");
    const nfr = text("crates/arc-storage/tests/nfr010.rs");
    expect(nfr).toContain('"job_lifecycle",');
    expect(nfr).toContain("assert_eq!(count, 2);");
    expect(nfr).toContain("WHERE id='0001_phase0'");
    expect(nfr).toContain("WHERE id='0002_managed_exec'");
    expect(nfr).toContain("assert_eq!(recorded_second, second);");
    for (const path of ["crates/arc-daemon/tests/startup.rs", "crates/arc-daemon/tests/resident/schema.rs"]) {
      expect(text(path)).toContain("SET id='0001_other' WHERE id='0001_phase0'");
      expect(text(path)).toContain("'0003_future'");
      expect(text(path)).toContain("PRAGMA user_version=3");
    }
    expect(text("crates/arc-daemon/tests/startup.rs")).toContain("version 3 (supported 2)");
    expect(checkCurrentPreservation(dir).pass).toBe(true);
    // Recipes cannot silently operate on current or arbitrary alternate input.
    expect(() => schemaReference("crates/arc-storage/tests/nfr010.rs", nfr)).toThrow("SCHEMA_REFERENCE");
    expect(schemaReference("unassigned.rs", "literal 1")).toBe("literal 1");
  });
}, 3600000);
test("schema guards reject wrong counts, inventories, checksums and legacy-witness changes", () => {
  fixture(dir => {
    for (const [path, from, to] of [
      ["crates/arc-storage/tests/nfr010.rs", "assert_eq!(count, 2);", "assert_eq!(count, 1);"],
      ["crates/arc-storage/tests/nfr010.rs", '        "job_lifecycle",\n', ""],
      ["crates/arc-storage/tests/nfr010.rs", '        "permits",\n', ""],
      ["crates/arc-storage/tests/nfr010.rs", "046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384", "0".repeat(64)],
      ["crates/arc-storage/tests/nfr010.rs", "assert_eq!(recorded_second, second);", ""],
      ["crates/arc-storage/src/migration/tests/acceptance.rs", "assert_eq!(stamped.witness, 1);", "assert_eq!(stamped.witness, 2);"],
      ["crates/arc-storage/src/migration/tests/acceptance.rs", "assert_eq!(stamped.witness, 2);", "assert_eq!(stamped.witness, 1);"],
      ["crates/arc-storage/src/migration/tests/acceptance.rs", "assert_eq!(report.supported_version, 1);", "assert_eq!(report.supported_version, 2);"],
      ["crates/arc-storage/src/migration/tests/acceptance.rs", "assert_eq!(stamped.schema, legacy.schema);", ""],
      ["crates/arc-daemon/src/db_worker/tests.rs", "assert_eq!(recover(&client).await.schema_migrations, 2);", "assert_eq!(recover(&client).await.schema_migrations, 1);"],
      ["crates/arc-daemon/src/server_tests.rs", 'assert_eq!(result["schema_migrations"], 2);', 'assert_eq!(result["schema_migrations"], 1);'],
      ["crates/arc-daemon/tests/support/runtime.rs", "Version 2.", "Version 1."],
    ]) reject(dir, path, s => s.replace(from, to), "REGRESSION");
  });
}, 3600000);
test("adapted schema tests cannot be disabled, return early or omit rejection snapshots", () => {
  fixture(dir => {
    for (const path of schemaAdaptationPaths) {
      reject(dir, path, s => s.replace("assert_eq!", "assert_ne!"), "REGRESSION");
      reject(dir, path, s => s.replace(/(fn \w+\([^)]*\)[^{]*\{)/, "$1\nreturn;"), "REGRESSION");
    }
    for (const path of ["crates/arc-storage/src/migration/tests/acceptance.rs",
      "crates/arc-storage/tests/nfr010.rs", "crates/arc-daemon/tests/startup.rs", "crates/arc-daemon/tests/resident/schema.rs"]) {
      reject(dir, path, s => s.replace("#[test]", "#[test]\n#[ignore]"), "REGRESSION");
      reject(dir, path, s => '#![cfg(any())]\n' + s, "REGRESSION");
    }
    for (const [path, from, to] of [
      ["crates/arc-daemon/tests/startup.rs", "assert_eq!(database_snapshot(&connection), before);", ""],
      ["crates/arc-daemon/tests/resident/schema.rs", "assert_eq!(snapshot(&connection), before);", ""],
      ["crates/arc-daemon/tests/startup.rs", "SET id='0001_other' WHERE id='0001_phase0'", "SET id='0001_other'"],
      ["crates/arc-daemon/tests/resident/schema.rs", "PRAGMA user_version=3", "PRAGMA user_version=2"],
    ]) reject(dir, path, s => s.replace(from, to), "REGRESSION");
  });
}, 3600000);
test("external macro imports and new nested modules cannot shadow preserved assertions", () => {
  fixture(dir => {
    const core = "crates/arc-core/src/lib.rs", shadow = resolve(dir, "crates/arc-core/src/guard_shadow.rs");
    writeFileSync(shadow, 'macro_rules! assert { ($($t:tt)*) => {}; }\nmacro_rules! assert_eq { ($($t:tt)*) => {}; }\n');
    for (const attr of ["#[macro_use]", "#[macro_use(assert, assert_eq)]", "#[cfg_attr(test, macro_use)]"])
      reject(dir, core, s => `${attr}\nmod guard_shadow;\n${s}`, "REGRESSION: assertion macro");
    // Scan the new file itself, including nested declarations and aliases, even
    // when the old file has no direct assertion definitions/imports to compare.
    for (const source of [
      'mod nested { macro_rules! assert_eq { ($($t:tt)*) => {}; } }\n',
      'macro_rules! r#assert_eq { ($($t:tt)*) => {}; }\n',
      'use foreign_macros::unchecked as assert_eq;\n',
      '#[macro_use]\nextern crate foreign_macros;\n',
    ]) {
      writeFileSync(shadow, source);
      reject(dir, core, s => `mod guard_shadow;\n${s}`, "REGRESSION: assertion macro");
    }
    unlinkSync(shadow);
  });
}, 3600000);
test("generated assertion factory and wildcard imports fail conservatively", () => {
  fixture(dir => {
    const core = "crates/arc-core/src/lib.rs";
    const factory = 'macro_rules! install_guard { ($name:ident, $d:tt) => { macro_rules! $name { ($d left:expr, $d right:expr) => { let _ = (&$d left, &$d right); }; } }; }\ninstall_guard!(assert_eq, $);\n';
    reject(dir, core, s => factory + s, "REGRESSION: assertion macro");
    for (const item of ['use foreign_macros::*;', 'use foreign::{macros::*, safe};',
      'mod nested { use foreign::{safe, macros::{self, *}}; }'])
      reject(dir, core, s => item + '\n' + s, "REGRESSION: assertion macro");
    const added = resolve(dir, "crates/arc-core/src/guard_factory.rs");
    for (const source of [factory, `mod nested { ${factory} }`, 'use foreign::{macros::*, safe};']) {
      writeFileSync(added, source);
      reject(dir, core, s => 'mod guard_factory;\n' + s, "REGRESSION: assertion macro");
    }
    unlinkSync(added);
  });
}, 3600000);
test("unresolved item macro expansions cannot introduce preserved-test bindings", () => {
  fixture(dir => {
    const core = "crates/arc-core/src/lib.rs", include = resolve(dir, "crates/arc-core/src/guard_shadow.inc");
    writeFileSync(include, 'macro_rules! assert_eq { ($($t:tt)*) => { let _ = stringify!($($t)*); }; }\n');
    for (const item of ['include!("guard_shadow.inc");', 'foreign::install_guard!();',
      'mod nested { include!("guard_shadow.inc"); }'])
      reject(dir, core, s => s.replace('\n', `\n${item}\n`), "REGRESSION: assertion macro");
    unlinkSync(include);
  });
}, 3600000);
test("regression checks reject changed assertions, early returns, ignored/cfg-disabled tests and helper redirection", () => {
  fixture(dir => {
    reject(dir, "crates/arc-cli/Cargo.toml", s => s.replace('[package]', '[package]\nautotests = false'), "REGRESSION");
    reject(dir, "crates/arc-cli/Cargo.toml", s => s + '\n[[test]]\nname = "binary"\npath = "tests/other.rs"\n', "REGRESSION");
    const binary = "crates/arc-cli/tests/binary.rs";
    reject(dir, binary, s => s.replace('assert!(Path::new(path).is_file(), "missing arcctl binary at {path}");',
      'assert!(true, "missing arcctl binary at {path}");'), "REGRESSION");
    reject(dir, binary, s => s.replace('let path = env!', 'return; let path = env!'), "REGRESSION");
    reject(dir, binary, s => s.replace('#[test]', '#[test]\n#[ignore]'), "REGRESSION");
    reject(dir, binary, s => '#![cfg(any())]\n' + s, "REGRESSION");
    reject(dir, binary, s => 'macro_rules! assert { ($($t:tt)*) => {}; }\n' + s, "REGRESSION");
    const core = "crates/arc-core/src/lib.rs";
    reject(dir, core, s => s.replace('length.min(value.len())', '0'), "REGRESSION");
    reject(dir, core, s => s.replace('mod tests {', '#[cfg(any())]\nmod tests {'), "REGRESSION");
    reject(dir, core, s => s.replace('reply(buffer, length, value)', 'reply(buffer, length, b"other")'), "REGRESSION");
  });
}, 3600000);
test("production roots and enclosing modules cannot disable or hide existing tests", () => {
  fixture(dir => {
    for (const path of ["crates/arc-core/src/lib.rs", "crates/arc-daemon/src/main.rs", "crates/arc-cli/src/main.rs"]) {
      for (const attr of ["#![cfg(not(test))]", "#![cfg(any())]", "#![cfg_attr(test, cfg(any()))]",
        "#![cfg_attr(all(test, target_os = \"macos\"), cfg(not(test)))]"])
        reject(dir, path, s => attr + "\n" + s, "REGRESSION");
    }
    const core = "crates/arc-core/src/lib.rs";
    reject(dir, core, s => s.replace('mod tests {', 'mod tests {\n#![cfg(not(test))]'), "REGRESSION");
    reject(dir, core, s => s.replace('mod tests {', 'mod tests {\n#![cfg_attr(test, cfg(any()))]'), "REGRESSION");
    reject(dir, core, s => s.replace('#[cfg(all(test, target_os = "macos"))]',
      '#[cfg(not(test))]\nmod hidden {\n#[cfg(all(test, target_os = "macos"))]') + '\n}\n', "REGRESSION");
    reject(dir, core, s => s.replace('#[cfg(all(test, target_os = "macos"))]',
      'fn hidden() {\n#[cfg(all(test, target_os = "macos"))]') + '\n}\n', "REGRESSION");
    reject(dir, "crates/arc-daemon/src/main.rs", s => s.replace('mod db_worker;',
      '#[cfg_attr(test, cfg(any()))]\nmod db_worker;'), "REGRESSION");
  });
}, 3600000);
test("migration byte, index, mode and symlink changes are rejected", () => {
  fixture(dir => {
    reject(dir, MIGRATION, s => s + "\n", "protected bytes changed");
    const file = resolve(dir, MIGRATION), bytes = readFileSync(file);
    writeFileSync(file, Buffer.concat([bytes, Buffer.from("\n")]));
    gitBytes(dir, ["add", "--", MIGRATION]); writeFileSync(file, bytes);
    expect(() => checkCurrentPreservation(dir)).toThrow("index migration");
    gitBytes(dir, ["read-tree", FEATURE_BASELINE]);
    chmodSync(file, 0o755);
    expect(() => checkCurrentPreservation(dir)).toThrow("migration executable");
    chmodSync(file, 0o644); unlinkSync(file); symlinkSync("missing", file);
    expect(() => checkCurrentPreservation(dir)).toThrow();
  });
}, 3600000);
test("coverage denominator, exclusions, profile/PID merge, reports and harness bypasses fail", () => {
  fixture(dir => {
    for (const [path, from, to] of [
      ["scripts/coverage/accounting.ts", "BigInt(total)*80n", "BigInt(total)*1n"],
      ["scripts/coverage/accounting.ts", "if(file.testOnly) continue;", "if(file.testOnly || file.crate === \"arc-core\") continue;"],
      ["scripts/coverage/source.ts", "return undefined;", "return false;"],
      ["scripts/coverage/config.ts", '\"arc-cli\"', '\"omitted-crate\"'],
      ["scripts/coverage/run.ts", '"--disable-default-ignore-filename-regex",', '"--ignore-filename-regex", "crates",'],
      ["scripts/coverage/run.ts", 'if(JSON.stringify(merge)!==JSON.stringify(profiles.map(p=>join(target,p)).sort()))', 'if(false)'],
      ["scripts/coverage/run.ts", 'if(!profiles.length)', 'if(false)'],
      ["scripts/coverage/gate.ts", 'statSync(path).mtimeMs<started', 'false'],
      ["scripts/coverage/evidence.test.ts", 'expect(report.crates.map((x:any)=>x.crate)).toEqual([...CRATES]);', 'expect(true).toBe(true);'],
      ["scripts/harness/equivalence.test.ts", 'expect(failed).toBe(false);', 'expect(true).toBe(true);'],
      ["scripts/harness/coverage-verification.test.ts", '"accounting.test.ts",', ''],
    ]) reject(dir, path, s => s.replace(from, to), "protected bytes changed");
    const harness = "scripts/harness/validate.sh";
    for (const change of [
      (s: string) => s.replace('set -e', 'exit 0\nset -e'),
      (s: string) => s.replace('if ! bun run test:coverage; then', 'if ! true; then'),
      (s: string) => s.replace('MAX_LINES=500', 'MAX_LINES=5000'),
      (s: string) => s.replace('set -e', 'export SKIP_COVERAGE=1\nset -e'),
      (s: string) => s.replace('printf "\\n"', 'printf "changed\\n"'),
    ]) reject(dir, harness, change, "protected bytes changed");
    chmodSync(resolve(dir, harness), 0o644);
    expect(() => checkCurrentPreservation(dir)).toThrow("local bytes/mode changed");
  });
}, 3600000);
