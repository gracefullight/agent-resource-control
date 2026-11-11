import { test, expect } from "bun:test";
import { chmodSync, copyFileSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, relative, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { walk } from "../../coverage/source";
import { baseline, baselineTree, context, currentInputs, directory, hash, load, root } from "./exec-fail-open-inputs";
import { capturePending, validateCommand } from "./exec-fail-open-receipts";
import { validateFailedAttempt } from "./exec-fail-open-gates";
import { productionRecipe } from "./exec-fail-open-recipe";

test("real captures bind identical staged and untracked inputs across checkout revisions", async () => {
  const base = mkdtempSync(resolve(tmpdir(), "arc-cross-checkout-")), seed = resolve(base, "seed");
  const git = (cwd: string, ...argv: string[]) => {
    const env = { ...process.env, GIT_AUTHOR_NAME: "Fixture", GIT_AUTHOR_EMAIL: "fixture@example.invalid",
      GIT_COMMITTER_NAME: "Fixture", GIT_COMMITTER_EMAIL: "fixture@example.invalid" };
    const r = spawnSync("git", argv, { cwd, env, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
    if (r.status !== 0) throw Error(`git ${argv.join(" ")}: ${r.stderr}`);
    return r.stdout.trim();
  };
  const actual = currentInputs(root), f = load(root, `${directory}/contract.json`).inputFingerprint;
  const overlay = (dir: string) => {
    const existing = [...f.requiredFiles, ...f.optionalIfPresent, ...f.recursiveScopes.flatMap((scope: string) =>
      existsSync(resolve(dir, scope)) ? walk(resolve(dir, scope)).map(p => relative(dir, p)) : [])];
    for (const path of new Set<string>(existing)) if (!actual.files[path] && existsSync(resolve(dir, path))) unlinkSync(resolve(dir, path));
    for (const [path, info] of Object.entries(actual.files)) {
      const target = resolve(dir, path); mkdirSync(dirname(target), { recursive: true });
      copyFileSync(resolve(root, path), target); chmodSync(target, info.mode);
    }
    expect(currentInputs(dir)).toEqual(actual);
  };
  const copyOutput = (from: string, to: string, receipt: any) => {
    for (const path of Object.values(receipt.output) as string[]) {
      mkdirSync(dirname(resolve(to, path)), { recursive: true }); copyFileSync(resolve(from, path), resolve(to, path));
    }
  };
  try {
    // Construct a fixture snapshot from actual input bytes, even when root HEAD is the baseline.
    git(root, "clone", "--quiet", "--shared", "--no-checkout", root, seed);
    git(seed, "checkout", "--quiet", baseline); overlay(seed); git(seed, "add", "-A");
    const tree = git(seed, "write-tree"), snapshot = git(seed, "commit-tree", tree, "-p", baseline, "-m", "isolated fixture snapshot");
    git(seed, "update-ref", "refs/heads/cross-fixture-snapshot", snapshot);
    expect(snapshot).not.toBe(baseline);
    console.log(JSON.stringify({ fixtureOnly: true, inputHead: git(root, "rev-parse", "HEAD"), inputTree: git(root, "rev-parse", "HEAD^{tree}"),
      inputSourceHash: actual.sha256, inputCount: Object.keys(actual.files).length, fixtureSnapshot: snapshot, fixtureTree: tree, baseline, baselineTree }));

    for (const [index, actorRevision, integratedRevision] of [[0, snapshot, baseline], [1, baseline, snapshot]] as const) {
      const a = resolve(base, `actor-${index}`), b = resolve(base, `integrated-${index}`);
      for (const dir of [a, b]) git(seed, "clone", "--quiet", "--shared", "--no-checkout", seed, dir);
      git(a, "checkout", "--quiet", actorRevision); git(b, "checkout", "--quiet", integratedRevision);
      overlay(a); overlay(b);
      git(b, "config", "diff.noprefix", "true"); git(b, "config", "diff.context", "5");
      const untracked = "scripts/harness/cross-workspace-untracked-fixture.ts";
      for (const dir of [a, b]) writeFileSync(resolve(dir, untracked), "fixture-only input\n");
      git(b, "add", "--", untracked);
      expect(git(a, "status", "--short", "--", untracked)).toStartWith("??");
      expect(git(b, "status", "--short", "--", untracked)).toStartWith("A");
      const captured = await capturePending(["bun", "--version"], `cross-fixture-${index}`, "target/exec-fail-open/pending", "backend", a,
        () => ({ ...context(a), fixtureOnly: true }));
      expect(captured.receipt.exit).toBe(0);
      expect(() => validateCommand(captured.receipt, context(a), a)).toThrow("fixture evidence is inadmissible");
      copyOutput(a, b, captured.receipt);
      // The saved capture remains fixture-only; only the in-memory predicate probe removes the marker.
      const receipt = { ...captured.receipt, fixtureOnly: undefined }, ctx = context(b);
      expect(ctx.observedGit.head).not.toBe(receipt.observedGit.head);
      expect(ctx.observedGit.tree).not.toBe(receipt.observedGit.tree);
      expect(ctx.sourceHash).toBe(receipt.sourceHash);
      expect(ctx.observedGit.baselineInputsSha256).toBe(receipt.observedGit.baselineInputsSha256);
      expect(() => validateCommand(receipt, context(a), a)).not.toThrow();
      expect(() => validateCommand(receipt, ctx, b)).not.toThrow();

      // Optional inputs must distinguish an absent path from any present non-file.
      const optional = ".cargo/config", optionalA = resolve(a, optional), optionalB = resolve(b, optional);
      expect(f.optionalIfPresent).toContain(optional);
      for (const file of [optionalA, optionalB]) if (lstatSync(file, { throwIfNoEntry: false })) unlinkSync(file);
      expect(lstatSync(optionalA, { throwIfNoEntry: false })).toBeUndefined();
      expect(lstatSync(optionalB, { throwIfNoEntry: false })).toBeUndefined();
      const absent = await capturePending(["bun", "--version"], `cross-absent-${index}`, "target/exec-fail-open/pending", "backend", a,
        () => ({ ...context(a), fixtureOnly: true }));
      copyOutput(a, b, absent.receipt);
      const absentReceipt = { ...absent.receipt, fixtureOnly: undefined };
      expect(absentReceipt.currentInputs[optional]).toBeUndefined();
      expect(() => validateCommand(absentReceipt, context(b), b)).not.toThrow();
      mkdirSync(dirname(optionalA), { recursive: true }); mkdirSync(dirname(optionalB), { recursive: true });
      symlinkSync("missing-config-target", optionalB);
      expect(() => validateCommand(absentReceipt, context(b), b)).toThrow(`nonregular artifact: ${optional}`);
      unlinkSync(optionalB);
      mkdirSync(optionalB);
      expect(() => validateCommand(absentReceipt, context(b), b)).toThrow(`nonregular artifact: ${optional}`);
      rmSync(optionalB, { recursive: true });
      expect(() => validateCommand(absentReceipt, context(b), b)).not.toThrow();

      for (const file of [optionalA, optionalB]) { writeFileSync(file, "fixture config\n"); chmodSync(file, 0o640); }
      git(b, "add", "--", optional);
      const regular = await capturePending(["bun", "--version"], `cross-optional-${index}`, "target/exec-fail-open/pending", "backend", a,
        () => ({ ...context(a), fixtureOnly: true }));
      copyOutput(a, b, regular.receipt);
      expect(regular.receipt.currentInputs[optional]?.mode).toBe(0o640);
      expect(regular.receipt.currentInputs[optional]?.sha256).toBe(hash(Buffer.from("fixture config\n")));
      const regularReceipt = { ...regular.receipt, fixtureOnly: undefined };
      expect(() => validateCommand(absentReceipt, context(b), b)).toThrow("stale receipt inputs/git");
      expect(() => validateCommand(regularReceipt, context(b), b)).not.toThrow();
      chmodSync(optionalB, 0o600); expect(() => validateCommand(regularReceipt, context(b), b)).toThrow("stale receipt inputs/git");
      chmodSync(optionalB, 0o640); writeFileSync(optionalB, "different config\n");
      expect(() => validateCommand(regularReceipt, context(b), b)).toThrow("stale receipt inputs/git");
      git(b, "reset", "--quiet", "--", optional);
      unlinkSync(optionalA); unlinkSync(optionalB);
      expect(() => validateCommand(absentReceipt, context(b), b)).not.toThrow();
      if (actual.files[optional]) for (const file of [optionalA, optionalB]) {
        copyFileSync(resolve(root, optional), file); chmodSync(file, actual.files[optional].mode);
      }
      expect(() => validateCommand(receipt, context(b), b)).not.toThrow();

      // Reverse which checkout stages the same new file and capture a fresh receipt.
      git(b, "reset", "--quiet", "--", untracked); git(a, "add", "--", untracked);
      expect(git(a, "status", "--short", "--", untracked)).toStartWith("A");
      expect(git(b, "status", "--short", "--", untracked)).toStartWith("??");
      const reverse = await capturePending(["bun", "--version"], `cross-reverse-${index}`, "target/exec-fail-open/pending", "backend", a,
        () => ({ ...context(a), fixtureOnly: true }));
      copyOutput(a, b, reverse.receipt);
      expect(() => validateCommand({ ...reverse.receipt, fixtureOnly: undefined }, context(b), b)).not.toThrow();

      const failed = await capturePending(["bun", "-e", "process.exit(7)"], `cross-failed-${index}`, "target/exec-fail-open/pending", "backend", a,
        () => ({ ...context(a), fixtureOnly: true }));
      expect(failed.receipt.exit).toBe(7); copyOutput(a, b, failed.receipt);
      const failedReceipt = { ...failed.receipt, fixtureOnly: undefined }, recipe = productionRecipe(b);
      expect(() => validateFailedAttempt(b, recipe, context(b), failedReceipt)).not.toThrow();
      const wrongOrigin = { ...failedReceipt.observedGit, originInputsSha256: "0".repeat(64) };
      expect(() => validateFailedAttempt(b, recipe, context(b), { ...failedReceipt, observedGit: wrongOrigin, observedGitAfter: wrongOrigin })).toThrow();

      const reject = (patch: any) => expect(() => validateCommand({ ...receipt, ...patch }, context(b), b)).toThrow();
      reject({ baselineHead: "0".repeat(40) }); reject({ baselineTree: "0".repeat(40) });
      for (const observedGit of [
        { ...receipt.observedGit, baselineInputsSha256: "0".repeat(64) },
        { ...receipt.observedGit, head: "0".repeat(40) },
        { ...receipt.observedGit, tree: "0".repeat(40) },
        { ...receipt.observedGit, head: integratedRevision, tree: ctx.observedGit.tree },
        { ...receipt.observedGit, originInputsSha256: "0".repeat(64) },
        { ...receipt.observedGit, headDiffSha256: "0".repeat(64) },
      ]) reject({ observedGit, observedGitAfter: observedGit });
      reject({ actorId: "reviewer" }); reject({ skill: "oma-qa" });
      reject({ sourceHashAfter: "0".repeat(64) }); reject({ host: { ...ctx.host, hostname: "other" } });
      reject({ executable: { ...receipt.executable, sha256: "0".repeat(64) } });
      reject({ stdoutSha256: "0".repeat(64) });
      const tracked = resolve(b, "scripts/harness/exec-fail-open-graph.ts"), original = readFileSync(tracked), mode = lstatSync(tracked).mode & 0o777;
      writeFileSync(tracked, Buffer.concat([original, Buffer.from("\n// divergent\n")]));
      expect(() => validateCommand(receipt, context(b), b)).toThrow(); writeFileSync(tracked, original);
      chmodSync(tracked, mode ^ 0o111); expect(() => validateCommand(receipt, context(b), b)).toThrow(); chmodSync(tracked, mode);
      const extra = resolve(b, untracked);
      writeFileSync(extra, "different fixture input\n"); expect(() => validateCommand(receipt, context(b), b)).toThrow();
      unlinkSync(extra); expect(() => validateCommand(receipt, context(b), b)).toThrow();
      symlinkSync(tracked, extra); expect(() => validateCommand(receipt, context(b), b)).toThrow(); unlinkSync(extra);
      writeFileSync(extra, "fixture-only input\n"); expect(() => validateCommand(receipt, context(b), b)).not.toThrow();
      const contract = resolve(b, directory, "contract.json"), saved = readFileSync(contract);
      unlinkSync(contract); expect(() => validateCommand(receipt, context(b), b)).toThrow(); writeFileSync(contract, saved);
      expect(() => validateCommand(receipt, context(b), b)).not.toThrow();

      // A deletion from a recursive scope is also independent of its index state.
      const removed = "crates/arc-cli/src/exec.rs";
      unlinkSync(resolve(a, removed)); unlinkSync(resolve(b, removed)); git(b, "add", "--", removed);
      const deletion = await capturePending(["bun", "--version"], `cross-deletion-${index}`, "target/exec-fail-open/pending", "backend", a,
        () => ({ ...context(a), fixtureOnly: true }));
      copyOutput(a, b, deletion.receipt);
      expect(() => validateCommand({ ...deletion.receipt, fixtureOnly: undefined }, context(b), b)).not.toThrow();
    }
  } finally { rmSync(base, { recursive: true, force: true }); }
}, 3600000);
