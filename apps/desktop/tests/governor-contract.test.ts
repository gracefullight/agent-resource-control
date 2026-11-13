import { describe, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import fixtures from "./fixtures/governor-wire.json";

const generatedPath = resolve(
  import.meta.dir,
  "../src/bridge/governor.generated.ts",
);
const fixturePath = resolve(import.meta.dir, "fixtures/governor-wire.json");

describe("Rust-derived Governor contract", () => {
  test("serialized Rust values structurally satisfy every generated switch variant without writing", () => {
    const before = [
      readFileSync(generatedPath),
      readFileSync(fixturePath),
    ] as const;
    const source = `import type { GovernorSwitchState, GovernorSwitchReadResult, GetGovernorSwitchRequest, SetGovernorSwitchRequest, GovernorMutationResult } from ${JSON.stringify(generatedPath)};
const states: GovernorSwitchState[] = ${JSON.stringify(fixtures.states)};
const reads: GetGovernorSwitchRequest[] = ${JSON.stringify(fixtures.reads)};
const requests: SetGovernorSwitchRequest[] = ${JSON.stringify(fixtures.requests)};
const results: GovernorMutationResult[] = ${JSON.stringify(fixtures.results)};
const readResults: GovernorSwitchReadResult[] = ${JSON.stringify(fixtures.readResults)};
// @ts-expect-error a switch revision must be numeric
const badRevision: GovernorSwitchState = { key: "governor.enabled", enabled: true, revision: "0" };
// @ts-expect-error an explicit result tag is required
const badTag: GovernorMutationResult = { status: "UNCERTAIN" };
// @ts-expect-error optional timestamps are numbers or null
const badTime: GovernorSwitchState = { key: "governor.enabled", enabled: true, revision: 0, updated_at_ms: "0" };
// @ts-expect-error only the fixed Governor key is allowed
const badKey: SetGovernorSwitchRequest = { key: "meter.enabled", enabled: false, expected_revision: 0, mutation_id: "0-x" };
`;
    const options = {
      strict: true,
      exactOptionalPropertyTypes: true,
      noEmit: true,
      skipLibCheck: true,
      target: "ES2022",
      module: "ESNext",
      moduleResolution: "bundler",
      types: [],
    };
    // TypeScript 7 supplies the native tsc CLI. No unstable compiler API or
    // parallel compiler dependency is needed for this structural check.
    const temporary = mkdtempSync(join(tmpdir(), "arc-governor-contract-"));
    try {
      writeFileSync(join(temporary, "agreement.ts"), source);
      writeFileSync(
        join(temporary, "tsconfig.json"),
        JSON.stringify({ compilerOptions: options, files: ["agreement.ts"] }),
      );
      const result = Bun.spawnSync(
        [
          resolve(import.meta.dir, "../node_modules/.bin/tsc"),
          "-p",
          join(temporary, "tsconfig.json"),
        ],
        { stdout: "pipe", stderr: "pipe", timeout: 7200000 },
      );
      expect(result.stdout.toString() + result.stderr.toString()).toBe("");
      expect(result.exitCode).toBe(0);
    } finally {
      rmSync(temporary, { recursive: true, force: true });
    }
    expect(readFileSync(generatedPath)).toEqual(before[0]);
    expect(readFileSync(fixturePath)).toEqual(before[1]);
  });

  test("fixtures retain safe-integer bounds, omitted optionals and enum spellings", () => {
    expect(fixtures.generatedFrom).toContain("Rust arc-ipc::governor");
    expect(fixtures.states[0]).toEqual({
      key: "governor.enabled",
      enabled: true,
      revision: 0,
    });
    expect(fixtures.states.at(-1)?.revision).toBe(Number.MAX_SAFE_INTEGER);
    for (const state of fixtures.states) {
      expect(Number.isSafeInteger(state.revision)).toBe(true);
      if ("updated_at_ms" in state) {
        expect(Number.isSafeInteger(state.updated_at_ms)).toBe(true);
      }
    }
    expect(new Set(fixtures.results.map((result) => result.status))).toEqual(
      new Set(["COMMITTED", "CONFLICT", "REJECTED"]),
    );
    expect(
      fixtures.results.some(
        (result) => "replayed" in result && result.replayed,
      ),
    ).toBe(true);
    expect(
      fixtures.results.some((result) => "changed" in result && !result.changed),
    ).toBe(true);
    expect(readFileSync(generatedPath, "utf8")).toContain(
      "Generated from Rust",
    );
    expect(readFileSync(generatedPath, "utf8")).not.toContain("bigint");
  });
});

// Runtime acceptance complements the compiler-backed Rust projection above.
import {
  boundedGovernorValue,
  governorMutationBase,
  isGovernorMutationResult,
  isGovernorQuery,
  isGovernorReadResult,
  isGovernorRequest,
  isGovernorSwitchState,
} from "@/bridge/governor-contract";

const key = "governor.enabled" as const;
const request = {
  key,
  enabled: false,
  expected_revision: 0,
  mutation_id: `0-${"a".repeat(32)}`,
};
const off = { key, enabled: false, revision: 1, updated_at_ms: 4 };
const receipt = {
  status: "COMMITTED",
  mutation_id: request.mutation_id,
  switch: off,
  changed: true,
  replayed: true,
};

test("runtime decoders preserve Rust omission/null rules and numeric bounds", () => {
  for (const timestamp of [{}, { updated_at_ms: null }, { updated_at_ms: 0 }])
    expect(
      isGovernorSwitchState({ key, enabled: true, revision: 0, ...timestamp }),
    ).toBe(true);
  expect(isGovernorSwitchState({ key, enabled: false, revision: 1 })).toBe(
    false,
  );
  for (const value of [
    -1,
    0.5,
    NaN,
    Infinity,
    Number.MAX_SAFE_INTEGER + 1,
    "1",
  ]) {
    expect(isGovernorSwitchState({ ...off, revision: value })).toBe(false);
    expect(isGovernorSwitchState({ ...off, updated_at_ms: value })).toBe(false);
    expect(isGovernorRequest({ ...request, expected_revision: value })).toBe(
      false,
    );
  }
  expect(
    isGovernorSwitchState({
      ...off,
      revision: Number.MAX_SAFE_INTEGER,
      updated_at_ms: Number.MAX_SAFE_INTEGER,
    }),
  ).toBe(true);
  expect(isGovernorSwitchState([key, false, 1, 4])).toBe(false);
  expect(isGovernorSwitchState({ ...off, private: true })).toBe(false);
  expect(isGovernorQuery({ key, mutation_id: null })).toBe(true);
  expect(isGovernorQuery({ key })).toBe(true);
  for (const id of [
    "00-a",
    `00-${"a".repeat(32)}`,
    `0-${"A".repeat(32)}`,
    `9007199254740992-${"a".repeat(32)}`,
    `0-${"a".repeat(33)}`,
  ])
    expect(governorMutationBase(id)).toBeNull();
});

test("runtime mutation/read decoders enforce result identity, tag, conditional fields and receipt ordering", () => {
  expect(isGovernorMutationResult(receipt, request)).toBe(true);
  expect(
    isGovernorReadResult(
      { switch: off, mutation_result: receipt },
      { key, mutation_id: request.mutation_id },
    ),
  ).toBe(true);
  expect(
    isGovernorReadResult({ switch: off, mutation_result: null }, { key }),
  ).toBe(true);
  expect(
    isGovernorReadResult({ switch: off, mutation_result: receipt }, { key }),
  ).toBe(false);
  expect(
    isGovernorReadResult(
      { switch: { key, enabled: true, revision: 0 }, mutation_result: receipt },
      { key, mutation_id: request.mutation_id },
    ),
  ).toBe(false);
  expect(
    isGovernorReadResult(
      { switch: { ...off, updated_at_ms: 5 }, mutation_result: receipt },
      { key, mutation_id: request.mutation_id },
    ),
  ).toBe(false);
  for (const patch of [
    { status: "committed" },
    { status: "UNCERTAIN" },
    { changed: false },
    { mutation_id: `0-${"b".repeat(32)}` },
    { switch: { ...off, enabled: true } },
    { reason: "EXTRA" },
  ])
    expect(isGovernorMutationResult({ ...receipt, ...patch }, request)).toBe(
      false,
    );
  for (const reason of ["REVISION_CONFLICT", "MUTATION_ID_CONFLICT"])
    expect(
      isGovernorMutationResult(
        {
          status: "CONFLICT",
          mutation_id: request.mutation_id,
          switch: off,
          reason,
        },
        request,
      ),
    ).toBe(true);
  expect(
    isGovernorMutationResult(
      {
        status: "REJECTED",
        mutation_id: request.mutation_id,
        switch: { key, enabled: true, revision: 0 },
        reason: "RECEIPT_CAPACITY",
      },
      request,
    ),
  ).toBe(true);
  expect(
    isGovernorMutationResult(
      {
        status: "REJECTED",
        mutation_id: request.mutation_id,
        switch: off,
        reason: "DB_BUSY",
      },
      request,
    ),
  ).toBe(false);
  expect(boundedGovernorValue({ extra: "x".repeat(2048) })).toBe(false);
});
