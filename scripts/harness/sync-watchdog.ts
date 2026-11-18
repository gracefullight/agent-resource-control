import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { groupMembers, identity, processGroup, OwnedMarker } from "./owned-marker";

type Registration = { anchorPid: number; anchorStart: string; supervisor: { pid: number; start: string } };
const registration = (path: string): Registration | undefined => {
  try { return JSON.parse(readFileSync(`${path}.owner`, "utf8")); }
  catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") return undefined; throw error; }
};
const supervisorIdentity = (path: string): Registration["supervisor"] | undefined => {
  try { return JSON.parse(readFileSync(`${path}.supervisor`, "utf8")); }
  catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") return undefined; throw error; }
};

// Called after the independently timed synchronous boundary has stopped the
// supervisor. A recorded marker and start time authorize each signal.
export function recoverOwned(marker: OwnedMarker, statusPath: string) {
  const owner = registration(statusPath), tracked = new Map<number, string>();
  const deadline = performance.now() + 60_000, pause = new Int32Array(new SharedArrayBuffer(4));
  const signalledGroups = new Set<number>();
  const retryGroup = () => {
    if (performance.now() >= deadline) return false;
    Atomics.wait(pause, 0, 0, 5);
    return true;
  };
  for (;;) {
    for (const item of marker.identities()) tracked.set(item.pid, item.start);
    if (owner && identity(owner.anchorPid)?.start === owner.anchorStart) tracked.set(owner.anchorPid, owner.anchorStart);
    for (const [pid, start] of tracked) {
      if (signalledGroups.has(pid) || identity(pid)?.start !== start ||
          processGroup(pid, { expectedStart: start, retry: retryGroup }) !== pid) continue;
      if (identity(pid)?.start !== start) continue;
      try { process.kill(-pid, "SIGKILL"); }
      catch (error) { if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error; }
      signalledGroups.add(pid);
    }
    let remaining = false;
    for (const [pid, start] of tracked) {
      const current = identity(pid);
      if (!current || current.start !== start) { tracked.delete(pid); continue; }
      remaining = true;
      if (current.zombie) continue;
      try { process.kill(pid, "SIGKILL"); }
      catch (error) { if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error; }
    }
    const groupRemains = [...signalledGroups].some(pid => groupMembers(pid).length > 0);
    if (!remaining && !groupRemains) return;
    if (performance.now() >= deadline) throw Error(`owned supervisor recovery timed out: ${[...tracked.keys()]}`);
    Atomics.wait(pause, 0, 0, 5);
  }
}

// Test injection: an independent process terminates the supervisor only after
// its native identity and the child's readiness are both visible.
if (import.meta.main) {
  const statusPath = process.argv[2], ready = process.argv[3];
  const deadline = performance.now() + 120_000;
  const timer = setInterval(() => {
    const supervisor = supervisorIdentity(statusPath);
    if (supervisor && existsSync(ready)) {
      if (identity(supervisor.pid)?.start !== supervisor.start) throw Error("supervisor identity changed");
      writeFileSync(`${statusPath}.outer-watchdog`, "injected after readiness\n");
      process.kill(supervisor.pid, "SIGKILL");
      clearInterval(timer);
    } else if (performance.now() >= deadline) {
      clearInterval(timer);
      process.exitCode = 1;
    }
  }, 5);
}
