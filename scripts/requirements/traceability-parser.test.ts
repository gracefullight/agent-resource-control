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
const fixtureFiles: Record<string, string> = {
  "tests/actual.rs": "fn native_exit_is_preserved() {}\nfn unknown_command_bypasses() {}",
  "tests/unknown_command.rs": "fn unknown_command_bypasses() {}",
  ".arc-verify/governor/contract.md": "# Contract\nThe native test must execute and pass.",
  ".arc-verify/governor/integration.json": JSON.stringify({ checks: {
    passed: true,
    native: { argv: ["cargo", "test", "native_exit_is_preserved"], exit: 0 },
    predecessor: { argv: ["cargo", "test", "native_exit_is_preserved"], exitCode: 0 },
    case: { id: "GV-N01", executedAndPassed: true },
  } }),
};
const access = { exists: (path: string) => Object.hasOwn(fixtureFiles, path), read: (path: string) => fixtureFiles[path] ?? "" };
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
const check = (text = matrix(), files = access) => validateTraceability(srs, text, files);
describe("exact SRS and traceability contract", () => {
  test("extracts declarations, not examples, ranges or references", () => {
    const result = extractInventory(srs);
    expect(result.Requirements.map((d) => d.id)).toEqual(["TRF-005", "SWP-001"]);
    expect(result.Acceptance.map((d) => d.id)).toEqual(["AT-TRF-07"]);
    expect(result.Catalogs.map((d) => d.id)).toEqual(["G-01"]);
    expect(result.Requirements[0]?.line).toBe(4);
    expect(() => extractInventory(`${srs}\n| TRF-005 | duplicate |`)).toThrow("Duplicate");
  });
  test("accepts complete inventories and separate six-phase arithmetic", () => {
    expect(check()).toMatchObject({ valid: true, missingRequirementIds: [], missingAcceptanceTestIds: [], missingCatalogIds: [], duplicateIds: [], phaseCountsReconciled: true });
  });
  test("counts primary IDs only; cross references may repeat", () => {
    expect(parseMatrix(matrix()).rows).toHaveLength(4);
    expect(check().phaseCounts.Requirements["Phase 1"]?.Total).toBe(1);
  });
  test("rejects missing and duplicate primary rows", () => {
    expect(check(matrix().replace("| SWP-001 | None", "| SWP-001~035 | None")).missingRequirementIds).toEqual(["SWP-001"]);
    expect(check(matrix().replace("| SWP-001 | None", "| TRF-005 | None")).duplicateIds).toEqual(["TRF-005"]);
    expect(check(matrix().replace("| AT-TRF-07 | TRF-005", "| AT-TRF-99 | TRF-005")).missingAcceptanceTestIds).toEqual(["AT-TRF-07"]);
  });
  test("rejects collapsed or unknown cross references", () => {
    for (const cross of ["AT-TRF-01~12", "AT-TRF-01–12", "AT-TRF-01-12", "AT-TRF-99"]) expect(check(matrix().replace("| TRF-005 | AT-TRF-07", `| TRF-005 | ${cross}`)).valid).toBe(false);
  });
  test.each(["TRF-005", "G-01"])("Acceptance-test cross-references reject declared IDs from the wrong inventory: %s", (cross) => {
    const report = check(matrix().replace("| TRF-005 | AT-TRF-07", `| TRF-005 | ${cross}`));
    expect(report.valid).toBe(false);
    expect(report.errors.join(" ")).toContain(`cross-reference ${cross}`);
  });
  test.each(["AT-TRF-07", "G-01"])("Requirement cross-references reject declared IDs from the wrong inventory: %s", (cross) => {
    const report = check(matrix().replace("| AT-TRF-07 | TRF-005", `| AT-TRF-07 | ${cross}`));
    expect(report.valid).toBe(false);
    expect(report.errors.join(" ")).toContain(`cross-reference ${cross}`);
  });
  test.each([
    ["| TRF-005 | AT-TRF-07", "| TRF-005 |"],
    ["| AT-TRF-07 | TRF-005", "| AT-TRF-07 |"],
  ])("unmapped cross-references require a reason in either direction: %s", (from, prefix) => {
    for (const cross of ["None", "None ()", "None: "]) expect(check(matrix().replace(from, `${prefix} ${cross}`)).valid).toBe(false);
    for (const cross of ["None (outside this slice)", "None: no declaration maps this case"])
      expect(check(matrix().replace(from, `${prefix} ${cross}`)).valid).toBe(true);
  });
  test("priority cannot stand in for a phase; one phase only", () => {
    for (const phase of ["P0", "Phase 0~1", "Phase 6", ""]) expect(check(matrix().replace("| Phase 1 | Partial", `| ${phase} | Partial`)).valid).toBe(false);
  });
  test("requires status, evidence, cycle and unmapped reason", () => {
    for (const [from, to] of [["| Partial |", "| Done |"], ["| governor-2026-11-13 |", "| pending |"], ["Pending: Sweeper not implemented by Governor", ""], ["None (outside this slice)", ""]]) expect(check(matrix().replace(from ?? "", to ?? "")).valid).toBe(false);
  });
  test("rejects missing, ignored, external and escaping evidence paths", () => {
    for (const path of ["tests/missing.rs", "target/ignored.rs", "apps/desktop/src-tauri/target/log.rs", "../other/test.rs", "/tmp/test.rs", "https://example.com/test.rs"]) expect(check(matrix().replaceAll("tests/actual.rs", path)).valid).toBe(false);
  });
  test("validates actual test selectors and JSON pointers", () => {
    expect(check(matrix().replaceAll("#native_exit_is_preserved", "#invented")).valid).toBe(false);
    expect(check(matrix().replaceAll("tests/actual.rs#native_exit_is_preserved", ".arc-verify/governor/integration.json#/checks/passed")).valid).toBe(true);
    expect(check(matrix().replaceAll("tests/actual.rs#native_exit_is_preserved", ".arc-verify/governor/integration.json#/missing")).valid).toBe(false);
  });
  test("rejects claimed completion with pending or mere source evidence", () => {
    expect(check(matrix().replace("| Phase 1 | Complete | [native](tests/actual.rs#native_exit_is_preserved)", "| Phase 1 | Complete | Pending: case not executed yet")).valid).toBe(false);
    const modified = matrix().replace("| Phase 3 | Pending | Pending: Sweeper not implemented by Governor", "| Phase 3 | Complete | [native](tests/actual.rs#native_exit_is_preserved)");
    expect(check(modified).errors.join(" ")).toContain("broader SRS obligation");
    expect(check(matrix().replace(", [receipt](.arc-verify/governor/integration.json#/checks/passed)", "")).errors.join(" ")).toContain("recorded executable receipt");
  });
  test.each([false, null])("Complete rejects a selected nonpassing receipt result: %j", (result) => {
    const files = { ...access, read: (path: string) => path.endsWith(".json") ? JSON.stringify({ checks: { passed: result, unrelatedPassed: true } }) : access.read(path) };
    const report = check(matrix(), files);
    expect(report.valid).toBe(false);
    expect(report.errors.join(" ")).toContain("recorded executable receipt");
  });
  test("Complete rejects specification-only evidence even inside the evidence directory", () => {
    const report = check(matrix().replace(".arc-verify/governor/integration.json#/checks/passed", ".arc-verify/governor/contract.md"));
    expect(report.valid).toBe(false);
    expect(report.errors.join(" ")).toContain("recorded executable receipt");
  });
  test.each(["passed", "native", "native/exit", "predecessor", "predecessor/exitCode", "case"])("Complete accepts a relevant affirmative execution receipt: %s", (selector) => {
    expect(check(matrix().replace("#/checks/passed", `#/checks/${selector}`)).valid).toBe(true);
  });
  test.each([
    { argv: ["cargo", "test"], exit: 1 },
    { argv: ["cargo", "test"], exit: null },
    { exit: 0 },
    { executedAndPassed: false },
  ])("Complete rejects a receipt object without an affirmative executed result: %j", (result) => {
    const files = { ...access, read: (path: string) => path.endsWith(".json") ? JSON.stringify({ checks: { passed: result } }) : access.read(path) };
    expect(check(matrix(), files).valid).toBe(false);
  });
  test.each([
    { argv: ["cargo", "test"], exit: 1, passed: true },
    { argv: ["cargo", "test"], exitCode: 1, pass: true },
    { argv: ["cargo", "test"], exit: null, executedAndPassed: true },
    { argv: ["cargo", "test"], exit: 0, exitCode: 1, passed: true },
  ])("Complete rejects failed command receipts despite success flags: %j", (result) => {
    const files = { ...access, read: (path: string) => path.endsWith(".json") ? JSON.stringify({ checks: { passed: result } }) : access.read(path) };
    const report = check(matrix(), files);
    expect(report.valid).toBe(false);
    expect(report.errors.join(" ")).toContain("recorded executable receipt");
  });
  test.each([
    [{ executed: false, passed: true }, "passed"],
    [{ executed: false, pass: true }, "pass"],
    [{ executed: false, executedAndPassed: true }, "executedAndPassed"],
    [{ executedAndPassed: false, passed: true }, "passed"],
    [{ passed: false, pass: true }, "pass"],
    [{ argv: ["cargo", "test"], exit: 1, passed: true }, "passed"],
    [{ argv: ["cargo", "test"], exit: null, passed: true }, "passed"],
    [{ argv: ["cargo", "test"], exitCode: 1, pass: true }, "pass"],
    [{ executed: false, argv: ["cargo", "test"], exit: 0 }, "exit"],
    [{ argv: ["cargo", "test"], exit: 0, exitCode: 1 }, "exit"],
  ] as const)("Complete rejects selected success fields from failed or unexecuted cases: %j #%s", (result, selector) => {
    const files = { ...access, read: (path: string) => path.endsWith(".json") ? JSON.stringify({ checks: { case: result } }) : access.read(path) };
    const report = check(matrix().replace("#/checks/passed", `#/checks/case/${selector}`), files);
    expect(report.valid).toBe(false);
    expect(report.errors.join(" ")).toContain("recorded executable receipt");
  });
  test("Complete allows unknown-containing evidence filenames and selectors with a passing receipt", () => {
    expect(check(matrix().replaceAll("#native_exit_is_preserved", "#unknown_command_bypasses")).valid).toBe(true);
    expect(check(matrix().replaceAll("tests/actual.rs#native_exit_is_preserved", "tests/unknown_command.rs#unknown_command_bypasses")).valid).toBe(true);
    expect(check(matrix().replace("| Complete | [native](tests/actual.rs#native_exit_is_preserved)", "| Complete | `tests/unknown_command.rs#unknown_command_bypasses`")).valid).toBe(true);
  });
  test.each(["Pending: case not executed yet", "Execution is unverified", "Outcome unknown", "Only partial execution"])("Complete rejects contradictory prose beside affirmative evidence: %s", (prose) => {
    const report = check(matrix().replace("](.arc-verify/governor/integration.json#/checks/passed)", `](.arc-verify/governor/integration.json#/checks/passed); ${prose}`));
    expect(report.valid).toBe(false);
    expect(report.errors.join(" ")).toContain("Complete contradicts");
  });
  test.each(["Pending: case not executed yet", "Execution is unverified", "Outcome unknown", "Only partial execution"])("Complete rejects contradictory Markdown link labels beside affirmative evidence: %s", (prose) => {
    const report = check(matrix().replaceAll("[native]", `[${prose}]`));
    expect(report.valid).toBe(false);
    expect(report.errors.join(" ")).toContain("Complete contradicts");
  });
  test("historical completion cannot cite a cycle unsupported by its receipts", () => {
    expect(check(matrix().replaceAll("governor-2026-11-13", "predecessor-2026-11-12")).errors.join(" ")).toContain("linked predecessor receipt");
  });
  test("does not combine requirement and acceptance counts or omit empty phases", () => {
    expect(check(matrix().replace("| Requirements | Phase 1 | 1 |", "| Requirements | Phase 1 | 2 |")).phaseCountsReconciled).toBe(false);
    expect(check(matrix().replace("| Acceptance | Phase 5 | 0 | 0 | 0 | 0 | 0 |", "")).phaseCountsReconciled).toBe(false);
    expect(check(matrix().replace("| Catalogs | Phase 5 | 0 | 0 | 0 | 0 | 0 |", "| Catalogs | Phase 4 | 0 | 0 | 0 | 0 | 0 |")).phaseCountsReconciled).toBe(false);
  });
  test("parses escaped pipes and ignores fenced fake matrices", () => {
    expect(check(`${matrix()}\n\n\`\`\`md\n${matrix()}\n\`\`\``).valid).toBe(true);
    expect(check(matrix().replace("Pending: all attribution remains incomplete", "Pending: all attribution \\| measurements remain incomplete")).valid).toBe(true);
  });
});
