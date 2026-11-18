import { spawn } from "node:child_process";
import { closeSync, createReadStream, existsSync, renameSync, unwatchFile, watchFile, writeFileSync, writeSync } from "node:fs";
import { resolve } from "node:path";
import type { Writable } from "node:stream";
import { groupMembers, identity, OwnedMarker } from "./owned-marker";

const script = resolve(import.meta.dir, "owned-process.ts");
const cleanupLimit = 60_000;
const markerFd = 63;
type OwnedResult = { exit: number | null; signal: string | null; pid: number; stdout: Buffer; stderr: Buffer; exitMs: number };

// The anchor remains alive in a private process group after the command exits.
// That reserves the group identity until the parent has drained both streams
// and has terminated any redirected descendants still in the group.
function anchor(argv: string[]) {
  const descriptors: (number | "ignore")[] = Array(markerFd + 1).fill("ignore");
  descriptors[0] = 0; descriptors[1] = 1; descriptors[2] = 2; descriptors[markerFd] = markerFd;
  const child = spawn(argv[0], argv.slice(1), { stdio: descriptors });
  let registered = false;
  const pending: object[] = [];
  const send = (value: object) => {
    if (registered) writeSync(3, JSON.stringify(value) + "\n");
    else pending.push(value);
  };
  // Keep the marker if the supervisor dies before recording this anchor.
  // Once the identity is recorded, release it before reporting root exit.
  const registration = createReadStream("", { fd: 4, autoClose: true });
  registration.once("data", () => {
    closeSync(markerFd); registered = true;
    for (const value of pending) send(value);
    pending.length = 0;
    registration.destroy();
  });
  child.on("spawn", () => send({ pid: child.pid }));
  child.on("error", error => send({ error: String(error), exit: null, signal: null }));
  child.on("exit", (exit, signal) => send({ exit, signal }));
  closeSync(0); closeSync(1); closeSync(2);
  setInterval(() => {}, 2 ** 30);
}

export async function runOwned(
  argv: string[], cwd: string, env: Record<string, string | undefined>,
  options: { watchdogMs?: number; signal?: AbortSignal; stdin?: Buffer; onStdout?: (chunk: Buffer) => void;
    markerPath?: string; onAnchor?: (pid: number, start: string) => void } = {},
): Promise<OwnedResult> {
  const start = performance.now();
  const marker = new OwnedMarker(options.markerPath);
  let child;
  try {
    const descriptors: ("pipe" | "ignore" | number)[] = Array(markerFd + 1).fill("ignore");
    descriptors[0] = "pipe"; descriptors[1] = "pipe"; descriptors[2] = "pipe"; descriptors[3] = "pipe"; descriptors[4] = "pipe";
    descriptors[markerFd] = marker.fd;
    child = spawn(process.execPath, [script, "--anchor", JSON.stringify(argv)], {
      cwd, env: { ...env, ARC_TEST_OWNERSHIP: marker.path }, detached: true, stdio: descriptors,
    });
  } catch (error) { if (!options.markerPath) marker.remove(); throw error; }
  finally { marker.closeParent(); }
  const anchorPid = child.pid;
  if (!anchorPid) {
    if (!options.markerPath) marker.remove();
    throw Error(`cannot start owned supervisor: ${argv[0]}`);
  }
  const stdout: Buffer[] = [], stderr: Buffer[] = [];
  let rootPid = 0, exit: number | null = null, signal: string | null = null, exitMs = 0;
  let rootDone = false, stdoutDone = false, stderrDone = false, closed = false, settled = false, cleanupVerified = false;
  let anchorExited = false, anchorStart: string | undefined;
  let reason: "complete" | "watchdog" | "error" | undefined;
  let failure = "", protocol = "";
  let cleanupTimer: ReturnType<typeof setTimeout> | undefined;
  let poll: ReturnType<typeof setTimeout> | undefined;
  const descendants = new Map<number, string>();
  let finish!: (value: OwnedResult) => void, reject!: (error: Error) => void;
  const result = new Promise<OwnedResult>((resolveResult, rejectResult) => { finish = resolveResult; reject = rejectResult; });
  const stopDescendants = () => {
    for (const entry of marker.identities()) if (entry.pid !== anchorPid) descendants.set(entry.pid, entry.start);
    let remaining = false;
    for (const [pid, start] of descendants) {
      const current = identity(pid);
      if (!current || current.start !== start) { descendants.delete(pid); continue; }
      remaining = true;
      if (current.zombie) continue;
      try { process.kill(pid, "SIGKILL"); }
      catch (error) { if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error; }
    }
    return remaining;
  };
  const terminate = (why: "complete" | "watchdog" | "error") => {
    if (reason) return;
    reason = why;
    try { stopDescendants(); } catch (error) { failure += ` descendant termination: ${error}`; }
    // Never signal a numeric process group after its anchor has disappeared.
    try {
      const anchor = anchorStart ? identity(anchorPid) : undefined;
      if (!anchorExited && anchor && !anchor.zombie && anchor.start === anchorStart) {
        try { process.kill(-anchorPid, "SIGKILL"); }
        catch (error) { if ((error as NodeJS.ErrnoException).code !== "ESRCH") failure += ` group termination: ${error}`; }
      }
    } catch (error) { failure += ` anchor observation: ${error}`; }
    cleanupTimer = setTimeout(() => {
      settled = true;
      child.stdout.destroy(); child.stderr.destroy(); child.stdio[3]?.destroy();
      reject(Error(`owned child cleanup exceeded ${cleanupLimit} ms; marker retained at ${marker.path}: ${failure}`));
    }, cleanupLimit);
    checkGone();
  };
  const maybeComplete = () => {
    if (rootDone && stdoutDone && stderrDone && !reason) terminate(failure ? "error" : "complete");
  };
  const checkGone = () => {
    if (settled || !reason) return;
    if (poll) clearTimeout(poll);
    let remaining = false, group = false;
    try {
      process.kill(-anchorPid, 0);
      group = true;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ESRCH") {
        try { group = groupMembers(anchorPid).length > 0; }
        catch (observation) { failure += ` group observation: ${observation}`; group = true; }
      }
    }
    try { remaining = stopDescendants(); }
    catch (error) { failure += ` descendant observation: ${error}`; remaining = true; }
    if (!closed || group || remaining) { poll = setTimeout(checkGone, 5); return; }
    if (cleanupTimer) clearTimeout(cleanupTimer);
    settled = true; cleanupVerified = true;
    const captured = { exit, signal, pid: rootPid, stdout: Buffer.concat(stdout), stderr: Buffer.concat(stderr), exitMs };
    if (reason === "watchdog") reject(Object.assign(Error(`owned child watchdog expired for pid ${rootPid || anchorPid}: ${failure}`), captured));
    else if (reason === "error" || failure) reject(Object.assign(Error(`owned child failed: ${failure}`), captured));
    else finish(captured);
  };
  child.stdout.on("data", data => {
    const bytes = Buffer.from(data); stdout.push(bytes);
    try { options.onStdout?.(bytes); }
    catch (error) { failure += ` output observer: ${error}`; terminate("error"); }
  });
  child.stderr.on("data", data => stderr.push(Buffer.from(data)));
  child.stdout.on("end", () => { stdoutDone = true; maybeComplete(); });
  child.stderr.on("end", () => { stderrDone = true; maybeComplete(); });
  const control = child.stdio[3]!;
  control.on("data", data => {
    protocol += data.toString();
    while (protocol.includes("\n")) {
      const end = protocol.indexOf("\n"), line = protocol.slice(0, end); protocol = protocol.slice(end + 1);
      try {
        const message = JSON.parse(line);
        if (message.pid) rootPid = message.pid;
        if ("exit" in message) {
          exit = message.exit; signal = message.signal; rootDone = true; exitMs = performance.now() - start;
          if (message.error) failure += message.error;
        }
      } catch (error) { failure += ` invalid child status: ${error}`; rootDone = true; }
    }
    maybeComplete();
  });
  child.on("error", error => { failure += String(error); terminate("error"); });
  child.on("exit", () => {
    anchorExited = true;
    if (!reason) { failure += " owned anchor exited unexpectedly"; terminate("error"); }
  });
  child.on("close", () => {
    closed = true;
    if (!reason) {
      // The group anchor is already gone; its numeric ID is no longer safe to signal.
      settled = true;
      reject(Error(`owned supervisor exited before child completion; group cleanup could not be verified; marker retained at ${marker.path}`));
      return;
    }
    checkGone();
  });
  child.stdin.on("error", error => { if ((error as NodeJS.ErrnoException).code !== "EPIPE") { failure += String(error); terminate("error"); } });
  child.stdin.end(options.stdin);
  const expire = () => terminate("watchdog");
  const watchdog = setTimeout(expire, options.watchdogMs ?? 120_000);
  options.signal?.addEventListener("abort", expire, { once: true });
  const registration = child.stdio[4] as Writable;
  registration.on("error", error => { if (!reason) { failure += ` registration channel: ${error}`; terminate("error"); } });
  try {
    const anchor = identity(anchorPid);
    if (!anchor) throw Error("anchor disappeared before identity was recorded");
    anchorStart = anchor.start;
    options.onAnchor?.(anchor.pid, anchor.start);
    registration.end("R");
  } catch (error) { failure += ` supervisor registration: ${error}`; terminate("error"); }
  if (options.signal?.aborted) expire();
  try { return await result; }
  finally {
    clearTimeout(watchdog); if (poll) clearTimeout(poll);
    options.signal?.removeEventListener("abort", expire);
    if (!options.markerPath && cleanupVerified) marker.remove();
  }
}

async function syncMain(argv: string[], statusPath: string, forceWatchdogOn: string, markerPath: string) {
  const supervisor = identity(process.pid);
  if (!supervisor) throw Error("supervisor identity unavailable");
  writeFileSync(`${statusPath}.supervisor.pending`, JSON.stringify(supervisor));
  renameSync(`${statusPath}.supervisor.pending`, `${statusPath}.supervisor`);
  const controller = new AbortController();
  const check = () => { if (forceWatchdogOn && existsSync(forceWatchdogOn)) controller.abort(); };
  if (forceWatchdogOn) { watchFile(forceWatchdogOn, { interval: 5 }, check); check(); }
  const input = await Bun.stdin.arrayBuffer();
  try {
    const result = await runOwned(argv, process.cwd(), process.env, {
      stdin: Buffer.from(input), signal: controller.signal, markerPath,
      onAnchor: (anchorPid, anchorStart) => {
        // Readiness-driven injection for the independent recovery regression.
        if (existsSync(`${statusPath}.hold-registration`)) {
          const wait = new Int32Array(new SharedArrayBuffer(4));
          while (true) Atomics.wait(wait, 0, 0, 1_000);
        }
        const pending = `${statusPath}.owner.pending`;
        writeFileSync(pending, JSON.stringify({ anchorPid, anchorStart, supervisor }));
        renameSync(pending, `${statusPath}.owner`);
      },
    });
    process.stdout.write(result.stdout); process.stderr.write(result.stderr);
    process.exitCode = result.exit ?? 128;
    writeFileSync(statusPath, JSON.stringify({ completed: true }));
    if (existsSync(`${statusPath}.hold-after-completion`)) {
      const wait = new Int32Array(new SharedArrayBuffer(4));
      while (true) Atomics.wait(wait, 0, 0, 1_000);
    }
  } catch (error) {
    const result = error as Error & { stdout?: Buffer; stderr?: Buffer };
    if (result.stdout) process.stdout.write(result.stdout);
    if (result.stderr) process.stderr.write(result.stderr);
    writeFileSync(statusPath, JSON.stringify({ completed: false, error: result.message }));
    process.stderr.write(`${result.message}\n`); process.exitCode = 124;
  } finally { if (forceWatchdogOn) unwatchFile(forceWatchdogOn, check); }
}

if (import.meta.main) {
  const mode = process.argv[2], argv = JSON.parse(process.argv[3]);
  if (mode === "--anchor") anchor(argv);
  else if (mode === "--sync") await syncMain(argv, process.argv[4], process.argv[5], process.argv[6]);
  else throw Error("unknown owned-process mode");
}
