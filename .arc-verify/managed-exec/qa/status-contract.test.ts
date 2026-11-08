import { test, expect } from "bun:test";
import { resolve } from "node:path";
test("adopted signaled by-ID status exposes the observed core flag", async () => {
  const root = resolve(import.meta.dir, "../../..");
  const child = Bun.spawn(["python3", resolve(import.meta.dir, "probes.py")], { cwd: root, stdout: "pipe", stderr: "pipe" });
  const timer = setTimeout(() => child.kill("SIGTERM"), 3600000);
  const [exit, stdout, stderr] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
  clearTimeout(timer);
  expect(exit).toBe(0);
  expect(stderr).toBe("");
  const observed = JSON.parse(stdout);
  expect(observed.nativeSignal).toBe(15);
  expect(observed.persistedCoreDumped).toBe(false);
  expect(observed.signaledStatusOutput).toMatch(/core[^\n]*false/i);
}, 3600000);
