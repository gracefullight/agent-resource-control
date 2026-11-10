//! Acceptance reads only predecessor artifacts, even when a future file exists.
import { resolve } from "node:path";
type Phase = "gates" | "evidence";
const active: { root: string; phase: Phase }[] = [];
export function predecessorReads<T>(root: string, phase: Phase, run: () => T): T {
  active.push({ root: resolve(root), phase });
  try { return run(); } finally { active.pop(); }
}
export function checkPredecessorRead(root: string, path: string) {
  for (const guard of active.filter(g => g.root === resolve(root))) {
    const successors = guard.phase === "gates"
      ? ["qa/review.md", "qa/review.json", "final.md", "final.json", "terminal.json"]
      : ["terminal.json"];
    if (successors.some(name => resolve(root, path) === resolve(root, `.arc-verify/exec-fail-open/${name}`)))
      throw Error(`EXEC_FAIL_OPEN: ${guard.phase} attempted successor read: ${path}`);
  }
}
