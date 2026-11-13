/** SRS declaration and matrix contract for the dedicated requirements-docs Actor.
 * Primary tables: Requirement ID | Acceptance-test ID | Phase | Status | Evidence | Cycle;
 * Acceptance-test ID | Requirement ID | Phase | Status | Evidence | Cycle;
 * Catalog ID | Catalog | Phase | Status | Evidence | Cycle.
 * Summary: Inventory | Phase | Total | Complete | Partial | Pending | Blocked,
 * where Inventory is Requirements, Acceptance or Catalogs (all six phases each).
 * Pending/Blocked evidence may be `Pending: reason` / `Blocked: reason`.
 * Other evidence uses repo-relative Markdown links, optionally #test-selector
 * or #/JSON/pointer. Complete needs executed test/receipt evidence. This slice
 * cannot complete broader vendor, Sweeper, learning or cross-platform work.
 * Cross-references resolve in the named inventory, or use None with a reason.
 */
import { existsSync, readFileSync } from "node:fs";
import { isAbsolute, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";

export const SRS_PATH = "docs/requirements/ARC_SRS_2026-11-06.md";
export const MATRIX_PATH = "docs/requirements/traceability.md";
export const STATUSES = ["Complete", "Partial", "Pending", "Blocked"] as const;
export const INVENTORIES = ["Requirements", "Acceptance", "Catalogs"] as const;
export type Status = (typeof STATUSES)[number];
export type InventoryName = (typeof INVENTORIES)[number];
export type Declaration = { id: string; inventory: InventoryName; catalog: string; line: number; text: string };
export type Inventory = Record<InventoryName, Declaration[]>;
export type MatrixRow = { id: string; inventory: InventoryName; line: number; cells: Record<string, string> };
export type Counts = { Total: number } & Record<Status, number>;
export type PhaseCounts = Record<InventoryName, Record<string, Counts>>;
export type FileAccess = { exists(path: string): boolean; read(path: string): string };
const requirementFamilies = new Set(["VND", "DET", "TRF", "MTR", "POL", "SWP", "SAFE", "NFR"]);
const idPattern = /^(?:[A-Z]+-){1,2}\d{2,3}$/;
const clean = (value: string) => value.trim().replace(/^`([^`]+)`$/, "$1").replace(/^\*\*([^*]+)\*\*$/, "$1");
function cells(line: string): string[] | null {
  if (!/^\s*\|.*\|\s*$/.test(line)) return null;
  return line.trim().slice(1, -1).split(/(?<!\\)\|/).map((cell) => clean(cell.replace(/\\\|/g, "|")));
}
function outsideFences(text: string, visit: (line: string, number: number) => void) {
  let fence: string | undefined;
  text.split(/\r?\n/).forEach((line, index) => {
    const marker = /^\s*(`{3,}|~{3,})/.exec(line)?.[1];
    if (marker) {
      if (!fence) fence = marker;
      else if (marker[0] === fence[0] && marker.length >= fence.length) fence = undefined;
      return;
    }
    if (!fence) visit(line, index + 1);
  });
}
export function extractInventory(srs: string): Inventory {
  const result: Inventory = { Requirements: [], Acceptance: [], Catalogs: [] };
  const seen = new Set<string>();
  outsideFences(srs, (line, number) => {
    const heading = /^#{1,6}\s+(AT-[A-Z]+-\d{2,3}):\s+(.+)$/.exec(line);
    const table = cells(line);
    const id = heading?.[1] ?? table?.[0];
    if (!id || !idPattern.test(id)) return;
    const family = id.split("-")[0] ?? "";
    const inventory: InventoryName = heading ? "Acceptance" : requirementFamilies.has(family) ? "Requirements" : "Catalogs";
    if (seen.has(id)) throw new Error(`Duplicate SRS declaration ${id} at line ${number}`);
    seen.add(id);
    result[inventory].push({ id, inventory, catalog: family, line: number, text: heading?.[2] ?? table?.slice(1).join(" | ") ?? "" });
  });
  if (INVENTORIES.some((kind) => result[kind].length === 0)) throw new Error("SRS lacks a declared requirements, acceptance or other catalog inventory");
  return result;
}
export function parseMatrix(markdown: string) {
  const rows: MatrixRow[] = [];
  const summaries: { line: number; cells: Record<string, string> }[] = [];
  const errors: string[] = [];
  let header: string[] | undefined;
  let kind: InventoryName | "Summary" | undefined;
  outsideFences(markdown, (line, number) => {
    const row = cells(line);
    if (!row) { header = undefined; kind = undefined; return; }
    const first = row[0];
    const newKind = first === "Requirement ID" ? "Requirements" : first === "Acceptance-test ID" ? "Acceptance" : first === "Catalog ID" ? "Catalogs" : first === "Inventory" ? "Summary" : undefined;
    if (newKind) { header = row; kind = newKind; return; }
    if (!kind || !header || row.every((cell) => /^:?-{3,}:?$/.test(cell))) return;
    if (header.length !== row.length) errors.push(`line ${number}: expected ${header.length} cells, got ${row.length}`);
    const data = Object.fromEntries(header.map((key, i) => [key, row[i] ?? ""]));
    if (kind === "Summary") summaries.push({ line: number, cells: data });
    else rows.push({ id: clean(row[0] ?? ""), inventory: kind, line: number, cells: data });
  });
  return { rows, summaries, errors };
}
function newCounts(): Counts { return { Total: 0, Complete: 0, Partial: 0, Pending: 0, Blocked: 0 }; }
function newPhaseCounts(): PhaseCounts {
  return Object.fromEntries(INVENTORIES.map((kind) => [kind, Object.fromEntries(Array.from({ length: 6 }, (_, i) => [`Phase ${i}`, newCounts()]))])) as PhaseCounts;
}
const evidenceReference = /\[([^\]]*)\]\(([^\s)]+)\)|`([^`\s]+\.[a-z]+(?:#[^`\s]+)?)`/g;
function evidenceLinks(text: string) {
  return [...text.matchAll(evidenceReference)].map((m) => m[2] ?? m[3] ?? "");
}
function jsonPointer(value: unknown, pointer: string): unknown {
  return pointer.split("/").slice(1).reduce<unknown>((current, part) => current && typeof current === "object" ? (current as Record<string, unknown>)[part.replace(/~1/g, "/").replace(/~0/g, "~")] : undefined, value);
}
function failedReceipt(value: unknown): boolean {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const result = value as Record<string, unknown>;
  return result.executed === false || result.passed === false || result.pass === false || result.executedAndPassed === false
    || (Object.hasOwn(result, "exit") && result.exit !== 0)
    || (Object.hasOwn(result, "exitCode") && result.exitCode !== 0);
}
function passingReceipt(value: unknown): boolean {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const result = value as Record<string, unknown>;
  if (failedReceipt(result)) return false;
  if (result.executedAndPassed === true || result.passed === true || result.pass === true) return true;
  // Maintained command receipts use argv with exit (Governor) or exitCode
  // (predecessor gates). An arbitrary zero or a receipt path is not execution.
  return Array.isArray(result.argv) && result.argv.length > 0 && result.argv.every((arg) => typeof arg === "string")
    && (Object.hasOwn(result, "exit") ? result.exit : result.exitCode) === 0;
}
function executedEvidence(link: string, access: FileAccess): boolean {
  const [path = "", fragment] = link.split("#", 2);
  if (!/^\.arc-verify\/.*\.json$/.test(path) || !access.exists(path) || (fragment && !fragment.startsWith("/"))) return false;
  try {
    const receipt: unknown = JSON.parse(access.read(path));
    const selected = fragment ? jsonPointer(receipt, fragment) : receipt;
    const containing = fragment ? jsonPointer(receipt, fragment.slice(0, fragment.lastIndexOf("/"))) : undefined;
    // A selected success flag or exit cannot override its command/case failure.
    if (failedReceipt(selected) || failedReceipt(containing)) return false;
    // Only the selected result/case counts; unrelated passing siblings cannot
    // turn a false/null result into successful evidence.
    if (selected === true || passingReceipt(selected)) return true;
    if (selected === 0 && fragment && /\/(?:exit|exitCode)$/.test(fragment))
      return passingReceipt(jsonPointer(receipt, fragment.slice(0, fragment.lastIndexOf("/"))));
  } catch { /* Invalid or unreadable receipts cannot establish execution. */ }
  return false;
}
// Known broader obligations whose full acceptance this macOS Governor cycle
// does not execute. Historical cycles must carry their own concrete receipts.
function broaderThanGovernor(id: string) {
  return /^(?:VND|SWP|POL)-|^AT-(?:SWP|PLG|POL)-/.test(id) || new Set([
    "DET-002", "DET-003", "DET-009", "DET-010", "DET-011", "DET-013", "DET-016",
    "TRF-010", "TRF-012", "TRF-013", "TRF-014", "TRF-016", "TRF-017", "TRF-019", "TRF-020", "TRF-021", "TRF-022",
    "NFR-008", "NFR-015", "NFR-016", "NFR-017", "NFR-020", "NFR-024", "NFR-028",
    "G-03", "G-04", "G-05", "G-06", "G-07", "G-10",
    "DEC-002", "DEC-004", "DEC-014", "DEC-015", "DEC-016", "DEC-017", "DEC-018", "DEC-021", "DEC-025", "DEC-026", "DEC-027",
    "ADR-006", "ADR-007", "ADR-010", "ADR-012", "ADR-013", "ADR-015", "ADR-016", "ADR-017",
    "AT-TRF-01", "AT-TRF-03", "AT-TRF-04", "AT-TRF-08", "AT-TRF-12", "AT-GUI-02",
  ]).has(id);
}
export function validateTraceability(srs: string, markdown: string, access: FileAccess) {
  const inventory = extractInventory(srs);
  const parsed = parseMatrix(markdown);
  const errors = [...parsed.errors];
  const duplicateIds: string[] = [];
  const seen = new Set<string>();
  const phaseCounts = newPhaseCounts();
  const declarations = new Map(INVENTORIES.flatMap((kind) => inventory[kind].map((d) => [d.id, d] as const)));
  let fieldsValid = true;
  for (const row of parsed.rows) {
    const problems: string[] = [];
    if (seen.has(row.id)) duplicateIds.push(row.id);
    seen.add(row.id);
    const declaration = declarations.get(row.id);
    if (!declaration || declaration.inventory !== row.inventory) problems.push("unknown ID, collapsed range or wrong inventory");
    const phase = row.cells.Phase ?? "";
    const status = row.cells.Status as Status;
    const evidence = row.cells.Evidence ?? "";
    const cycle = row.cells.Cycle ?? "";
    if (!/^Phase [0-5]$/.test(phase)) problems.push("one primary Phase 0-5 required (priority is not a phase)");
    if (!STATUSES.includes(status)) problems.push("invalid or missing Status");
    if (!/^[a-z][a-z0-9-]*\d[a-z0-9-]*$/.test(cycle)) problems.push("explicit cycle identifier required");
    if (!evidence.trim()) problems.push("Evidence required");
    const crossColumn = row.inventory === "Requirements" ? "Acceptance-test ID" : row.inventory === "Acceptance" ? "Requirement ID" : "Catalog";
    if (!(row.cells[crossColumn] ?? "").trim()) problems.push(`${crossColumn} required (use None with a reason when unmapped)`);
    if (row.inventory !== "Catalogs") {
      const cross = row.cells[crossColumn] ?? "";
      if (/[~–…]|\d\s*-\s*(?:[A-Z]+-)*\d/.test(cross)) problems.push("collapsed cross-reference range");
      if (/^None\b/i.test(cross)) {
        if (!/[\p{L}\p{N}]/u.test(cross.replace(/^None\b/i, ""))) problems.push(`${crossColumn}: None requires an unmapped reason`);
      } else {
        const expectedInventory = row.inventory === "Requirements" ? "Acceptance" : "Requirements";
        const references = [...cross.matchAll(/\b(?:[A-Z]+-){1,2}\d{2,3}\b/g)];
        if (references.length === 0) problems.push(`${crossColumn} requires declared IDs or None with a reason`);
        for (const match of references) {
          if (declarations.get(match[0])?.inventory !== expectedInventory) problems.push(`cross-reference ${match[0]} must resolve to a ${expectedInventory} declaration`);
        }
      }
    }
    const links = evidenceLinks(evidence);
    const pendingReason = /^(?:Pending|Blocked):\s*\S.{8,}/.test(evidence);
    if (links.length === 0 && !pendingReason) problems.push("concrete repository evidence link or explicit pending/blocker reason required");
    for (const link of links) {
      const [path = "", fragment] = link.split("#", 2);
      if (isAbsolute(path) || path.split(/[\\/]/).includes("..") || /^(?:https?:|file:)/.test(path) || path.startsWith("target/") || path.includes("/target/")) {
        problems.push(`evidence must be Git-visible repository path: ${path}`); continue;
      }
      if (!access.exists(path)) { problems.push(`missing evidence path: ${path}`); continue; }
      if (fragment && (path.endsWith(".json") || /\.(rs|tsx?|jsx?)$/.test(path))) {
        try {
          const source = access.read(path);
          if (path.endsWith(".json") && fragment.startsWith("/") ? jsonPointer(JSON.parse(source), fragment) === undefined : !source.includes(fragment)) problems.push(`missing evidence selector: ${link}`);
        } catch { problems.push(`unreadable evidence: ${link}`); }
      }
    }
    if (status === "Complete") {
      const prose = evidence.replace(evidenceReference, (_reference, label: string | undefined) => label ?? "");
      if (/pending|blocked|unverified|not (?:run|executed|implemented)|unknown|partial/i.test(prose)) problems.push("Complete contradicts pending or limited evidence");
      if (!links.some((link) => executedEvidence(link, access))) problems.push("Complete requires recorded executable receipt evidence with an affirmative executed result, not source/specification alone");
      if (cycle === "governor-2026-11-13" && broaderThanGovernor(row.id)) problems.push("Governor slice cannot complete this broader SRS obligation");
    }
    if (cycle && cycle !== "governor-2026-11-13" && (status === "Complete" || status === "Partial")) {
      const supported = links.some((link) => {
        const path = link.split("#", 1)[0] ?? "";
        if (!/^\.arc-verify\/.*\.(?:json|md)$/.test(path) || !access.exists(path)) return false;
        try { return access.read(path).includes(cycle); } catch { return false; }
      });
      if (!supported) problems.push("historical cycle must be identified by a linked predecessor receipt");
    }
    if (problems.length) { fieldsValid = false; errors.push(`${row.id} line ${row.line}: ${problems.join("; ")}`); }
    const counts = phaseCounts[row.inventory][phase];
    if (declaration?.inventory === row.inventory && counts && STATUSES.includes(status)) { counts.Total++; counts[status]++; }
  }
  const missing = (kind: InventoryName) => inventory[kind].map((d) => d.id).filter((id) => !seen.has(id));
  let phaseCountsReconciled = true;
  const summaryKeys = new Set<string>();
  for (const row of parsed.summaries) {
    const kind = row.cells.Inventory as InventoryName;
    const phase = row.cells.Phase ?? "";
    const key = `${kind}:${phase}`;
    const expected = phaseCounts[kind]?.[phase];
    if (!expected || summaryKeys.has(key)) { phaseCountsReconciled = false; errors.push(`invalid or duplicate phase summary line ${row.line}`); continue; }
    summaryKeys.add(key);
    for (const column of ["Total", ...STATUSES] as const) {
      const actual = row.cells[column] ?? "";
      if (!/^(0|[1-9]\d*)$/.test(actual) || Number(actual) !== expected[column]) { phaseCountsReconciled = false; errors.push(`${key}: ${column} must be ${expected[column]}, got ${actual}`); }
    }
  }
  if (summaryKeys.size !== 18) { phaseCountsReconciled = false; errors.push("Requirements, Acceptance and Catalogs need separate counts for all six phases"); }
  for (const kind of INVENTORIES) if (Object.values(phaseCounts[kind]).reduce((n, c) => n + c.Total, 0) !== inventory[kind].length) phaseCountsReconciled = false;
  const missingRequirementIds = missing("Requirements");
  const missingAcceptanceTestIds = missing("Acceptance");
  const missingCatalogIds = missing("Catalogs");
  const valid = fieldsValid && phaseCountsReconciled && errors.length === 0 && duplicateIds.length === 0 && missingRequirementIds.length === 0 && missingAcceptanceTestIds.length === 0 && missingCatalogIds.length === 0;
  return { valid, missingRequirementIds, missingAcceptanceTestIds, missingCatalogIds, duplicateIds: [...new Set(duplicateIds)], allRowsHaveStatusEvidenceAndCycle: fieldsValid, phaseCountsReconciled, phaseCounts, errors, sourceCounts: Object.fromEntries(INVENTORIES.map((kind) => [kind, inventory[kind].length])) };
}
export function repositoryAccess(root: string): FileAccess {
  const visibility = new Map<string, boolean>();
  const under = (path: string) => { const absolute = resolve(root, path); if (!absolute.startsWith(resolve(root) + sep)) throw new Error("outside repository"); return absolute; };
  return { exists: (path) => {
    if (!visibility.has(path)) visibility.set(path, existsSync(under(path)) && spawnSync("git", ["check-ignore", "--quiet", "--", path], { cwd: root }).status === 1);
    return visibility.get(path) ?? false;
  }, read: (path) => readFileSync(under(path), "utf8") };
}
if (import.meta.main) {
  const srs = readFileSync(SRS_PATH, "utf8");
  if (process.argv.includes("--inventory")) console.log(JSON.stringify(extractInventory(srs), null, 2));
  else { const report = validateTraceability(srs, readFileSync(MATRIX_PATH, "utf8"), repositoryAccess(process.cwd())); console.log(JSON.stringify(report, null, 2)); process.exitCode = report.valid ? 0 : 1; }
}
