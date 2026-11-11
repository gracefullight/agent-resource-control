import { test, expect } from "bun:test";
import { readFileSync, writeFileSync, mkdtempSync, mkdirSync, rmSync, existsSync, unlinkSync } from "node:fs";
import { resolve } from "node:path";
import { tmpdir } from "node:os";
import { spawnSync } from "node:child_process";
import { root, context, reference, directory, canonicalBytes, boundJSON, validateCommon } from "./exec-fail-open-inputs";
import { validateGates, validateGateData, validateFloors, validatePriorArchive } from "./exec-fail-open-gates";
import { rejectFutureReferences, validateGraph, predecessorBindings } from "./exec-fail-open-graph";
import { fixtureFirstRun, fixtureCatalog } from "./exec-fail-open-fixture";
import { validateCommand } from "./exec-fail-open-receipts";
import { validateTiming, validateInvocationNumbers } from "./exec-fail-open-native";
import { checkPredecessorRead, predecessorReads } from "./exec-fail-open-reads";
import { requiredTests } from "./exec-fail-open-recipe";
import { seal } from "./exec-fail-open-compact";
import { retainedEntry } from "./exec-fail-open-container";

test("production inventory requires connection-denial and invalid-socket tests", () => {
  for (const name of ["connect_access_denied_before_register_runs_once", "connect_access_denied_after_register_ack_loss_stays_uncertain", "connect_access_denied_before_claim_stays_uncertain", "connect_access_denied_after_claim_ack_loss_stays_uncertain", "invalid_socket_parent_not_directory_does_not_launch", "invalid_socket_endpoint_not_socket_does_not_launch"])
    expect(requiredTests.filter(testName => testName === name)).toHaveLength(1);
});

test("production inventory requires both broken-stderr regressions", () => {
  for (const name of ["broken_stderr_unavailable_preserves_child_exit", "broken_stderr_uncertain_preserves_child_exit"])
    expect(requiredTests.filter(testName => testName === name)).toHaveLength(1);
});

test("predecessor read guard uses exact successor paths and restores nested root phases", () => {
  const predecessor = `${directory}/implementation/commands/focused-cli-final.json`;
  const bytes = canonicalBytes(root, predecessor);
  expect(bytes.length).toBeGreaterThan(0);
  predecessorReads(root, "gates", () => {
    expect(canonicalBytes(root, predecessor)).toEqual(bytes);
    for (const path of ["implementation/missing-final.json", "implementation/missing-terminal.json", "other/final.json", "other/terminal.json"])
      expect(() => canonicalBytes(root, `${directory}/${path}`)).toThrow("ENOENT");
  });
  const otherRoot = mkdtempSync(resolve(tmpdir(), "arc-guard-root-"));
  try {
  const successors = ["qa/review.md", "qa/review.json", "final.md", "final.json", "terminal.json"];
  // The nonexistent root proves rejection happens before filesystem access.
  for (const phase of ["gates", "evidence"] as const) predecessorReads(otherRoot, phase, () => {
    for (const path of successors) {
      const read = () => canonicalBytes(otherRoot, `${directory}/${path}`);
      expect(read).toThrow(phase === "gates" || path === "terminal.json" ? "attempted successor read" : "ENOENT");
    }
    // Same suffix outside the exact evidence namespace remains a predecessor.
    expect(() => canonicalBytes(otherRoot, "elsewhere/final.json")).toThrow("ENOENT");
    const terminal = `${directory}/terminal.json`;
    for (const alias of [terminal, `./${terminal}`, resolve(otherRoot, terminal), `${directory}//terminal.json`, `${directory}/qa/../terminal.json`])
      expect(() => checkPredecessorRead(otherRoot, alias)).toThrow("attempted successor read");
  });
  predecessorReads(otherRoot, "gates", () => {
    for (const present of [false, true]) {
      const isolated = mkdtempSync(resolve(tmpdir(), "arc-root-isolation-"));
      try {
        if (present) { mkdirSync(resolve(isolated, directory), { recursive: true }); writeFileSync(resolve(isolated, directory, "final.json"), "{}\n"); }
        expect(() => checkPredecessorRead(isolated, `${directory}/final.json`)).not.toThrow();
        if (present) expect(canonicalBytes(isolated, `${directory}/final.json`).toString()).toBe("{}\n");
        else expect(() => canonicalBytes(isolated, `${directory}/final.json`)).toThrow("ENOENT");
      } finally { rmSync(isolated, { recursive: true, force: true }); }
    }
    predecessorReads(otherRoot, "evidence", () => {
      expect(() => canonicalBytes(otherRoot, `${directory}/final.md`)).toThrow("gates attempted successor read");
    });
  });
  predecessorReads(otherRoot, "evidence", () => {
    expect(() => predecessorReads(otherRoot, "gates", () => {
      expect(() => canonicalBytes(otherRoot, `${directory}/qa/review.json`)).toThrow("gates attempted successor read");
      throw Error("nested unwind");
    })).toThrow("nested unwind");
    expect(() => canonicalBytes(otherRoot, `${directory}/qa/review.json`)).toThrow("ENOENT");
    expect(() => canonicalBytes(otherRoot, `${directory}/terminal.json`)).toThrow("evidence attempted successor read");
  });
  expect(() => predecessorReads(otherRoot, "gates", () => { throw Error("outer unwind"); })).toThrow("outer unwind");
  for (const path of successors)
    expect(() => canonicalBytes(otherRoot, `${directory}/${path}`)).toThrow("ENOENT");
  } finally { rmSync(otherRoot, { recursive: true, force: true }); }
});

test("retained native receipts bind their historical source and cannot satisfy the live gate", () => {
  const ctx = context(root, "retained"), live = context(root);
  const gates = JSON.parse(canonicalBytes(root, `${directory}/implementation/gates.json`).toString());
  expect(ctx.sourceHash).not.toBe(live.sourceHash);
  expect(() => validateCommon(gates, ctx, "implementation-gates", "backend")).not.toThrow();
  expect(() => validateCommon(gates, live, "implementation-gates", "backend")).toThrow("stale receipt inputs/git");
  expect(() => validateGates(root)).toThrow("stale receipt inputs/git");
  validatePriorArchive(root, gates);
  expect(gates.commands).toHaveLength(11);
  for (const ref of gates.commands) {
    const command = boundJSON(root, ref);
    validateCommon(command, ctx, "command", "backend");
    expect(command.status).toBe("passed");
    expect(command.sourceHashAfter).toBe(ctx.sourceHash);
  }
  for (const [ref, kind] of [[gates.timings, "native-timing"], [gates.historical, "historical-fixture"],
    [gates.preservation, "preservation"]] as const)
    validateCommon(boundJSON(root, ref), ctx, kind, "backend");
  expect(() => validateFloors(gates.coverage.crates, "historical-fixture")).not.toThrow();
  expect(() => validateFloors(gates.coverage.crates)).toThrow("missing");
  expect(() => validateCommon({ ...gates, sourceHash: "0".repeat(64) }, ctx, "implementation-gates", "backend")).toThrow("stale");
}, 3600000);
test("fixture graph constructs gates before successors and rejects cycles, future inputs and fake production receipts", () => {
  fixtureFirstRun((dir, refs, terminal) => {
    expect(terminal.gates.exit).toBe(0); expect(terminal.failed.exit).not.toBe(0);
    expect(terminal.assembly.exit).toBe(0);
    expect(terminal.staleAssembly.exit).not.toBe(0); expect(terminal.failedAssembly.exit).not.toBe(0);
    expect(() => validateGates(dir)).toThrow();
    const ctx = context(root);
    expect(() => validateCommand(terminal.fixture, ctx, dir)).toThrow();
    expect(() => validateGateData(root, terminal.ready.gates)).toThrow("fixture evidence is inadmissible");
    expect(() => validateGraph(fixtureCatalog.map(n => n.id === "gates" ? { ...n, requires: [...n.requires, "final"] } : n))).toThrow();
    expect(() => predecessorBindings(fixtureCatalog, "gates", { inputs: refs.inputs, "prerequisite-observations": refs["prerequisite-observations"], final: refs.final })).toThrow();
    for (const path of [`${directory}/qa/review.json`, `${directory}/final.json`, `${directory}/terminal.json`, "scripts/harness/exec-fail-open-gates.test.ts", "scripts/harness/exec-fail-open-evidence.test.ts"]) expect(() => rejectFutureReferences({ path })).toThrow();
    for (const path of ["final.json", "terminal.json", "exec-fail-open-gates.test.ts"]) expect(() => rejectFutureReferences({ path })).not.toThrow();
    const contractPath = resolve(dir, directory, "contract.json"), originalContract = readFileSync(contractPath);
    const changedContract = JSON.parse(originalContract.toString());
    changedContract.receiptDependencyGraph.nodes.find((n: any) => n.id === "gates").requires.push("final");
    writeFileSync(contractPath, JSON.stringify(changedContract));
    expect(() => validateGates(dir, terminal.recipe)).toThrow("successor dependency");
    writeFileSync(contractPath, originalContract);
    const candidate = terminal.ready.gates;
    // These mutations enter the same gate validator used by the assembler.
    expect(() => validateGateData(dir, { ...candidate, sourceHash: "stale" }, undefined, terminal.recipe)).toThrow("stale");
    const failedRef = { path: `${directory}/implementation/commands/failed.json`, sha256: "" };
    const failedBytes = readFileSync(resolve(dir, failedRef.path));
    try {
      writeFileSync(resolve(dir, failedRef.path), JSON.stringify(terminal.failed)); failedRef.sha256 = reference(dir, failedRef.path).sha256;
      expect(() => validateGateData(dir, { ...candidate, commands: [failedRef] }, undefined, terminal.recipe)).toThrow("failed fixture subprocess");
    } finally { writeFileSync(resolve(dir, failedRef.path), failedBytes); }
    expect(() => validateGateData(dir, { ...candidate, timings: { path: `${directory}/final.json`, sha256: refs.final.sha256 } }, undefined, terminal.recipe)).toThrow("successor");
    // Inject an actual attempted read at the environment leaf. The production
    // read boundary rejects it before existence or bytes can affect acceptance.
    expect(() => validateGateData(dir, candidate, undefined, { ...terminal.recipe,
      preservation: () => { canonicalBytes(dir, `${directory}/final.json`); return { passed: true }; },
    })).toThrow("attempted successor read");
    const bytes = readFileSync(resolve(dir, "inputs.json")); writeFileSync(resolve(dir, "inputs.json"), "tampered");
    expect(readFileSync(resolve(dir, "inputs.json"))).not.toEqual(bytes);
    expect(() => terminal.validate("gates")).toThrow();
  });
});
test("stale or incomplete timing, failed commands and source/host substitutions are rejected", () => {
  const ctx = context(root);
  for (const value of [{}, { passed: true }, { ...ctx, kind: "native-timing", actorId: "backend", skill: "oma-backend", passed: true, metadataBudgetMs: 4000, invocationTargetMs: 2000, cases: [] }]) expect(() => validateTiming(value, ctx, root, [])).toThrow();
  for (const value of [{}, { ...ctx, kind: "command", actorId: "backend", skill: "oma-backend", sourceHash: "stale" }, { ...ctx, kind: "command", actorId: "backend", skill: "oma-backend", exit: 17, status: "passed" }]) expect(() => validateCommand(value, ctx, root)).toThrow();
});

test("measured-target arithmetic rejects overshoots, replay, altered exits and reduced coverage floors", () => {
  // Unit fixture numbers exercise the same leaf predicates; they are never
  // emitted as native measurements or production acceptance.
  const value = { passed: true, invocations: 1, recoveryInvocations: 1, observedExit: 17, elapsedToInvocationMs: 1800 };
  expect(() => validateInvocationNumbers(value)).not.toThrow();
  for (const patch of [{ elapsedToInvocationMs: 2000.001 }, { invocations: 2 }, { recoveryInvocations: 2 }, { observedExit: 0 }, { elapsedToInvocationMs: NaN }]) expect(() => validateInvocationNumbers({ ...value, ...patch })).toThrow();
  const names = ["arc-core", "arc-detector", "arc-storage", "arc-ipc", "arc-daemon", "arc-cli", "arc-traffic"];
  const floors = names.map(crate => ({ crate, total: 100, covered: 80 }));
  expect(() => validateFloors(floors)).not.toThrow();
  expect(() => validateFloors(floors.slice(1))).toThrow();
  expect(() => validateFloors(floors.map(c => c.crate === "arc-cli" ? { ...c, covered: 79 } : c))).toThrow();
  const historical = floors.slice(0, -1);
  expect(() => validateFloors(historical, "historical-fixture")).not.toThrow();
  expect(() => validateFloors(historical)).toThrow();
  expect(() => validateFloors(historical.slice(1), "historical-fixture")).toThrow();
  expect(() => validateFloors(historical.map(c => c.crate === "arc-cli" ? { ...c, covered: 79 } : c), "historical-fixture")).toThrow();
});

test("complete gate accepts exact predecessor names and rejects actual successors in selected records", () => {
  fixtureFirstRun((dir, _refs, state) => {
    const candidate = state.ready.gates, recipe = state.recipe;
    const base = `${directory}/implementation/commands`;
    const original = readFileSync(resolve(dir, base, "prerequisite.json"));
    const stage = (path: string) => {
      const result = spawnSync("git", ["add", "--", path], { cwd: dir, encoding: "utf8" });
      expect(result.status).toBe(0);
    };
    const legacy = (name: string, bytes = original) => {
      const path = `${base}/${name}`;
      writeFileSync(resolve(dir, path), bytes);
      return reference(dir, path);
    };
    const compact = (name: string, logical: string, bytes = original) => {
      const path = `${base}/${name}`, entry = `${base}/${logical}`;
      const sealed = seal(dir, path, [retainedEntry(entry, bytes)]);
      stage(path);
      return sealed.entries[entry];
    };
    const refs = [
      legacy("focused-cli-final.json"), legacy("workspace-final.json"), legacy("nonterminal.json"),
      compact("container-final.json", "selected-terminal.json"),
      compact("container-terminal.json", "selected-final.json"),
    ];
    const successorPaths = ["qa/review.md", "qa/review.json", "final.md", "final.json", "terminal.json"].map(p => `${directory}/${p}`);
    const commands = ["scripts/harness/exec-fail-open-gates.test.ts", "scripts/harness/exec-fail-open-evidence.test.ts"]
      .flatMap(path => [path, `./${path}`, resolve(dir, path)]);
    const saved = new Map<string, Buffer>();
    for (const path of successorPaths) {
      const full = resolve(dir, path);
      if (existsSync(full)) { saved.set(path, readFileSync(full)); unlinkSync(full); }
    }
    const errors: string[] = [];
    for (const present of [false, true]) {
      if (present) for (const path of successorPaths) {
        mkdirSync(resolve(dir, path, ".."), { recursive: true });
        writeFileSync(resolve(dir, path), saved.get(path) ?? "{}\n");
      }
      expect(validateGateData(dir, candidate, undefined, recipe).passed).toBe(true);
      for (const ref of refs) {
        try { validateGateData(dir, { ...candidate, commands: [ref] }, undefined, recipe); }
        catch (error) { errors.push(`${present ? "present" : "absent"}: ${ref.path} ${ref.selector?.path ?? ""}: ${error}`); }
      }
    }
    expect(errors).toEqual([]);

    for (const present of [false, true]) {
      for (const path of successorPaths) {
        const full = resolve(dir, path);
        if (present) writeFileSync(full, saved.get(path) ?? "{}\n");
        else if (existsSync(full)) unlinkSync(full);
      }
      expect(validateGateData(dir, candidate, undefined, recipe).passed).toBe(true);
      for (const path of [...successorPaths, ...commands]) {
        const bad = { path, sha256: "0".repeat(64) };
        expect(() => validateGateData(dir, { ...candidate, commands: [bad] }, undefined, recipe)).toThrow("successor");
      }
      const nestedValues = [
        ...successorPaths.map(path => ({ path, sha256: "0".repeat(64) })),
        ...successorPaths.map(path => ({ ...refs[3], selector: { path, sha256: "0".repeat(64) } })),
        ...commands.map(path => ({ argv: ["bun", "test", path] })),
      ];
      for (const [index, nested] of nestedValues.entries()) {
        const bytes = Buffer.from(JSON.stringify({ ...JSON.parse(original.toString()), nested }));
        const plain = legacy(`nested-${present}-${index}.json`, bytes);
        const selected = compact(`nested-container-${present}-${index}.json`, `nested-selected-${present}-${index}.json`, bytes);
        for (const ref of [plain, selected])
          expect(() => validateGateData(dir, { ...candidate, commands: [ref] }, undefined, recipe)).toThrow("successor");
      }
    }
  });
}, 3600000);

// Add a reachable observation to the otherwise unchanged historical receipt.
// Its extra field is not a prerequisite in the fixture recipe, so a passing
// result means the actual gate walk accepted the nested evidence.
function guardFixture(run: (f: any) => void) {
  fixtureFirstRun((dir, _refs, state) => {
    const base = `${directory}/implementation/commands`, hist = state.ready.gates.historical.path;
    const historical = JSON.parse(readFileSync(resolve(dir, hist), "utf8"));
    const command = JSON.parse(readFileSync(resolve(dir, base, "prerequisite.json"), "utf8"));
    let number = 0;
    const stage = (path: string) => {
      const result = spawnSync("git", ["add", "--", path], { cwd: dir, encoding: "utf8" });
      expect(result.status).toBe(0);
    };
    const makeRaw = (selected: boolean, suffix: string, bytes: Buffer) => {
      const id = number++;
      if (!selected) {
        const path = `${base}/guard-${id}${suffix}`;
        writeFileSync(resolve(dir, path), bytes);
        return reference(dir, path);
      }
      const path = `${base}/guard-container-${id}.json`, logical = `${base}/guard-selected-${id}${suffix}`;
      const sealed = seal(dir, path, [retainedEntry(logical, bytes)]);
      stage(path);
      return sealed.entries[logical];
    };
    const make = (selected: boolean, suffix: string, nested?: unknown) => makeRaw(selected, suffix,
      Buffer.from(JSON.stringify({ ...command, ...(nested === undefined ? {} : { nested }) })));
    const check = (ref: any) => {
      writeFileSync(resolve(dir, hist), JSON.stringify({ ...historical, nestedEvidence: [ref] }));
      const gates = { ...state.ready.gates, historical: reference(dir, hist) };
      try { validateGateData(dir, gates, undefined, state.recipe); return "accepted"; }
      catch (error) { return String(error); }
    };
    const successors = ["qa/review.md", "qa/review.json", "final.md", "final.json", "terminal.json"].map(p => `${directory}/${p}`);
    const saved = new Map(successors.filter(path => existsSync(resolve(dir, path))).map(path => [path, readFileSync(resolve(dir, path))]));
    const setSuccessors = (present: boolean) => {
      for (const path of successors) {
        if (present) { mkdirSync(resolve(dir, path, ".."), { recursive: true }); writeFileSync(resolve(dir, path), saved.get(path) ?? "{}\n"); }
        else if (existsSync(resolve(dir, path))) unlinkSync(resolve(dir, path));
      }
    };
    run({ dir, base, command, make, makeRaw, stage, check, setSuccessors, candidate: state.ready.gates, recipe: state.recipe });
  });
}

test("complete gate rejects nested successor aliases in legacy and selected receipts", () => {
  guardFixture(({ dir, make, check }) => {
    const successors = ["qa/review.md", "qa/review.json", "final.md", "final.json", "terminal.json"].map(p => `${directory}/${p}`);
    const aliases = [
      ["canonical", successors[1]], ["dot", `./${successors[1]}`], ["absolute", resolve(dir, successors[1])],
      ["repeated", `${directory}//final.json`], ["component-dot", `${directory}/./final.md`],
      ["parent", `${directory}/implementation/../terminal.json`], ["nested-parent", `${directory}/qa/temp/../review.md`],
      ["absolute-final", resolve(dir, successors[3])],
    ];
    const results: any[] = [], failures: any[] = [];
    for (const present of [false, true]) {
      for (const path of successors) {
        if (present) { mkdirSync(resolve(dir, path, ".."), { recursive: true }); writeFileSync(resolve(dir, path), "{}\n"); }
        else if (existsSync(resolve(dir, path))) unlinkSync(resolve(dir, path));
      }
      for (const selected of [false, true]) for (const field of ["path", "selector"]) for (const [label, alias] of aliases) {
        expect(check(make(selected, ".json"))).toBe("accepted");
        const ref = { path: alias, sha256: "0".repeat(64) };
        const nested = field === "path" ? ref : { path: `${directory}/implementation/collection.json`, sha256: "0".repeat(64), selector: ref };
        const result = check(make(selected, ".json", { layer: [[nested]] }));
        results.push({ present, selected, field, label, result });
        if (!/successor|unsafe (?:artifact|entry)/.test(result)) failures.push({ present, selected, field, label, result });
      }
    }
    console.log("nested successor alias outcomes", JSON.stringify(results));
    expect(failures).toEqual([]);
  });
}, 3600000);

test("complete gate rejects terminal validators in structured command strings", () => {
  guardFixture(({ dir, make, check, setSuccessors }) => {
    const failures: any[] = [];
    for (const present of [false, true]) for (const selected of [false, true]) {
      setSuccessors(present);
      expect(check(make(selected, ".json", { command: "bun test scripts/harness/unrelated.test.ts" }))).toBe("accepted");
      expect(check(make(selected, ".json", { command: "/usr/local/bin/exec-fail-open-gates.test.ts --help" }))).toBe("accepted");
      expect(check(make(selected, ".json", { note: "bun test scripts/harness/exec-fail-open-gates.test.ts" }))).toBe("accepted");
      for (const name of ["gates", "evidence"]) {
        const path = `scripts/harness/exec-fail-open-${name}.test.ts`;
        for (const alias of [path, `./${path}`, resolve(dir, path), `scripts//harness/./exec-fail-open-${name}.test.ts`]) {
          for (const key of ["command", "commands", "argv"]) {
            for (const expression of [`bun test ${alias} --verbose`, `bun test '${alias}' --verbose`, `sh -c "bun test ${alias}"`]) {
              const result = check(make(selected, ".json", { steps: [[{ [key]: expression }]] }));
              if (!/successor|terminal validation/.test(result)) failures.push({ present, selected, name, alias, key, expression, result });
            }
          }
        }
      }
    }
    console.log("structured command bypasses", JSON.stringify(failures));
    expect(failures).toEqual([]);
  });
}, 3600000);

test("complete gate inspects direct and referenced coverage dependencies", () => {
  guardFixture(({ make, candidate, recipe, dir, setSuccessors }) => {
    const validate = (nested: unknown) => {
      try { validateGateData(dir, { ...candidate, coverage: { ...candidate.coverage, nested } }, undefined, recipe); return "accepted"; }
      catch (error) { return String(error); }
    };
    const successor = { path: `${directory}/final.json`, sha256: "0".repeat(64) };
    const failures: any[] = [];
    for (const present of [false, true]) for (const selected of [false, true]) {
      setSuccessors(present);
      expect(validate({ note: "final.json", path: "unrelated/final.json" })).toBe("accepted");
      expect(validate(make(selected, ".json"))).toBe("accepted");
      const values = [{ inner: [[successor]] }, make(selected, ".json", { inner: [[successor]] }),
        { selector: { ...successor, path: `./${successor.path}` } }, { command: "bun test scripts/harness/exec-fail-open-evidence.test.ts" }];
      for (const [index, value] of values.entries()) {
        const result = validate(value);
        if (!/successor|terminal validation/.test(result)) failures.push({ present, selected, index, result });
      }
    }
    console.log("coverage successor bypasses", JSON.stringify(failures));
    expect(failures).toEqual([]);
  });
}, 3600000);

test("complete gate inspects consumed legacy receipts outside evidence directory", () => {
  guardFixture(({ dir, command, make, candidate, recipe }) => {
    const timingPath = candidate.timings.path;
    const timing = JSON.parse(readFileSync(resolve(dir, timingPath), "utf8"));
    const validate = (ref: any) => {
      writeFileSync(resolve(dir, timingPath), JSON.stringify({ ...timing, cases: timing.cases.map((c: any) => ({ ...c, commandReceipt: ref })) }));
      const gates = { ...candidate, commands: [ref], timings: reference(dir, timingPath) };
      try { validateGateData(dir, gates, undefined, recipe); return "accepted"; }
      catch (error) { return String(error); }
    };
    const failures: any[] = [], successor = { path: `${directory}/final.json`, sha256: "0".repeat(64) };
    for (const present of [false, true]) {
      const successorFile = resolve(dir, successor.path);
      if (present) writeFileSync(successorFile, "{}\n"); else if (existsSync(successorFile)) unlinkSync(successorFile);
      for (const suffix of [".json", ".txt", ""]) {
        const path = `target/outside-receipt-${present}${suffix}`;
        writeFileSync(resolve(dir, path), JSON.stringify(command));
        expect(validate(reference(dir, path))).toBe("accepted");
        writeFileSync(resolve(dir, path), JSON.stringify({ ...command, nested: { layer: [successor] } }));
        const bad = reference(dir, path);
        expect(boundJSON(dir, bad).nested.layer[0]).toEqual(successor);
        for (const [kind, ref] of [["legacy", bad], ["selected", make(true, suffix, { dependency: bad })]] as const) {
          const result = validate(ref);
          if (!/successor|terminal validation/.test(result)) failures.push({ present, suffix, kind, result });
        }
      }
    }
    console.log("outside receipt successor bypasses", JSON.stringify(failures));
    expect(failures).toEqual([]);
  });
}, 3600000);

test("complete gate inspects legacy and staged JSON receipts regardless of logical filename", () => {
  guardFixture(({ make, check, dir }) => {
    const results: any[] = [], failures: any[] = [];
    const path = `${directory}/final.json`;
    for (const present of [false, true]) for (const selected of [false, true]) for (const suffix of [".json", ".txt", ""]) {
      if (present) writeFileSync(resolve(dir, path), "{}\n");
      else if (existsSync(resolve(dir, path))) unlinkSync(resolve(dir, path));
      expect(check(make(selected, suffix))).toBe("accepted");
      const ref = make(selected, suffix, { layer: [{ path, sha256: "0".repeat(64) }] });
      expect(boundJSON(dir, ref).nested.layer[0].path).toBe(path);
      const result = check(ref); results.push({ present, selected, suffix, result });
      if (!/successor|unsafe (?:artifact|entry)/.test(result)) failures.push({ present, selected, suffix, result });
    }
    console.log("selected receipt suffix outcomes", JSON.stringify(results));
    expect(failures).toEqual([]);
  });
}, 3600000);

test("complete gate preserves opaque inputs, sibling selections, tamper and cycle guards", () => {
  guardFixture(({ dir, base, command, make, makeRaw, stage, check }) => {
    const successor = { path: `${directory}/final.json`, sha256: "0".repeat(64) };
    expect(check(make(false, ".json", { note: "final.json", commandDescription: "exec-fail-open-gates.test.ts", executable: { path: "/usr/local/bin/exec-fail-open-gates.test.ts", sha256: "0".repeat(64) } }))).toBe("accepted");
    for (const selected of [false, true]) {
      expect(check(make(selected, ".json", { metadata: { executable: { path: "bin/unrelated-tool", sha256: "0".repeat(64) } } }))).toBe("accepted");
      expect(check(make(selected, ".json", { metadata: { executable: { path: "scripts/harness/exec-fail-open-gates.test.ts", sha256: "0".repeat(64) } } }))).toContain("successor");
    }
    for (const commandPath of ["scripts/harness/exec-fail-open-gates.test.ts", "scripts/harness/exec-fail-open-evidence.test.ts"]) {
      for (const alias of [commandPath, `./${commandPath}`, resolve(dir, commandPath), `scripts//harness/./${commandPath.split("/").pop()}`]) {
        expect(check(make(false, ".json"))).toBe("accepted");
        expect(check(make(true, ".json", { steps: [{ argv: ["bun", "test", alias] }] }))).toContain("successor");
      }
    }
    for (const selected of [false, true]) {
      expect(check(makeRaw(selected, ".txt", Buffer.from("opaque bytes\n")))).toBe("accepted");
      expect(check(makeRaw(selected, ".json", Buffer.from("not JSON\n")))).toMatch(/JSON|parse|Syntax/i);
    }
    const container = `${base}/siblings.json`, a = `${base}/sibling-a.txt`, b = `${base}/sibling-b`;
    const hidden = `${base}/unselected.json`;
    const sealed = seal(dir, container, [retainedEntry(a, Buffer.from(JSON.stringify(command))),
      retainedEntry(b, Buffer.from(JSON.stringify(command))), retainedEntry(hidden, Buffer.from(JSON.stringify({ ...command, successor })))]);
    stage(container);
    expect(check([sealed.entries[a], sealed.entries[b]])).toBe("accepted");
    expect(check(sealed.entries[a])).toBe("accepted");
    expect(check(sealed.entries[hidden])).toContain("successor");
    const unsafePath = { ...sealed.entries[a], path: `./${container}` };
    const unsafeSelector = { ...sealed.entries[a], selector: { ...sealed.entries[a].selector, path: `./${a}` } };
    for (const ref of [unsafePath, unsafeSelector]) {
      expect(check(make(false, ".json"))).toBe("accepted");
      expect(check(make(false, ".json", ref))).toMatch(/unsafe (?:artifact|entry)/);
    }
    const bytes = readFileSync(resolve(dir, container));
    try {
      writeFileSync(resolve(dir, container), Buffer.concat([bytes, Buffer.from(" ")]));
      expect(check(sealed.entries[a])).toMatch(/hash differs|index bytes/);
    } finally { writeFileSync(resolve(dir, container), bytes); }
    const cycleContainer = `${base}/cycle.json`, cycleEntry = `${base}/cycle.txt`;
    const cycleRef = { path: cycleContainer, sha256: "0".repeat(64), selector: { path: cycleEntry, sha256: "0".repeat(64) } };
    const cycle = seal(dir, cycleContainer, [retainedEntry(cycleEntry, Buffer.from(JSON.stringify({ ...command, nested: cycleRef })))]);
    stage(cycleContainer);
    expect(check(cycle.entries[cycleEntry])).toContain("self or terminal successor dependency");
    const disguised = makeRaw(true, ".txt", Buffer.from(JSON.stringify({ kind: "retained-evidence-container", nested: successor })));
    expect(check(disguised)).toContain("successor");
  });
}, 3600000);

test("complete gate resolves nested commands against their own cwd and limits successors to its root", () => {
  guardFixture(({ dir, make, check, setSuccessors }) => {
    const failures: any[] = [];
    for (const name of ["final.json", "terminal.json"]) writeFileSync(resolve(dir, name), "{}\n");
    for (const present of [false, true]) for (const selected of [false, true]) {
      setSuccessors(present);
      const ok = [
        reference(dir, "final.json"), reference(dir, "terminal.json"),
        { command: { cwd: ".", args: ["bun", "test", resolve(root, "scripts/harness/exec-fail-open-gates.test.ts")] } },
        { command: { cwd: ".", args: ["bun", "test", "exec-fail-open-gates.test.ts"] } },
        { command: "cd scripts/harness && bun test ./unrelated.test.ts>/dev/null" },
        { command: `bun test ${resolve(root, "scripts/harness/exec-fail-open-gates.test.ts")}>/dev/null` },
        { note: "bun test scripts/harness/exec-fail-open-gates.test.ts" },
      ];
      for (const value of ok) {
        const result = check(make(selected, ".json", value));
        if (result !== "accepted") failures.push({ present, selected, value, result });
      }
      for (const name of ["gates", "evidence"]) {
        const file = `exec-fail-open-${name}.test.ts`, path = `scripts/harness/${file}`;
        const bad = [
          { command: { cwd: "scripts/harness", args: ["bun", "test", `./${file}`] } },
          { command: { cwd: ".", args: ["bun", "test", path] } },
          { command: { cwd: "scripts/harness", nested: [{ args: ["bun", "test", file] }] } },
          { command: `cd scripts/harness && bun test ./${file}` },
          { command: `cd scripts/harness && bun test ./${file}>/dev/null` },
          { command: { cwd: "scripts/harness", args: ["bun", "test", `./${file}>/dev/null`] } },
          { command: `cd scripts/harness && bun test ./${file} 2>/dev/null` },
          { command: `sh -c "cd scripts/harness && bun test ./${file}>>/dev/null"` },
          { command: { cwd: resolve(dir, "scripts/harness"), argv: ["bun", "test", file] } },
          { path: `${directory}/qa/../final.json` },
        ];
        for (const value of bad) {
          expect(check(make(selected, ".json"))).toBe("accepted");
          const result = check(make(selected, ".json", value));
          if (!/successor|terminal validation|unsafe (?:artifact|entry)/.test(result)) failures.push({ present, selected, value, result });
        }
      }
    }
    console.log("contextual command/root guard outcomes", JSON.stringify(failures));
    expect(failures).toEqual([]);
  });
}, 3600000);
