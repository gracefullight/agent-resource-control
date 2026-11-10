//! First-run regression calls the actual assembler and both production validators.
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync, rmSync, existsSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { spawnSync } from "node:child_process";
import { root as productRoot, load, reference, hash, requireValue, directory, baseline, baselineTree } from "./exec-fail-open-inputs";
import { fixtureRecipe, validateFixtureProcess, fixtureCatalog } from "./exec-fail-open-fixture-recipe";
import { validateGates } from "./exec-fail-open-gates";
import { validateEvidence } from "./exec-fail-open-evidence";
export { fixtureCatalog };
export function fixtureFirstRun(run: (root: string, refs: Record<string, {sha256: string}>, terminal: any) => void, retainedFiles = 0) {
  const root = mkdtempSync(join(tmpdir(), "arc-evidence-first-"));
  const save = (path: string, value: any) => {
    mkdirSync(resolve(root, path, ".."), { recursive: true });
    writeFileSync(resolve(root, path), typeof value === "string" ? value : JSON.stringify(value, null, 2)+"\n");
    return reference(root, path);
  };
  const git = (...args: string[]) => {
    const r = spawnSync("git", args, { cwd: root, encoding: "utf8" }); requireValue(r.status === 0, r.stderr);
  };
  let number = 0;
  const execute = (argv: string[], actor = "backend") => {
    const recipe = fixtureRecipe(root), ctx = recipe.context(), started = new Date().toISOString(), start = performance.now();
    const r = spawnSync(argv[0], argv.slice(1), { cwd: root, encoding: "utf8", timeout: 3600000 });
    requireValue(!r.error && r.status !== null && r.pid > 0, "fixture subprocess did not finish");
    const base = `target/exec-fail-open/fixture-${number++}`;
    const stdout = save(`${base}/stdout.log`, r.stdout), stderr = save(`${base}/stderr.log`, r.stderr);
    const observed = { fixtureOnly: true, pid: r.pid, exit: r.status, stdoutSha256: stdout.sha256, stderrSha256: stderr.sha256 };
    return { ...ctx, kind: "command", actorId: actor, skill: actor === "reviewer" ? "oma-qa" : "oma-backend", argv, pid: r.pid, exit: r.status,
      started, ended: new Date().toISOString(), elapsedMs: performance.now()-start, status: r.status === 0 ? "passed" : "failed", sourceHashAfter: recipe.context().sourceHash,
      output: { stdoutPath: stdout.path, stderrPath: stderr.path }, stdoutSha256: stdout.sha256, stderrSha256: stderr.sha256,
      processObservation: save(`${base}/process.json`, observed),
      ...(r.status === 0 && argv.length === 2 && argv[1] === resolve(root, "fixture-worker.ts")
        ? { execution: { passed: 1, failed: 0, ignored: 0, tests: [{ name: JSON.parse(r.stdout).test, status: "ok" }] } } : {}) };
  };
  try {
    git("init", "--quiet"); save("inputs.json", { fixtureOnly: true, input: "executed first-run integrity check" });
    const worker = resolve(root, "fixture-worker.ts"), argv = [Bun.which("bun")!, worker];
    save("fixture-worker.ts", `import {readFileSync} from 'node:fs'; import {createHash} from 'node:crypto'; import {spawnSync} from 'node:child_process'; const child=spawnSync(process.execPath,['-e',"process.stdout.write('fixture-child-once');process.exit(17)"],{encoding:'utf8'}); if(child.status!==17||child.stdout!=='fixture-child-once') throw Error('fixture child failed'); const input=readFileSync('inputs.json'); if(!JSON.parse(input).fixtureOnly) throw Error('not fixture'); console.log(JSON.stringify({fixtureOnly:true, inputSha256:createHash('sha256').update(input).digest('hex'),test:'fixture::prerequisite',childExit:child.status,invocations:1,passed:true}));`);
    save("fixture-recipe.json", { fixtureOnly: true, workspaceArgv: argv, context: {
      fixtureOnly: true, schemaVersion: 1, baselineHead: baseline, baselineTree, currentInputs: {}, observedGit: { fixtureOnly: true }, host: { fixtureOnly: true }, tools: { fixtureOnly: true },
    } });
    save(`${directory}/contract.json`, { fixtureOnly: true, receiptDependencyGraph: { nodes: fixtureCatalog }, commandPhases: { prerequisiteCommands: [argv] } });
    for (const name of ["contract.md", "design-contract.md"]) save(`${directory}/${name}`, "Fixture only. Never production acceptance.\n");
    const prerequisite = execute(argv); validateFixtureProcess(root, prerequisite);
    save(`${directory}/implementation/commands/prerequisite.json`, prerequisite);
    const ctx = fixtureRecipe(root).context(), pid = prerequisite.pid;
    const actual = JSON.parse(readFileSync(resolve(root, prerequisite.output.stdoutPath), "utf8"));
    const raw = { fixtureOnly: true, scenario: "absent", test: "fixture::prerequisite", recordedUnixMs: Date.parse(prerequisite.ended),
      candidateBinaryPath: worker, candidateSha256: hash(readFileSync(worker)), boundary: "fixture subprocess elapsed measurement", samples: { startNs: 0, endNs: prerequisite.elapsedMs*1e6 },
      parentSpawnToInvocationMs: prerequisite.elapsedMs, subdivisionReason: "Fixture-only subprocess, no native metadata subdivisions", counts: {}, events: [], proxyClock: "fixture-only", commitRows: [], invocations: actual.invocations, recoveryInvocations: actual.invocations, observedExit: actual.childExit, passed: actual.passed };
    save(`target/exec-fail-open/native/${pid}/absent.json`, raw);
    save(`target/exec-fail-open/native/${pid}/negative-control.json`, { fixtureOnly: true });
    save("target/exec-fail-open/history/preparation.json", { fixtureOnly: true, files: {}, identity: ctx.sourceHash, tools: [], binary: { path: "fixture-worker.ts", sha256: raw.candidateSha256 } });
    save("target/exec-fail-open/history/routes.jsonl", JSON.stringify({ fixtureOnly: true, recordedUnixMs: Date.parse(prerequisite.ended) })+"\n");
    save(".arc-verify/rust-coverage/baseline/latest.json", { pass: true, runId: "fixture-only", directory: ".arc-verify/rust-coverage/baseline/runs/fixture-only" });
    save(".arc-verify/rust-coverage/baseline/runs/fixture-only/summary.json", { fixtureOnly: true, sourceHash: ctx.sourceHash, pass: true, childProof: [],
      crates: ["arc-core", "arc-detector", "arc-storage", "arc-ipc", "arc-daemon", "arc-cli"].map(crate => ({ crate, covered: 80, total: 100 })) });
    const refs: Record<string, { sha256: string }> = { inputs: reference(root, "inputs.json"), "prerequisite-observations": reference(root, `${directory}/implementation/commands/prerequisite.json`) };
    const modulePath = (name: string) => JSON.stringify(resolve(import.meta.dir, `exec-fail-open-${name}.ts`));
    const prefix = `import {fixtureRecipe} from ${modulePath("fixture-recipe")}; const root=${JSON.stringify(root)}; const recipe=fixtureRecipe(root);`;
    const successors = ["qa/review.json", "final.json", "terminal.json"].map(p => resolve(root, directory, p));
    requireValue(successors.every(p => !existsSync(p)), "fixture successors preexist");
    save(`${directory}/implementation/commands/stale.json`, { ...prerequisite, sourceHash: "stale" });
    const staleAssembly = execute([Bun.which("bun")!, "-e", `${prefix} import {assemble} from ${modulePath("assemble")}; assemble(['stale'],recipe);`]);
    requireValue(staleAssembly.exit !== 0 && !existsSync(resolve(root, directory, "implementation/gates.json")), "assembler admitted stale inputs");
    const failedCommand = execute([Bun.which("bun")!, "-e", "throw Error('fixture-only failed prerequisite')"]);
    save(`${directory}/implementation/commands/failed.json`, failedCommand);
    const failedAssembly = execute([Bun.which("bun")!, "-e", `${prefix} import {assemble} from ${modulePath("assemble")}; assemble(['failed'],recipe);`]);
    requireValue(failedAssembly.exit !== 0 && !existsSync(resolve(root, directory, "implementation/gates.json")), "assembler admitted failed inputs");
    const assembly = execute([Bun.which("bun")!, "-e", `${prefix} import {assemble} from ${modulePath("assemble")}; assemble(['prerequisite'],recipe);`]);
    requireValue(assembly.exit === 0, readFileSync(resolve(root, assembly.output.stderrPath), "utf8"));
    validateFixtureProcess(root, assembly); refs.gates = reference(root, `${directory}/implementation/gates.json`);
    const predecessorPaths = ["inputs.json", ...readdirSync(resolve(root, directory, "implementation")).filter(n => n.endsWith(".json")).map(n => `${directory}/implementation/${n}`)];
    const unchanged = new Map(predecessorPaths.map(path => [path, hash(readFileSync(resolve(root, path)))]));
    const gates = execute([Bun.which("bun")!, "-e", `${prefix} import {validateGates} from ${modulePath("gates")}; validateGates(root,recipe); console.log('validated fixture gates');`]);
    validateFixtureProcess(root, gates); requireValue(successors.every(p => !existsSync(p)), "gate read/created successor");
    refs["gates-command-observation"] = save("gates-command-observation.json", gates);
    const reproduction = execute(argv, "reviewer"); fixtureRecipe(root).command(reproduction, ctx, "reviewer");
    refs["qa-reproduction"] = save("qa-reproduction.json", reproduction);
    const ready = validateGates(root, fixtureRecipe(root));
    const report = save(`${directory}/qa/review.md`, `Fixture-only independent reproduction, not a real review. Baseline ${baseline}. `+"fixture evidence cannot establish production acceptance. ".repeat(5));
    refs.review = save(`${directory}/qa/review.json`, { ...ctx, kind: "independent-review", actorId: "reviewer", skill: "oma-qa", status: "passed", openBlockers: 0,
      productFilesEdited: false, implementationTestsEdited: false, findings: [], report,
      reviewedReceipts: [ready.reference, ready.gates.timings, ready.gates.historical, ...ready.gates.commands], commands: [reproduction] });
    const claims = save(`${directory}/claims-review.md`, "Fixture-only. desktop, native Windows, vendor hooks and scheduling remain outstanding; business results are unmeasured. ".repeat(3));
    const final = { ...ctx, kind: "final-decision", verdict: "pass", openBlockers: 0, gates: ready.reference, timings: ready.gates.timings, coverage: ready.gates.coverage,
      review: refs.review, reviewReport: report, claimsReview: claims, outstanding: ["desktop", "native Windows", "vendor hooks", "scheduling"], limitations: "unmeasured fixture only", evidenceFiles: 0 };
    save(`${directory}/final.json`, final);
    for (let i = 0; i < retainedFiles; i++) save(`${directory}/retained-${i}.json`, { fixtureOnly: true });
    git("add", directory);
    const count = spawnSync("git", ["ls-files", "--", directory], { cwd: root, encoding: "utf8" }).stdout.trim().split("\n").length;
    final.evidenceFiles = count; refs.final = save(`${directory}/final.json`, final); git("add", directory);
    requireValue(!existsSync(resolve(root, directory, "terminal.json")), "terminal preexists first evidence run");
    const evidence = execute([Bun.which("bun")!, "-e", `${prefix} import {validateEvidence} from ${modulePath("evidence")}; validateEvidence(root,recipe); console.log('validated fixture evidence');`]);
    requireValue(evidence.exit === 0, readFileSync(resolve(root, evidence.output.stderrPath), "utf8"));
    validateFixtureProcess(root, evidence); refs["evidence-command-observation"] = save("evidence-command-observation.json", evidence);
    refs.terminal = save(`${directory}/terminal.json`, { fixtureOnly: true, commands: [gates, evidence], passed: true });
    for (const [path, sha] of unchanged) requireValue(hash(readFileSync(resolve(root, path))) === sha, "terminal rewrote predecessor");
    // Fail the real validator in a subprocess, then prove a relabeled observation
    // remains failed. This is not a graph-only stand-in for validation.
    const failed = execute([Bun.which("bun")!, "-e", `${prefix} import {validateGateData} from ${modulePath("gates")}; import {readFileSync} from 'node:fs'; const g=JSON.parse(readFileSync(${JSON.stringify(resolve(root,directory,"implementation/gates.json"))},'utf8')); g.sourceHash='stale'; validateGateData(root,g,undefined,recipe);`]);
    requireValue(failed.exit !== 0, "stale validator unexpectedly passed");
    let rejected = false; try { validateFixtureProcess(root, { ...failed, exit: 0, status: "passed" }); } catch { rejected = true; }
    requireValue(rejected, "failed terminal relabeled pass");
    const validate = (id: string) => id === "gates" ? validateGates(root, fixtureRecipe(root)) : validateEvidence(root, fixtureRecipe(root));
    run(root, refs, { assembly, staleAssembly, failedAssembly, gates, evidence, failed, fixture: prerequisite, validate, recipe: fixtureRecipe(root), ready });
  } finally { rmSync(root, { recursive: true, force: true }); }
}
