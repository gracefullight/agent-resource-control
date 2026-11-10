import { test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { root, directory } from "./exec-fail-open-inputs";
import { checkDocs } from "./exec-fail-open-docs";
const input = () => ({ main: readFileSync(resolve(root, "crates/arc-cli/src/main.rs"), "utf8"),
  doc: readFileSync(resolve(root, "docs/managed-exec.md"), "utf8"), exec: readFileSync(resolve(root, "crates/arc-cli/src/exec.rs"), "utf8"),
  copy: readFileSync(resolve(root, "crates/arc-cli/src/exec/preparation.rs"), "utf8"), claims: readFileSync(resolve(root, directory, "claims-review.md"), "utf8") });
test("help, fallback diagnostics, native semantics and unmeasured scope match actual implementation", () => {
  expect(checkDocs(root).passed).toBe(true);
});
test("stale prerequisites, persistence promises, secret diagnostics and missing scope are rejected", () => {
  const mutations: [(x: ReturnType<typeof input>) => void, string][] = [
    [(x) => { x.main = x.main.replace("still attempts", "never attempts"); }, "help differs"],
    [(x) => { x.main = x.main.replace("If the daemon is unavailable or tracking is unconfirmed due to an outage or exhausted DB_BUSY retries, ARC still attempts the command once.", "If the daemon is unavailable or tracking cannot be confirmed, ARC still attempts the command once."); }, "help differs"],
    [(x) => { x.main = x.main.replace("invalid protocol replies, local preparation failures and ", ""); }, "help differs"],
    [(x) => { x.main = x.main.replace("invalid protocol replies, ", ""); }, "help differs"],
    [(x) => { x.main = x.main.replace("local preparation failures and ", ""); }, "help differs"],
    [(x) => { x.doc += "\nRegistration must be confirmed before spawn."; }, "stale prerequisite or persistence claim"],
    [(x) => { x.doc += "\nThe record is saved."; }, "stale prerequisite or persistence claim"],
    [(x) => { x.copy += 'eprintln!("{argv:?}");'; }, "secret-bearing wrapper diagnostic"],
    [(x) => { x.claims = x.claims.replaceAll("vendor hooks", "hooks"); }, "outstanding scope missing vendor hooks"],
  ];
  for (const [mutate, expected] of mutations) {
    const value = input(), before = JSON.stringify(value);
    expect(checkDocs(root, value).passed).toBe(true);
    mutate(value);
    expect(JSON.stringify(value)).not.toBe(before);
    expect(() => checkDocs(root, value)).toThrow(expected);
  }
});
