import "./exec-fail-open-compact.test";
import "./exec-fail-open-compact-flow.test";
import { test, expect } from "bun:test";
import { readFileSync, writeFileSync, symlinkSync } from "node:fs";
import { resolve } from "node:path";
import { root, context, canonicalBytes, validateIndex, validateCommon, reference, directory } from "./exec-fail-open-inputs";
import { validateEvidence, validateReviewOutcome, validateVerdict } from "./exec-fail-open-evidence";
import { fixtureFirstRun } from "./exec-fail-open-fixture";
test("sealed current evidence binds independent QA and final decision without requiring a terminal sink", () => {
  expect(validateEvidence(root).passed).toBe(true);
}, 3600000);
test("fixture first pass validates gates and final before real terminal observations and preserves all predecessors", () => {
  fixtureFirstRun((dir, refs, terminal) => {
    expect(terminal.gates.exit).toBe(0); expect(terminal.evidence.exit).toBe(0);
    expect(Object.keys(refs)).toHaveLength(9);
    expect(terminal.assembly.exit).toBe(0);
    expect(() => validateEvidence(dir)).toThrow();
    expect(() => validateEvidence(dir, { ...terminal.recipe, command: (r: any, ctx: any, actor?: string) => { if (actor === "reviewer") canonicalBytes(dir, `${directory}/terminal.json`); return terminal.recipe.command(r, ctx, actor); } })).toThrow("attempted successor read");
    const ctx = context(root);
    expect(() => validateCommon({ ...ctx, kind: "fixture-only" }, ctx, "final-decision")).toThrow();
    expect(() => validateCommon({ ...ctx, kind: "final-decision", fixtureOnly: true }, ctx, "final-decision")).toThrow("fixture evidence is inadmissible");
    expect(() => canonicalBytes(dir, "../inputs.json")).toThrow();
    symlinkSync(resolve(dir, "inputs.json"), resolve(dir, "link.json"));
    expect(() => canonicalBytes(dir, "link.json")).toThrow();
    const original = reference(dir, `${directory}/implementation/gates.json`); writeFileSync(resolve(dir, `${directory}/implementation/gates.json`), "altered");
    expect(reference(dir, `${directory}/implementation/gates.json`).sha256).not.toBe(original.sha256);
    expect(() => terminal.validate("final")).toThrow();
    expect(() => validateIndex(root, [".arc-verify/exec-fail-open/missing-required.json"])).toThrow();
  });
});

test("review and final outcome predicates reject self-review, open findings and false verdicts", () => {
  const qa = { actorId: "reviewer", skill: "oma-qa", status: "passed", openBlockers: 0, productFilesEdited: false, implementationTestsEdited: false, findings: [] };
  expect(() => validateReviewOutcome(qa)).not.toThrow();
  for (const patch of [{ actorId: "backend" }, { productFilesEdited: true }, { implementationTestsEdited: true }, { openBlockers: 1 }, { status: "blocked" }, { findings: [{ path: "crates/a.rs", evidence: "observed", disposition: "open" }] }]) expect(() => validateReviewOutcome({ ...qa, ...patch })).toThrow();
  expect(() => validateVerdict({ verdict: "pass", openBlockers: 0 })).not.toThrow();
  for (const final of [{ verdict: "pass", openBlockers: 1 }, { verdict: "fail", openBlockers: 0 }, { verdict: "blocked" }]) expect(() => validateVerdict(final)).toThrow();
});

test("fixture index enforces sealed bytes, regular files and the reserved terminal slot below 150", () => {
  const { mkdtempSync, mkdirSync, rmSync } = require("node:fs") as typeof import("node:fs");
  const { tmpdir } = require("node:os") as typeof import("node:os");
  const { spawnSync } = require("node:child_process") as typeof import("node:child_process");
  const dir = mkdtempSync(resolve(tmpdir(), "arc-sealed-index-"));
  const git = (...args: string[]) => { const r = spawnSync("git", args, { cwd: dir, encoding: "utf8" }); expect(r.status).toBe(0); };
  try {
    git("init", "--quiet"); mkdirSync(resolve(dir, ".arc-verify/exec-fail-open"), { recursive: true });
    for (let i = 0; i < 148; i++) writeFileSync(resolve(dir, `.arc-verify/exec-fail-open/${i}.json`), "{}\n");
    git("add", ".arc-verify/exec-fail-open");
    expect(validateIndex(dir, [".arc-verify/exec-fail-open/0.json"], 148)).toBe(148);
    writeFileSync(resolve(dir, ".arc-verify/exec-fail-open/terminal.json"), "{}\n"); git("add", ".arc-verify/exec-fail-open/terminal.json");
    expect(validateIndex(dir, [], 148)).toBe(149);
    writeFileSync(resolve(dir, ".arc-verify/exec-fail-open/0.json"), '{"changed":true}\n');
    expect(() => validateIndex(dir, [], 148)).toThrow("index bytes");
    writeFileSync(resolve(dir, ".arc-verify/exec-fail-open/0.json"), "{}\n");
    writeFileSync(resolve(dir, ".arc-verify/exec-fail-open/extra.json"), "{}\n"); git("add", ".arc-verify/exec-fail-open/extra.json");
    expect(() => validateIndex(dir, [], 148)).toThrow("below 150");
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
