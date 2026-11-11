import { expect, test } from "bun:test";
import { account } from "./accounting";
import { CRATES, HISTORICAL_CRATES } from "./config";
import { fixture } from "./fixtures";

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
