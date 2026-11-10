import { prohibitedReasons } from "../coverage/hygiene";
//! Lossless retained bytes. Container digests and entry digests have distinct meanings.
import { createHash } from "node:crypto";
import { lstatSync, readFileSync, realpathSync } from "node:fs";
import { resolve } from "node:path";
const fail = (ok: unknown, message: string): asserts ok => { if (!ok) throw Error(`EXEC_FAIL_OPEN: ${message}`); };
const sha = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
export function safeEntryPath(path: unknown): asserts path is string {
  fail(typeof path === "string" && path.startsWith(".arc-verify/exec-fail-open/") && !/[\\\x00-\x1f\x7f]/.test(path) && !path.split("/").some(p => !p || p === "." || p === ".."), "unsafe entry path/selector");
}
export type Entry = { path: string; size: number; sha256: string; encoding: "base64" | "external"; bytes: string; identity: string; verdict: string; scope?: "current" | "archive" };
export type Container = { schemaVersion: 1; kind: "retained-evidence-container"; purpose: "collection" | "archive" | "repair" | "combined"; entries: Entry[] };
export function retainedEntry(path: string, bytes: Buffer): Entry {
  safeEntryPath(path);
  let value: any; try { value = JSON.parse(bytes.toString()); } catch { value = {}; }
  return { path, size: bytes.length, sha256: sha(bytes), encoding: "base64", bytes: bytes.toString("base64"),
    identity: typeof value.kind === "string" ? value.kind : "original-artifact",
    verdict: String(value.status ?? value.verdict ?? value.passed ?? "unreported") };
}
export function decodeContainer(bytes: Buffer): Container {
  const c = JSON.parse(bytes.toString());
  fail(c?.schemaVersion === 1 && c.kind === "retained-evidence-container" && ["collection", "archive", "repair", "combined"].includes(c.purpose) && Array.isArray(c.entries) && c.entries.length > 0, "malformed retained container");
  const seen = new Set<string>();
  for (const e of c.entries) {
    safeEntryPath(e.path); fail(!seen.has(e.path), "duplicate retained entry"); seen.add(e.path);
    if (c.purpose === "combined") fail(e.scope === "current" || e.scope === "archive", "invalid combined entry scope");
    else fail(e.scope === undefined, "tagged entry requires combined container");
    fail(typeof e.bytes === "string" && typeof e.identity === "string" && e.identity.length > 0 && typeof e.verdict === "string", "malformed entry encoding/identity");
    if (e.encoding === "external") {
      fail(c.purpose === "combined" && e.scope === "archive" && Number.isSafeInteger(e.size) && e.size >= 0 && /^[a-f0-9]{64}$/.test(e.sha256) &&
        e.bytes === `target/exec-fail-open/task13-preserved/archive/${e.sha256}.json`, "unsafe external archive entry");
      continue;
    }
    fail(e.encoding === "base64", "malformed entry encoding/identity");
    const raw = Buffer.from(e.bytes, "base64");
    fail(prohibitedReasons(e.path, raw).length === 0, "prohibited retained bytes");
    fail(raw.toString("base64") === e.bytes && Number.isSafeInteger(e.size) && e.size === raw.length && e.sha256 === sha(raw), "retained entry bytes/digest differ");
    const expected = retainedEntry(e.path, raw);
    fail(e.identity === expected.identity && e.verdict === expected.verdict, "retained identity/verdict differ");
  }
  if (c.purpose === "combined") fail(c.entries.some((e: Entry) => e.scope === "current") && c.entries.some((e: Entry) => e.scope === "archive"), "combined container needs both scopes");
  return c;
}
export function selectedBytes(bytes: Buffer, selector: any, allowArchive = false, root?: string) {
  safeEntryPath(selector?.path);
  fail(typeof selector.sha256 === "string" && /^[a-f0-9]{64}$/.test(selector.sha256) && Object.keys(selector).sort().join() === "path,sha256", "unsafe entry selector");
  const c = decodeContainer(bytes);
  const e = c.entries.find(e => e.path === selector.path);
  fail(e && e.sha256 === selector.sha256, "missing or altered selected entry");
  const archived = c.purpose === "archive" || (c.purpose === "combined" && e.scope === "archive");
  fail(allowArchive ? archived : !archived, allowArchive ? "selection is not archived evidence" : "archived bytes cannot satisfy current evidence");
  if (e.encoding === "base64") return Buffer.from(e.bytes, "base64");
  fail(root, "external archive root required");
  const path = resolve(root, e.bytes), stat = lstatSync(path);
  fail(stat.isFile() && !stat.isSymbolicLink() && realpathSync(path) === resolve(realpathSync(root), e.bytes), "nonregular external archive");
  const raw = readFileSync(path), expected = retainedEntry(e.path, raw);
  fail(raw.length === e.size && sha(raw) === e.sha256 && prohibitedReasons(e.path, raw).length === 0 &&
    e.identity === expected.identity && e.verdict === expected.verdict, "external archive bytes/digest differ");
  return raw;
}
export const referenceIdentity = (ref: any) => JSON.stringify([ref.path, ref.selector?.path ?? null]);
