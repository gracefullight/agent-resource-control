import { test, expect } from "bun:test";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { tmpdir } from "node:os";
import { spawnSync } from "node:child_process";
import { directory, hash, reference } from "./exec-fail-open-inputs";
import { retainedEntry } from "./exec-fail-open-container";
import { seal } from "./exec-fail-open-compact";
import { accessCases, validateAccessRecord, validateAdditionalNative, validateTerminalRecord } from "./exec-fail-open-native-extra";

test("access and failed-target selections bind current bytes, counts and unmeasured timing reasons", () => {
  const root = mkdtempSync(resolve(tmpdir(), "arc-native-evidence-"));
  const git = (...args: string[]) => expect(spawnSync("git", args, { cwd: root }).status).toBe(0);
  try {
    git("init", "--quiet");
    mkdirSync(resolve(root, directory, "implementation"), { recursive: true });
    mkdirSync(resolve(root, "target"), { recursive: true });
    writeFileSync(resolve(root, directory, "contract.json"), "{}\n");
    const binary = resolve(root, "target/candidate"), now = Date.now(); writeFileSync(binary, "fixture candidate");
    const candidateSha256 = hash(readFileSync(binary));
    const access = accessCases.map(([id, name, actions, withheld]) => ({
      scenario: id, test: `connect_access::${name}`, nativeMacos: true, passed: true, invocations: 1, observedExit: 17, recoveryInvocations: 1, permits: 0, arcCleanup: false,
      parentSpawnToInvocationMs: 20, boundary: "parent before wrapper spawn to child handshake", samples: { clock: "std::time::Instant", startNs: 0, endNs: 20_000_000 },
      metadataElapsedMs: null, handoffMs: null, startupMs: null, subdivisionReason: "Internal intervals were not measured.",
      wrapperPid: 10, childPid: 11, childParentPid: 10, candidateBinaryPath: binary, candidateSha256, recordedUnixMs: now,
      // serde_json emits map keys in this order; key order is not evidence.
      proxy: { nativeErrno: 13, actions, commits: actions.map((action, i) => ({ action, childAttached: false, claimed: action === "CLAIM", revision: i + 1 })), ...(actions.length ? { ackWithheld: withheld } : {}) },
    }));
    const terminal = { scenario: "terminal", test: "fallback_terminal::native_pty_spawn_failure_restores_foreground_without_child_output", nativeMacos: true, passed: true, invocations: 0, candidateBinaryPath: binary, candidateSha256, recordedUnixMs: now };
    const workspace = { argv: ["cargo", "test", "--workspace"], started: new Date(now - 1000).toISOString(), ended: new Date(now + 1000).toISOString(), execution: { tests: [...access, terminal].map(r => ({ name: r.test, status: "ok" })) } };
    const commandPath = `${directory}/implementation/commands/workspace.json`, containerPath = `${directory}/implementation/collection.json`;
    const entries = [retainedEntry(commandPath, Buffer.from(JSON.stringify(workspace))), ...access.map(r => retainedEntry(`${directory}/implementation/native/${r.scenario}.json`, Buffer.from(JSON.stringify(r)))), retainedEntry(`${directory}/implementation/native/terminal-2.json`, Buffer.from(JSON.stringify(terminal)))];
    const sealed = seal(root, containerPath, entries); git("add", directory);
    const commandReceipt = sealed.entries[commandPath];
    const timing = { accessCases: access.map(row => ({ id: row.scenario, record: row, observation: sealed.entries[`${directory}/implementation/native/${row.scenario}.json`], commandReceipt })), failedTarget: { record: terminal, observation: sealed.entries[`${directory}/implementation/native/terminal-2.json`], commandReceipt } };
    expect(() => validateAdditionalNative(root, timing, [workspace])).not.toThrow();
    for (const field of ["metadataElapsedMs", "handoffMs", "startupMs", "subdivisionReason"]) {
      const changed: any = { ...access[0] }; delete changed[field];
      expect(() => validateAccessRecord(changed.scenario, changed, changed)).toThrow("unmeasured subdivisions");
    }
    expect(() => validateAccessRecord(access[0].scenario, { ...access[0], invocations: 0 }, { ...access[0], invocations: 0 })).toThrow("invocation evidence");
    const badCommit = { ...access[1], proxy: { ...access[1].proxy, commits: [{ ...access[1].proxy.commits[0], revision: 99 }] } };
    expect(() => validateAccessRecord(badCommit.scenario, badCommit, badCommit)).toThrow("commit evidence differs");
    expect(() => validateTerminalRecord({ ...terminal, invocations: 1 }, { ...terminal, invocations: 1 })).toThrow("failed target invocation");
    const container = JSON.parse(readFileSync(resolve(root, containerPath), "utf8")), changed = container.entries.find((e: any) => e.path.endsWith(`${access[0].scenario}.json`));
    Object.assign(changed, retainedEntry(changed.path, Buffer.from(JSON.stringify({ ...access[0], wrapperPid: 99, childParentPid: 99 }))));
    writeFileSync(resolve(root, containerPath), JSON.stringify(container)); git("add", directory);
    const newRef = reference(root, containerPath), rebind = (r: any) => ({ ...newRef, selector: { ...r.selector, ...(r.selector.path === changed.path ? { sha256: changed.sha256 } : {}) } });
    const altered = { accessCases: timing.accessCases.map(c => ({ ...c, observation: rebind(c.observation), commandReceipt: rebind(c.commandReceipt) })), failedTarget: { ...timing.failedTarget, observation: rebind(timing.failedTarget.observation), commandReceipt: rebind(commandReceipt) } };
    expect(() => validateAdditionalNative(root, altered, [workspace])).toThrow("access observation differs from selected bytes");
  } finally { rmSync(root, { recursive: true, force: true }); }
});
