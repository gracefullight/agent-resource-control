import { describe, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createDesktopClient } from "@/bridge/client";
import {
  isAdmissionDiagnostic,
  isScriptPolicyLimits,
  isTrafficClassification,
  isTrafficDiagnosticResult,
} from "@/bridge/traffic-scripts-contract";
import {
  trafficFixture,
  trafficReply,
  trafficWait,
} from "./fixtures/traffic-scripts-fixture";
import wire from "./fixtures/traffic-scripts-wire.json";

const generatedPath = resolve(
  import.meta.dir,
  "../src/bridge/traffic-scripts.generated.ts",
);
const fixturePath = resolve(
  import.meta.dir,
  "fixtures/traffic-scripts-wire.json",
);
const JOB = wire.queries[0]?.job_id ?? "";
const OTHER = "a".repeat(32);
const clone = <T>(value: T): T => structuredClone(value);

describe("Rust-derived negotiated Traffic contract", () => {
  test("all serialized Rust variants compile against generated declarations and existing outcome types without writes", () => {
    const before = [
      readFileSync(generatedPath),
      readFileSync(fixturePath),
    ] as const;
    const source = `import type { DiagnosticRequest, DiagnosticResult, TrafficClassification, ScriptPolicyLimits, Outcome } from ${JSON.stringify(generatedPath)};
import type { Outcome as DesktopOutcome } from ${JSON.stringify(resolve(import.meta.dir, "../src/bridge/types.ts"))};
const classifications: TrafficClassification[] = ${JSON.stringify(wire.classifications)};
const limits: ScriptPolicyLimits = ${JSON.stringify(wire.limits)};
const queries: DiagnosticRequest[] = ${JSON.stringify(wire.queries)};
const results: DiagnosticResult[] = ${JSON.stringify(wire.results)};
const outcomeAgreement = (outcome: Outcome): DesktopOutcome => outcome;
const reverseOutcomeAgreement = (outcome: DesktopOutcome): Outcome => outcome;
// @ts-expect-error class and reason must agree
const badClass: TrafficClassification = { classification: "TYPECHECK", locality: "HOST_LOCAL", reason: "BUN_SCRIPT_TEST", uncertainty: "NONE" };
// @ts-expect-error nullable classification and admission are required
const missingNullable: DiagnosticResult = { schema_version: 1, job_id: "${JOB}", recorded_at_ms: 1, liveness_fresh: false, limits };
// @ts-expect-error TYPECHECK limits cannot disappear in a legacy projection
const missingClass: ScriptPolicyLimits = { build: limits.build, test: limits.test, lint: limits.lint, budget: limits.budget };
// @ts-expect-error diagnostic reads never assert live process freshness
const fresh: DiagnosticResult = { ...results[0]!, liveness_fresh: true };
// @ts-expect-error state vocabulary is negotiated and closed
const badState: DiagnosticResult = { ...results[0]!, admission: { ...results[0]!.admission!, state: "BYPASSED" } };
`;
    const temporary = mkdtempSync(join(tmpdir(), "arc-traffic-contract-"));
    try {
      writeFileSync(join(temporary, "agreement.ts"), source);
      writeFileSync(
        join(temporary, "tsconfig.json"),
        JSON.stringify({
          compilerOptions: {
            strict: true,
            exactOptionalPropertyTypes: true,
            noEmit: true,
            skipLibCheck: true,
            target: "ES2022",
            module: "ESNext",
            moduleResolution: "bundler",
            types: [],
            paths: { "@/*": [resolve(import.meta.dir, "../src/*")] },
          },
          files: ["agreement.ts"],
        }),
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

  test("runtime classification accepts exactly the Rust-generated combinations", () => {
    expect(wire.generatedFrom).toContain("Rust arc-ipc::traffic::diagnostics");
    const expected = new Set(
      wire.classifications.map((c) => JSON.stringify(c)),
    );
    for (const classification of [
      "BUILD",
      "TEST",
      "LINT",
      "TYPECHECK",
      "REMOTE_CI",
      "REMOTE_EXEC",
      "MIXED",
      "UNKNOWN",
    ])
      for (const reason of new Set(wire.classifications.map((c) => c.reason)))
        for (const locality of ["HOST_LOCAL", "REMOTE", "MIXED", "UNKNOWN"])
          for (const uncertainty of ["NONE", "OBSERVE"]) {
            const candidate = { classification, locality, reason, uncertainty };
            expect(isTrafficClassification(candidate)).toBe(
              expected.has(JSON.stringify(candidate)),
            );
          }
    for (const value of [
      null,
      [],
      {},
      { ...wire.classifications[0], reason: "FUTURE" },
      { ...wire.classifications[0], argv: "private" },
    ])
      expect(isTrafficClassification(value)).toBe(false);
    for (const result of wire.results)
      expect(isTrafficDiagnosticResult(result, JOB)).toBe(true);
    expect(
      new Set(
        wire.results.flatMap((r) => (r.admission ? [r.admission.state] : [])),
      ),
    ).toEqual(
      new Set([
        "QUEUED",
        "RESERVED",
        "RUNNING",
        "COMPLETED",
        "CANCELLED",
        "ABANDONED",
        "UNCONFIRMED",
        "UNRESERVED",
      ]),
    );
  });

  test("configured four-class limits validate reservations against the shared budget rather than pinning defaults", () => {
    const limits = clone(wire.limits);
    limits.typecheck.window = 3;
    limits.typecheck.reservation.cpu_millis = 1500;
    expect(isScriptPolicyLimits(limits)).toBe(true);
    limits.typecheck.window = 4_294_967_295;
    expect(isScriptPolicyLimits(limits)).toBe(true);
    for (const bad of [0, -1, 1.5, Number.MAX_SAFE_INTEGER + 1, 4_294_967_296])
      expect(
        isScriptPolicyLimits({
          ...limits,
          typecheck: { ...limits.typecheck, window: bad },
        }),
      ).toBe(false);
    for (const field of ["cpu_millis", "memory_bytes"] as const) {
      for (const bad of [
        0,
        -1,
        NaN,
        Infinity,
        Number.MAX_SAFE_INTEGER + 1,
        limits.budget[field] + 1,
      ])
        expect(
          isScriptPolicyLimits({
            ...limits,
            typecheck: {
              ...limits.typecheck,
              reservation: { ...limits.typecheck.reservation, [field]: bad },
            },
          }),
        ).toBe(false);
    }
    const safe = {
      cpu_millis: Number.MAX_SAFE_INTEGER,
      memory_bytes: Number.MAX_SAFE_INTEGER,
    };
    expect(
      isScriptPolicyLimits({
        ...limits,
        budget: safe,
        typecheck: { window: 1, reservation: safe },
      }),
    ).toBe(true);
    expect(isScriptPolicyLimits({ ...limits, typecheck: undefined })).toBe(
      false,
    );
    expect(
      isScriptPolicyLimits({ ...limits, budget: [2000, 4294967296] }),
    ).toBe(false);
  });

  test("required nulls, safe numbers, exact object trees, IDs, outcomes and historical time arithmetic are enforced", () => {
    const result = trafficFixture(JOB);
    const admission = result.admission;
    if (!admission) throw new Error("Missing admission");
    for (const field of Object.keys(result)) {
      const malformed: Record<string, unknown> = { ...result };
      delete malformed[field];
      expect(isTrafficDiagnosticResult(malformed)).toBe(false);
    }
    for (const field of Object.keys(admission)) {
      const malformed: Record<string, unknown> = { ...admission };
      delete malformed[field];
      expect(
        isTrafficDiagnosticResult({ ...result, admission: malformed }),
      ).toBe(false);
    }
    for (const bad of [
      -1,
      0.1,
      NaN,
      Infinity,
      Number.MAX_SAFE_INTEGER + 1,
      "20",
    ])
      expect(
        isTrafficDiagnosticResult({ ...result, recorded_at_ms: bad }),
      ).toBe(false);
    for (const patch of [
      { job_id: OTHER },
      { schema_version: 2 },
      { liveness_fresh: true },
      { private: "x".repeat(8193) },
      { classification: Object.values(result.classification ?? {}) },
      { classification: null },
      { admission: Object.values(admission) },
      { admission: { ...admission, wait_reason: "class_window" } },
      { admission: { ...admission, sequence: 0 } },
      { admission: { ...admission, state: "COMPLETED" } },
      { admission: { ...admission, permit_active: true } },
      { admission: { ...admission, outcome: { kind: "EXITED", code: 17 } } },
      { admission: { ...admission, root_job_id: JOB } },
      { admission: { ...admission, queued_at_ms: 21 } },
    ])
      expect(isTrafficDiagnosticResult({ ...result, ...patch }, JOB)).toBe(
        false,
      );
    const running = trafficFixture(JOB, "RUNNING");
    for (const patch of [
      { granted_at_ms: 9 },
      { granted_at_ms: 21 },
      { queue_wait_ms: 3 },
      { queue_wait_ms: 11 },
      { reservation: [1000, 1073741824] },
      { permit_generation: Number.MAX_SAFE_INTEGER + 1 },
      { root_job_id: "INVALID" },
    ])
      expect(
        isTrafficDiagnosticResult({
          ...running,
          admission: { ...running.admission, ...patch },
        }),
      ).toBe(false);
    expect(
      isTrafficDiagnosticResult({
        ...result,
        recorded_at_ms: Number.MAX_SAFE_INTEGER,
      }),
    ).toBe(true);
    expect(
      isTrafficDiagnosticResult({
        ...result,
        classification: null,
        admission: null,
      }),
    ).toBe(true);
  });

  test("all wait causes require a matching recorded WAIT decision, including pressure context", () => {
    for (const reason of [
      "class_window",
      "cpu_budget",
      "memory_budget",
      "fifo",
      "memory_pressure",
    ] as const) {
      const result = trafficWait(JOB, reason);
      expect(isTrafficDiagnosticResult(result)).toBe(true);
      expect(
        isTrafficDiagnosticResult({
          ...result,
          admission: { ...result.admission, wait_reason: "admitted" },
        }),
      ).toBe(false);
      expect(
        isTrafficDiagnosticResult({
          ...result,
          admission: { ...result.admission, last_decision: null },
        }),
      ).toBe(false);
      expect(
        isTrafficDiagnosticResult({
          ...result,
          admission: {
            ...result.admission,
            last_decision: {
              ...result.admission?.last_decision,
              decided_at_ms: 21,
            },
          },
        }),
      ).toBe(false);
    }
    const pressure = trafficWait(JOB, "memory_pressure");
    expect(
      isTrafficDiagnosticResult({
        ...pressure,
        admission: {
          ...pressure.admission,
          last_decision: {
            ...pressure.admission?.last_decision,
            observation: {
              ...pressure.admission?.last_decision?.observation,
              pressure: "normal",
            },
          },
        },
      }),
    ).toBe(false);
  });

  test("terminal outcomes and complete retained/released permits preserve shared descendant facts", () => {
    const running = trafficFixture(JOB, "RUNNING");
    const completed = trafficFixture(JOB, "COMPLETED");
    for (const state of ["CANCELLED", "ABANDONED"] as const)
      for (const permit_active of [true, false]) {
        const admission = { ...running.admission, state, permit_active };
        expect(isAdmissionDiagnostic(admission)).toBe(true);
        expect(isTrafficDiagnosticResult({ ...running, admission })).toBe(true);
      }
    expect(
      isTrafficDiagnosticResult({
        ...completed,
        admission: {
          ...running.admission,
          state: "COMPLETED",
          outcome: { kind: "EXITED", code: 17 },
        },
      }),
    ).toBe(true);
    for (const outcome of [
      { kind: "EXITED", code: 0 },
      { kind: "EXITED", code: 17 },
      { kind: "SIGNALED", signal: 15, core_dumped: false },
      { kind: "SPAWN_FAILED", reason: "NOT_FOUND", os_code: 2 },
    ])
      expect(
        isTrafficDiagnosticResult({
          ...completed,
          admission: { ...completed.admission, outcome },
        }),
      ).toBe(true);
    for (const outcome of [
      { kind: "EXITED", code: 256 },
      { kind: "SIGNALED", signal: 0, core_dumped: false },
      { kind: "CANCELLED" },
      { kind: "SPAWN_FAILED", reason: "PRIVATE", os_code: null },
    ])
      expect(
        isTrafficDiagnosticResult({
          ...completed,
          admission: { ...completed.admission, outcome },
        }),
      ).toBe(false);
  });

  test("typed client uses one fixed ID-bound read, keeps unsupported distinct and preserves existing reads", async () => {
    const calls: unknown[] = [];
    const result = trafficReply(trafficWait(JOB));
    const client = createDesktopClient(async (command, args) => {
      calls.push([command, args]);
      return command === "daemon_health"
        ? {
            data: { daemon: "running", protocol_version: 1, sweeper: "off" },
            received_at_ms: 30,
          }
        : result;
    });
    expect(await client.trafficJobDiagnostics(JOB)).toEqual(result);
    expect(calls).toEqual([["traffic_job_diagnostics", { job_id: JOB }]]);
    await expect(client.trafficJobDiagnostics("A".repeat(32))).rejects.toEqual({
      code: "invalid_request",
      retryable: false,
    });
    expect(calls).toHaveLength(1);
    await expect(client.trafficJobDiagnostics(OTHER)).rejects.toEqual({
      code: "invalid_reply",
      retryable: false,
    });
    expect((await client.daemonHealth()).data.daemon).toBe("running");
    for (const failure of [
      { code: "unsupported", retryable: false },
      { code: "unavailable", retryable: true },
    ])
      await expect(
        createDesktopClient(async () => {
          throw failure;
        }).trafficJobDiagnostics(JOB),
      ).rejects.toEqual(failure);
    await expect(
      createDesktopClient(async () => {
        throw new Error("PRIVATE");
      }).trafficJobDiagnostics(JOB),
    ).rejects.toEqual({ code: "local", retryable: false });
  });
});
