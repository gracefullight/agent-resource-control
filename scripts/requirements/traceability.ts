/** SRS ID completeness/uniqueness and per-inventory count arithmetic.
 * Evidence, cross-reference prose, cycle labels and status truth are reviewed by QA.
 */
import { readFileSync } from "node:fs";

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
export function validateTraceability(srs: string, markdown: string) {
  const inventory = extractInventory(srs);
  const parsed = parseMatrix(markdown);
  const errors = [...parsed.errors];
  const duplicateIds: string[] = [];
  const seen = new Set<string>();
  const phaseCounts = newPhaseCounts();
  const declarations = new Map(INVENTORIES.flatMap((kind) => inventory[kind].map((d) => [d.id, d] as const)));
  for (const row of parsed.rows) {
    const problems: string[] = [];
    if (seen.has(row.id)) duplicateIds.push(row.id);
    seen.add(row.id);
    const declaration = declarations.get(row.id);
    if (!declaration || declaration.inventory !== row.inventory) problems.push("unknown ID, collapsed range or wrong inventory");
    const phase = row.cells.Phase ?? "";
    const status = row.cells.Status as Status;
    if (problems.length) errors.push(`${row.id} line ${row.line}: ${problems.join("; ")}`);
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
  const valid = phaseCountsReconciled && errors.length === 0 && duplicateIds.length === 0 && missingRequirementIds.length === 0 && missingAcceptanceTestIds.length === 0 && missingCatalogIds.length === 0;
  return { valid, missingRequirementIds, missingAcceptanceTestIds, missingCatalogIds, duplicateIds: [...new Set(duplicateIds)], phaseCountsReconciled, phaseCounts, errors, sourceCounts: Object.fromEntries(INVENTORIES.map((kind) => [kind, inventory[kind].length])) };
}
if (import.meta.main) {
  const srs = readFileSync(SRS_PATH, "utf8");
  if (process.argv.includes("--inventory")) console.log(JSON.stringify(extractInventory(srs), null, 2));
  else { const report = validateTraceability(srs, readFileSync(MATRIX_PATH, "utf8")); console.log(JSON.stringify(report, null, 2)); process.exitCode = report.valid ? 0 : 1; }
}
