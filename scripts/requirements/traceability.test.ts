import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { MATRIX_PATH, SRS_PATH, validateTraceability } from "./traceability";

test("repository traceability enumerates authoritative declarations and reconciles every phase", () => {
  const report = validateTraceability(readFileSync(SRS_PATH, "utf8"), readFileSync(MATRIX_PATH, "utf8"));
  expect(report.errors, JSON.stringify(report, null, 2)).toEqual([]);
  expect(report.missingRequirementIds).toEqual([]);
  expect(report.missingAcceptanceTestIds).toEqual([]);
  expect(report.missingCatalogIds).toEqual([]);
  expect(report.duplicateIds).toEqual([]);
  expect(report.phaseCountsReconciled).toBe(true);
  expect(report.valid).toBe(true);
});

test("repository IDs and counts remain valid when all narrative columns and provenance change", () => {
  const srs = readFileSync(SRS_PATH, "utf8");
  const matrix = readFileSync(MATRIX_PATH, "utf8");
  const rewritten = matrix.replaceAll("governor-2026-11-13", "arbitrary-cycle")
    .replaceAll("Evidence", "Optional prose").replaceAll("Assessment", "Unrelated column")
    .replaceAll(".arc-verify/", "missing-provenance/");
  expect(validateTraceability(srs, rewritten).valid).toBe(true);
});
