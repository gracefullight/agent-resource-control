import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { requireValue, directory } from "./exec-fail-open-inputs";
export function checkDocs(root: string, supplied?: { main: string; doc: string; exec: string; copy: string; claims: string }) {
  const input = supplied ?? { main: readFileSync(resolve(root, "crates/arc-cli/src/main.rs"), "utf8"),
    doc: readFileSync(resolve(root, "docs/managed-exec.md"), "utf8"),
    exec: readFileSync(resolve(root, "crates/arc-cli/src/exec.rs"), "utf8"),
    copy: readFileSync(resolve(root, "crates/arc-cli/src/exec/preparation.rs"), "utf8"),
    claims: readFileSync(resolve(root, `${directory}/claims-review.md`), "utf8") };
  const design = readFileSync(resolve(root, `${directory}/design-contract.md`), "utf8");
  const help = design.split("## Exact help amendment")[1].split("```text\n")[1].split("```")[0].trimEnd();
  const oldPromise = "If the daemon is unavailable or tracking cannot be confirmed, ARC still attempts the command once.";
  const oldExclusions = "Cancellation, invalid arguments and explicit authenticated rejection do not start a child.";
  requireValue(help.includes(oldPromise) && help.includes(oldExclusions), "accepted help copy missing amendment lines");
  const correctedHelp = help
    .replace(oldPromise, "If the daemon is unavailable or tracking is unconfirmed due to an outage or exhausted DB_BUSY retries, ARC still attempts the command once.")
    .replace(oldExclusions, "Cancellation, invalid arguments, invalid protocol replies, local preparation failures and explicit authenticated rejection do not start a child.")
    .replace("Use arcctl job status <JOB_ID> for read-only inspection after daemon recovery.", "Use arcctl job status <JOB_ID> to observe a record after daemon recovery; this may update the lifecycle record.");
  // Compare the actual Rust string value after unescaping its two supported escapes.
  const start = input.main.indexOf('#[command(after_help = "Examples:');
  const end = input.main.indexOf('")]\n    Exec', start);
  const actual = input.main.slice(start + '#[command(after_help = "'.length, end).replaceAll("\\\\", "\\").replaceAll('\\"', '"');
  requireValue(start >= 0 && end > start && actual === correctedHelp, "help differs from accepted copy");
  for (const term of ["The daemon is optional for execution", "at most two seconds", "TRACKING_UNAVAILABLE", "TRACKING_UNCERTAIN", "Neither message confirms lifecycle persistence", "may update the lifecycle record", "never replays the command", "A missing lifecycle record does not establish whether the command ran", "never authorizes replay", "does not allocate reservations or initiate ARC cleanup", "non-UTF-8", "working directory", "environment", "descriptors", "signal", "protocol-1", "DB_BUSY", "IPC_ERROR", "PROTOCOL_INCOMPATIBLE"])
    requireValue(input.doc.includes(term), `docs missing ${term}`);
  for (const term of ["desktop", "native Windows", "vendor hooks", "scheduling"])
    requireValue((input.doc + input.claims).includes(term) && input.claims.includes(term), `outstanding scope missing ${term}`);
  requireValue(input.claims.length >= 200 && input.claims.includes("unmeasured"), "claims review incomplete");
  requireValue(!/Start `arc-daemon`, wait|must be confirmed before spawn|tracking is guaranteed|(?:^|\n)(?:The |A )?(?:lifecycle )?record (?:is|was) saved/i.test(input.doc), "stale prerequisite or persistence claim");
  requireValue(input.exec.includes("preparation::started(tracking, &job_id)") && input.exec.indexOf("preparation::started") > input.exec.indexOf("let mut child = match spawned"), "started copy precedes successful spawn");
  requireValue(!/eprintln!\([^;]*(?:argv|error\}|socket\.display|environment|payload)/s.test(input.exec + input.copy), "secret-bearing wrapper diagnostic");
  for (const line of ["Command started. ARC tracking is unavailable. A lifecycle record may be absent.", "Command started. Lifecycle recording is unconfirmed.", "ARC rejected execution. No child was started.", "Execution cancelled. No child was started."])
    requireValue(input.copy.includes(line), "missing state-specific copy");
  return { passed: true };
}
