import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { MATRIX_PATH, repositoryAccess, SRS_PATH, validateTraceability } from "./traceability";

test("repository traceability enumerates authoritative declarations and reconciles every phase", () => {
  const report = validateTraceability(readFileSync(SRS_PATH, "utf8"), readFileSync(MATRIX_PATH, "utf8"), repositoryAccess(process.cwd()));
  expect(report.errors, JSON.stringify(report, null, 2)).toEqual([]);
  expect(report.missingRequirementIds).toEqual([]);
  expect(report.missingAcceptanceTestIds).toEqual([]);
  expect(report.missingCatalogIds).toEqual([]);
  expect(report.duplicateIds).toEqual([]);
  expect(report.allRowsHaveStatusEvidenceAndCycle).toBe(true);
  expect(report.phaseCountsReconciled).toBe(true);
  expect(report.valid).toBe(true);
});
