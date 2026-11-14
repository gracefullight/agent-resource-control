import { describe, expect, test } from "bun:test";
import { extractInventory, INVENTORIES, parseMatrix, validateTraceability } from "./traceability";

const srs = `# Requirements
| ID | Pri | Acceptance |
| --- | --- | --- |
| TRF-005 | P0 | streams preserved |
| SWP-001 | P0 | orphan cleanup |
| Goal ID | Goal |
| --- | --- |
| G-01 | attribution |
### AT-TRF-07: preserves exit
\`\`\`md
| TRF-999 | example only |
### AT-TRF-99: example only
\`\`\`
## References
| G-01 Meter | TRF-001~022 | not declarations |
`;
function matrix() {
  const rows = `| Requirement ID | Acceptance-test ID | Phase | Status | Evidence | Cycle |
| --- | --- | --- | --- | --- | --- |
| TRF-005 | AT-TRF-07 | Phase 1 | Partial | [native](tests/actual.rs#native_exit_is_preserved) | governor-2026-11-13 |
| SWP-001 | None (outside this slice) | Phase 3 | Pending | Pending: Sweeper not implemented by Governor | governor-2026-11-13 |

| Acceptance-test ID | Requirement ID | Phase | Status | Evidence | Cycle |
| --- | --- | --- | --- | --- | --- |
| AT-TRF-07 | TRF-005 | Phase 1 | Complete | [native](tests/actual.rs#native_exit_is_preserved), [receipt](.arc-verify/governor/integration.json#/checks/passed) | governor-2026-11-13 |

| Catalog ID | Catalog | Phase | Status | Evidence | Cycle |
| --- | --- | --- | --- | --- | --- |
| G-01 | G | Phase 1 | Pending | Pending: all attribution remains incomplete | governor-2026-11-13 |

| Inventory | Phase | Total | Complete | Partial | Pending | Blocked |
| --- | --- | --- | --- | --- | --- | --- |
`;
  return rows + INVENTORIES.flatMap((kind) => Array.from({ length: 6 }, (_, phase) => {
    const total = kind === "Requirements" ? Number(phase === 1 || phase === 3) : Number(phase === 1);
    const complete = Number(kind === "Acceptance" && phase === 1);
    const partial = Number(kind === "Requirements" && phase === 1);
    return `| ${kind} | Phase ${phase} | ${total} | ${complete} | ${partial} | ${total - complete - partial} | 0 |`;
  })).join("\n");
}
const check = (text = matrix()) => validateTraceability(srs, text);
describe("SRS ID completeness and count arithmetic", () => {
  test("extracts declarations, not examples, ranges or references", () => {
    const result = extractInventory(srs);
    expect(result.Requirements.map((d) => d.id)).toEqual(["TRF-005", "SWP-001"]);
    expect(result.Acceptance.map((d) => d.id)).toEqual(["AT-TRF-07"]);
    expect(result.Catalogs.map((d) => d.id)).toEqual(["G-01"]);
    expect(result.Requirements[0]?.line).toBe(4);
  });
  test.each(["| TRF-005 | duplicate |", "### AT-TRF-07: duplicate", "| G-01 | duplicate |"])("rejects duplicate declarations: %s", (declaration) => {
    expect(() => extractInventory(`${srs}\n${declaration}`)).toThrow("Duplicate SRS declaration");
  });
  test("accepts complete inventories and separate six-phase arithmetic", () => {
    expect(check()).toMatchObject({ valid: true, missingRequirementIds: [], missingAcceptanceTestIds: [], missingCatalogIds: [], duplicateIds: [], phaseCountsReconciled: true });
    expect(parseMatrix(matrix()).rows).toHaveLength(4);
    expect(check().phaseCounts.Requirements["Phase 1"]?.Total).toBe(1);
  });
  test.each([
    ["| SWP-001 | None", "| SWP-999 | None", "missingRequirementIds", "SWP-001"],
    ["| AT-TRF-07 | TRF-005", "| AT-TRF-99 | TRF-005", "missingAcceptanceTestIds", "AT-TRF-07"],
    ["| G-01 | G", "| G-99 | G", "missingCatalogIds", "G-01"],
  ] as const)("rejects missing primary IDs: %s", (from, to, key, id) => {
    const result = check(matrix().replace(from, to));
    expect(result.valid).toBe(false);
    expect(result[key]).toEqual([id]);
    expect(result.errors.some((error) => error.includes("unknown ID"))).toBe(true);
  });
  test.each(["| TRF-005 |", "| AT-TRF-07 |", "| G-01 |"])("rejects duplicate primary IDs: %s", (prefix) => {
    const row = matrix().split("\n").find((line) => line.startsWith(prefix));
    const result = check(matrix().replace(row!, `${row}\n${row}`));
    expect(result.valid).toBe(false);
    expect(result.duplicateIds).toEqual([prefix.split("|")[1]!.trim()]);
  });
  test.each([
    ["| Phase 1 | Partial |", "| invalid phase | Partial |"],
    ["| Phase 1 | Partial |", "| Phase 1 | uncountable status |"],
    ["| Requirements | Phase 1 | 1 |", "| Requirements | Phase 1 | 2 |"],
    ["| Acceptance | Phase 1 | 1 | 1 |", "| Acceptance | Phase 1 | 1 | 0 |"],
    ["| Catalogs | Phase 1 | 1 | 0 | 0 | 1 |", "| Catalogs | Phase 1 | 1 | 0 | 1 | 0 |"],
    ["| Acceptance | Phase 5 | 0 | 0 | 0 | 0 | 0 |", ""],
    ["| Catalogs | Phase 5 | 0 | 0 | 0 | 0 | 0 |", "| Catalogs | Phase 4 | 0 | 0 | 0 | 0 | 0 |"],
  ])("rejects incorrect, missing or duplicate counts: %s", (from, to) => {
    const result = check(matrix().replace(from!, to!));
    expect(result.valid).toBe(false);
    expect(result.phaseCountsReconciled).toBe(false);
  });
  test.each([
    ["AT-TRF-07 | Phase", "unknown cross reference | Phase"],
    ["None (outside this slice)", ""],
    ["governor-2026-11-13", "arbitrary label"],
    ["[native](tests/actual.rs#native_exit_is_preserved)", "Outcome unknown; only partial execution"],
    [".arc-verify/governor/integration.json#/checks/passed", "https://example.invalid/missing.json#/no-such-selector"],
    ["Pending: Sweeper not implemented by Governor", ""],
    ["G | Phase", "free catalog prose | Phase"],
  ])("accepts irrelevant wording, labels and evidence changes: %s", (from, to) => {
    expect(check(matrix().replaceAll(from!, to!)).valid).toBe(true);
  });
  test("ignores omitted or extra narrative cells when ID phase and status are countable", () => {
    const prose = matrix().replace("| TRF-005 | AT-TRF-07 | Phase 1 | Partial | [native](tests/actual.rs#native_exit_is_preserved) | governor-2026-11-13 |", "| TRF-005 | | Phase 1 | Partial |");
    expect(check(prose).valid).toBe(true);
    expect(check(prose.replace("| TRF-005 | | Phase 1 | Partial |", "| TRF-005 | | Phase 1 | Partial | arbitrary | extra | prose | cells |")).valid).toBe(true);
  });
  test("parses escaped pipes and ignores fenced fake matrices", () => {
    expect(check(`${matrix()}\n\n\`\`\`md\n${matrix()}\n\`\`\``).valid).toBe(true);
    expect(check(matrix().replace("Pending: all attribution remains incomplete", "arbitrary \\| wording")).valid).toBe(true);
  });
});
