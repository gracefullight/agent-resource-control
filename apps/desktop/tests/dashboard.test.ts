import { afterEach, describe, expect, test } from "bun:test";
import type { JobDetail } from "@/bridge/types";
import { STALE_AFTER_MS } from "@/dashboard/desktop-refresh-controller";
import {
  A,
  act,
  advance,
  B,
  cleanup,
  clone,
  deferred,
  found,
  golden,
  mountDesktop,
  NativeFixture,
  page,
  settle,
  T,
  within,
} from "./fixtures/desktop-test-harness";
import { registerJobMeterInteractionCases } from "./fixtures/job-meter-interaction-cases";
import { registerPolicyInteractionCases } from "./fixtures/policy-interaction-cases";

afterEach(cleanup);

const panel = (view: ReturnType<typeof mountDesktop>) =>
  within(view.getByRole("region", { name: "Selected job" }));
const recent = (view: ReturnType<typeof mountDesktop>) =>
  within(view.getByRole("region", { name: "Recent jobs (up to 50)" }));
const detailA = () => clone(golden.exit_zero.data.job) as JobDetail;

describe("production Dashboard and typed bridge", () => {
  registerJobMeterInteractionCases();
  registerPolicyInteractionCases();
  test("initial loading exposes Unknown metrics without invented totals or outcomes", async () => {
    const native = new NativeFixture();
    const pending = deferred<unknown>();
    native.enqueue("daemon_health", () => pending.promise);
    const view = mountDesktop(native);
    expect(view.getByText("Checking daemon…")).toBeDefined();
    expect(recent(view).getByText("Loading recent jobs…")).toBeDefined();
    expect(view.queryByText("No recorded jobs yet.")).toBeNull();
    expect(view.queryByText("Daemon connected")).toBeNull();
    expect(view.queryByText("Succeeded · Exit code 0")).toBeNull();
    const metrics = within(
      view.getByRole("region", { name: "Resource availability" }),
    );
    for (const label of [
      "Disk writes",
      "Storage reclaimed",
      "Active agents",
      "Global running jobs",
      "Global queued jobs",
      "Usage today",
    ])
      expect(metrics.getByText(label).nextElementSibling?.textContent).toBe(
        "Unknown",
      );
    expect(metrics.queryByText("0")).toBeNull();
    expect(native.count("recent_jobs")).toBe(0);
    await act(async () => {
      pending.resolve(golden.health);
    });
    await settle();
  });

  test("a validated empty page is distinct from unavailable tracking", async () => {
    const native = new NativeFixture();
    native.recent = page([]);
    const view = mountDesktop(native);
    await settle();
    expect(view.getByText("Daemon connected")).toBeDefined();
    expect(recent(view).getByText("No recorded jobs yet.")).toBeDefined();
    expect(
      recent(view).getByText("Showing 0 recent records · limit 50"),
    ).toBeDefined();
    expect(
      panel(view).getByText("Select a job to view recorded details."),
    ).toBeDefined();
    expect(native.count("job_by_id")).toBe(0);
    expect(view.queryByText(/no work ran/i)).toBeNull();
  });

  test("Completed summaries remain outcome unknown until matching details record exit zero", async () => {
    const native = new NativeFixture();
    const pending = deferred<unknown>();
    native.enqueue("job_by_id", () => pending.promise);
    const view = mountDesktop(native);
    await settle();
    expect(recent(view).getAllByText("Outcome unknown")).toHaveLength(2);
    expect(panel(view).getByText("Loading job details…")).toBeDefined();
    expect(view.queryByText("Succeeded · Exit code 0")).toBeNull();
    await act(async () => {
      pending.resolve(golden.exit_zero);
    });
    await settle();
    expect(panel(view).getByText("Succeeded · Exit code 0")).toBeDefined();
    expect(
      native.calls
        .filter((call) => call.command === "job_by_id")
        .map((call) => call.args),
    ).toEqual([{ job_id: A }]);
  });

  test("keyboard focus moves independently of selection and exit 17 remains a failed job", async () => {
    const view = mountDesktop();
    await settle();
    const options = view.getAllByRole("option");
    const first = options[0];
    const second = options[1];
    if (!first || !second) throw new Error("Expected two recent rows");
    expect(first.getAttribute("aria-selected")).toBe("true");
    expect(first.tabIndex).toBe(0);
    expect(second.tabIndex).toBe(-1);
    await act(async () => {
      first.focus();
    });
    await view.user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(second);
    expect(second.getAttribute("aria-selected")).toBe("false");
    expect(panel(view).getByText(A)).toBeDefined();
    await view.user.keyboard("{Enter}");
    await settle();
    expect(second.getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(second);
    expect(panel(view).getByText(B)).toBeDefined();
    expect(panel(view).getByText("Failed · Exit code 17")).toBeDefined();
    expect(panel(view).queryByText("Succeeded · Exit code 0")).toBeNull();
    expect(view.native.count("job_by_id")).toBe(2);
    await view.user.keyboard("{Home}");
    expect(document.activeElement).toBe(first);
    await view.user.keyboard(" ");
    await settle();
    expect(first.getAttribute("aria-selected")).toBe("true");
    await view.user.keyboard("{End}");
    expect(document.activeElement).toBe(second);
  });

  test("record, identity observation and native receipt times have distinct exact-time disclosures", async () => {
    const view = mountDesktop();
    await settle();
    const selected = panel(view);
    const updated = selected.getByLabelText("Exact time for Record updated");
    const observed = selected.getByLabelText(
      "Exact time for Identity observed",
    );
    const received = selected.getByLabelText(
      "Exact time for Status received by desktop",
    );
    expect(updated.querySelector("time")?.getAttribute("datetime")).toBe(
      new Date(T - 10_000).toISOString(),
    );
    expect(observed.querySelector("time")?.getAttribute("datetime")).toBe(
      new Date(T - 20_000).toISOString(),
    );
    expect(received.querySelector("time")?.getAttribute("datetime")).toBe(
      new Date(T).toISOString(),
    );
    updated.focus();
    expect(document.activeElement).toBe(updated);
    await view.user.click(updated);
    expect(updated.parentElement?.hasAttribute("open")).toBe(true);
    expect(updated.parentElement?.textContent).toContain(
      `Unix milliseconds ${T - 10_000}`,
    );
    const health = view.getByRole("region", { name: "Dashboard" });
    expect(
      within(health)
        .getByLabelText("Exact time for Last successful check")
        .querySelector("time")
        ?.getAttribute("datetime"),
    ).toBe(new Date(T).toISOString());
    expect(
      within(health).getByText("Daemon health observation time")
        .nextElementSibling?.textContent,
    ).toBe("Unknown");
  });

  test("selection A/B races discard the late A reply and serialize the bounded native reads", async () => {
    const native = new NativeFixture();
    const oldA = deferred<unknown>();
    native.enqueue("job_by_id", () => oldA.promise);
    const view = mountDesktop(native);
    await settle();
    const rowB = view.getAllByRole("option")[1];
    if (!rowB) throw new Error("Expected B");
    await view.user.click(rowB);
    expect(panel(view).getByText(B)).toBeDefined();
    expect(panel(view).queryByText(A)).toBeNull();
    expect(panel(view).queryByText("Succeeded · Exit code 0")).toBeNull();
    expect(native.count("job_by_id")).toBe(1);
    await act(async () => {
      oldA.resolve(golden.exit_zero);
    });
    await settle();
    expect(panel(view).getByText("Failed · Exit code 17")).toBeDefined();
    expect(panel(view).queryByText("Succeeded · Exit code 0")).toBeNull();
    expect(native.maxInFlight).toBe(1);
    expect(
      native.calls
        .filter((call) => call.command === "job_by_id")
        .map((call) => call.args),
    ).toEqual([{ job_id: A }, { job_id: B }]);
  });

  test("disconnect retains the selected failure and timestamps; retry reconnects without executing work", async () => {
    const view = mountDesktop();
    await settle();
    const rowB = view.getAllByRole("option")[1];
    if (!rowB) throw new Error("Expected B");
    await view.user.click(rowB);
    await settle();
    view.native.enqueue("daemon_health", () =>
      Promise.reject({ code: "unavailable", retryable: true }),
    );
    await view.user.click(view.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(view.getByText("Disconnected")).toBeDefined();
    expect(
      panel(view).getByText("Stale · showing last recorded data", {
        selector: '[aria-labelledby="selected-heading"] > p',
      }),
    ).toBeDefined();
    expect(panel(view).getByText("Failed · Exit code 17")).toBeDefined();
    expect(
      panel(view)
        .getByLabelText("Exact time for Status received by desktop")
        .querySelector("time")
        ?.getAttribute("datetime"),
    ).toBe(new Date(T).toISOString());
    expect(
      view.getByRole("button", { name: "Retry connection" }),
    ).toBeDefined();
    await advance(view.clock, 999);
    expect(view.native.count("daemon_health")).toBe(2);
    await advance(view.clock, 1);
    expect(view.getByText("Daemon connected")).toBeDefined();
    expect(panel(view).getByText(B)).toBeDefined();
    expect(panel(view).getByText("Failed · Exit code 17")).toBeDefined();
    expect(
      panel(view).queryByText("Stale · showing last recorded data"),
    ).toBeNull();
    expect(
      view.native.calls.every((call) =>
        [
          "governor_get_switch",
          "daemon_health",
          "meter_snapshot",
          "meter_summary",
          "policy_current",
          "policy_job",
          "traffic_job_diagnostics",
          "recent_jobs",
          "job_by_id",
          "desktop_metadata",
        ].includes(call.command),
      ),
    ).toBe(true);
  });

  test("staleness uses monotonic age and does not fabricate a confirmed disconnect", async () => {
    const view = mountDesktop();
    await settle();
    const pending = deferred<unknown>();
    view.native.enqueue("daemon_health", () => pending.promise);
    await advance(view.clock, 5_000);
    await advance(view.clock, STALE_AFTER_MS - 5_000);
    expect(view.getByText("Daemon connected")).toBeDefined();
    expect(view.queryByText("Disconnected")).toBeNull();
    expect(
      panel(view).getByText("Stale · showing last recorded data", {
        selector: '[aria-labelledby="selected-heading"] > p',
      }),
    ).toBeDefined();
    expect(panel(view).getByText("Succeeded · Exit code 0")).toBeDefined();
    await act(async () => {
      pending.resolve({ ...golden.health, received_at_ms: T - 3_600_000 });
    });
    await settle();
    expect(
      view
        .getByLabelText("Exact time for Last successful check")
        .querySelector("time")
        ?.getAttribute("datetime"),
    ).toBe(new Date(T - 3_600_000).toISOString());
    expect(
      panel(view).queryByText("Stale · showing last recorded data"),
    ).toBeNull();
  });

  test("a detail lookup failure leaves fresh health/list independent and retains the earlier recorded result", async () => {
    const view = mountDesktop();
    await settle();
    view.native.enqueue("job_by_id", () =>
      Promise.reject({ code: "uncertain", retryable: true }),
    );
    await view.user.click(view.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(view.getByText("Daemon connected")).toBeDefined();
    expect(panel(view).getByText("Succeeded · Exit code 0")).toBeDefined();
    expect(
      panel(view).getByText("Stale · showing last recorded data"),
    ).toBeDefined();
    expect(
      panel(view).getByText(/did not reply before the deadline/),
    ).toBeDefined();
    // Only the cached selected row is stale; the new recent response itself is fresh.
    expect(
      recent(view).getAllByText("Stale · showing last recorded data"),
    ).toHaveLength(1);
  });

  test("not-found without a cached match is different from errors and empty history", async () => {
    const native = new NativeFixture();
    native.jobs.set(A, clone(golden.not_found));
    const view = mountDesktop(native);
    await settle();
    expect(panel(view).getByText(A)).toBeDefined();
    expect(panel(view).getByText("No matching record.")).toBeDefined();
    expect(
      panel(view).getByText(
        "A missing record does not establish whether the command ran.",
      ),
    ).toBeDefined();
    expect(view.queryByText("No recorded jobs yet.")).toBeNull();
    expect(panel(view).queryByText("Succeeded · Exit code 0")).toBeNull();
  });

  test("not-found after a match keeps its outcome separately as last-known data", async () => {
    const view = mountDesktop();
    await settle();
    view.native.jobs.set(A, clone(golden.not_found));
    await view.user.click(view.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(panel(view).getByText("No matching record.")).toBeDefined();
    expect(
      panel(view).getByText(
        "Last-known detail from an earlier successful lookup",
      ),
    ).toBeDefined();
    expect(panel(view).getByText("Succeeded · Exit code 0")).toBeDefined();
    expect(
      panel(view).getByText("Stale · showing last recorded data"),
    ).toBeDefined();
  });

  test("malformed and mismatched replies are rejected, kept redacted and require explicit retry", async () => {
    const view = mountDesktop();
    await settle();
    view.native.enqueue("job_by_id", clone(golden.exit_seventeen));
    await view.user.click(view.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(panel(view).getByText(/Invalid daemon response/)).toBeDefined();
    expect(panel(view).getByText("Succeeded · Exit code 0")).toBeDefined();
    expect(panel(view).queryByText("Failed · Exit code 17")).toBeNull();
    const calls = view.native.count("daemon_health");
    await advance(view.clock, 60_000);
    expect(view.native.count("daemon_health")).toBe(calls);
    view.native.enqueue("daemon_health", {
      data: { secret: "PRIVATE_COMMAND_TEXT" },
      received_at_ms: T,
    });
    await view.user.click(view.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(
      within(view.getByRole("region", { name: "Dashboard" })).getByText(
        /Invalid daemon response/,
      ),
    ).toBeDefined();
    expect(view.container.textContent).not.toContain("PRIVATE_COMMAND_TEXT");
    expect(panel(view).getByText("Succeeded · Exit code 0")).toBeDefined();
  });

  test("incompatible daemon status never appears connected and raw rejection text never appears", async () => {
    const native = new NativeFixture();
    native.enqueue("daemon_health", () =>
      Promise.reject({ code: "protocol", retryable: false }),
    );
    const view = mountDesktop(native);
    await settle();
    expect(
      within(view.getByRole("region", { name: "Dashboard" })).getByText(
        /Incompatible daemon response/,
      ),
    ).toBeDefined();
    expect(view.queryByText("Daemon connected")).toBeNull();
    expect(view.queryByText("No recorded jobs yet.")).toBeNull();
    expect(native.count("recent_jobs")).toBe(0);
    await advance(view.clock, 60_000);
    expect(native.count("daemon_health")).toBe(1);
    native.enqueue("daemon_health", () =>
      Promise.reject("SECRET /private/path token"),
    );
    await view.user.click(
      view.getByRole("button", { name: "Retry connection" }),
    );
    await settle();
    expect(view.container.textContent).not.toContain("SECRET");
    expect(
      within(view.getByRole("region", { name: "Dashboard" })).getByText(
        "Status could not be retrieved.",
      ),
    ).toBeDefined();
  });

  test("older revisions and recorded-time regressions cannot overwrite accepted detail", async () => {
    const view = mountDesktop();
    await settle();
    const older = detailA();
    older.revision--;
    view.native.jobs.set(A, found(older));
    await view.user.click(view.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(panel(view).getByText(/Invalid daemon response/)).toBeDefined();
    expect(
      panel(view).getByText("Record revision").nextElementSibling?.textContent,
    ).toBe("3");
    const regressed = detailA();
    regressed.revision++;
    regressed.observation.observed_at_ms--;
    view.native.jobs.set(A, found(regressed));
    await view.user.click(view.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(
      panel(view).getByText("Record revision").nextElementSibling?.textContent,
    ).toBe("3");
    expect(panel(view).getByText("Succeeded · Exit code 0")).toBeDefined();
  });

  test("the same recorded revision remains valid when JSON field order changes", async () => {
    const view = mountDesktop();
    await settle();
    const job = detailA();
    const reordered = {
      ...Object.fromEntries(Object.entries(job).reverse()),
      outcome: { code: 0, kind: "EXITED" },
    };
    view.native.jobs.set(A, {
      data: { kind: "found", job: reordered },
      received_at_ms: T,
    });
    await view.user.click(view.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(panel(view).queryByText(/Invalid daemon response/)).toBeNull();
    expect(panel(view).getByText("Succeeded · Exit code 0")).toBeDefined();
  });

  test("the 50-record boundary discloses truncation and preserves selection outside the next page", async () => {
    const view = mountDesktop();
    await settle();
    const ids = Array.from({ length: 50 }, (_, index) =>
      (index + 100).toString(16).padStart(32, "0"),
    );
    const next = page(ids);
    next.data.truncated = true;
    view.native.recent = next;
    await view.user.click(view.getByRole("button", { name: "Refresh status" }));
    await settle();
    expect(view.getAllByRole("option")).toHaveLength(50);
    expect(
      recent(view).getByText("Showing 50 recent records · limit 50"),
    ).toBeDefined();
    expect(
      recent(view).getByText("Older records are omitted from this response."),
    ).toBeDefined();
    expect(
      panel(view).getByText("Selected job is outside the recent list."),
    ).toBeDefined();
    expect(panel(view).getByText(A)).toBeDefined();
    expect(panel(view).getByText("Succeeded · Exit code 0")).toBeDefined();
    expect(panel(view).queryByText("No matching record.")).toBeNull();
    const metrics = view.getByRole("region", { name: "Resource availability" });
    for (const label of [
      "Disk writes",
      "Storage reclaimed",
      "Active agents",
      "Global running jobs",
      "Global queued jobs",
      "Usage today",
    ])
      expect(
        within(metrics).getByText(label).nextElementSibling?.textContent,
      ).toBe("Unknown");
    expect(within(metrics).queryByText("50")).toBeNull();
  });

  test("a newer selected summary keeps an older settled detail stale until a valid revision catches up", async () => {
    const view = mountDesktop();
    await settle();
    const selectedRow = () =>
      within(recent(view).getByRole("option", { selected: true }));
    expect(panel(view).getByText("Succeeded · Exit code 0")).toBeDefined();
    expect(
      selectedRow().queryByText("Stale · showing last recorded data"),
    ).toBeNull();
    const next = page();
    const rowA = next.data.jobs[0];
    if (!rowA) throw new Error("Expected summary A");
    rowA.updated_at_ms += 1_000;
    view.native.recent = next;
    const pending = deferred<unknown>();
    view.native.enqueue("job_by_id", () => pending.promise);
    await advance(view.clock, 5_000);
    expect(panel(view).getByText("Loading job details…")).toBeDefined();
    expect(
      panel(view).getByText("Stale · showing last recorded data"),
    ).toBeDefined();
    expect(panel(view).getByText("Succeeded · Exit code 0")).toBeDefined();
    expect(
      selectedRow().getByText("Stale · showing last recorded data"),
    ).toBeDefined();
    await act(async () => {
      pending.resolve(found(detailA(), T + 5_000));
    });
    await settle();
    expect(panel(view).queryByText("Loading job details…")).toBeNull();
    expect(panel(view).queryByText(/Invalid daemon response/)).toBeNull();
    expect(
      selectedRow().getByText("Stale · showing last recorded data"),
    ).toBeDefined();
    expect(
      panel(view).getByText("Stale · showing last recorded data"),
    ).toBeDefined();
    expect(selectedRow().getByText("Succeeded · Exit code 0")).toBeDefined();
    expect(panel(view).getByText("Succeeded · Exit code 0")).toBeDefined();
    expect(
      panel(view).getByText("Record revision").nextElementSibling?.textContent,
    ).toBe("3");
    for (const [label, timestamp] of [
      ["Record updated", T - 10_000],
      ["Identity observed", T - 20_000],
    ] as const) {
      expect(
        panel(view)
          .getByLabelText(`Exact time for ${label}`)
          .querySelector("time")
          ?.getAttribute("datetime"),
      ).toBe(new Date(timestamp).toISOString());
    }
    const updated = detailA();
    updated.revision++;
    updated.updated_at_ms += 1_000;
    view.native.jobs.set(A, found(updated, T + 10_000));
    await advance(view.clock, 5_000);
    expect(view.native.count("job_by_id")).toBe(3);
    expect(
      panel(view).queryByText("Stale · showing last recorded data"),
    ).toBeNull();
    expect(
      selectedRow().queryByText("Stale · showing last recorded data"),
    ).toBeNull();
    expect(
      panel(view).getByText("Record revision").nextElementSibling?.textContent,
    ).toBe("4");
    expect(
      panel(view)
        .getByLabelText("Exact time for Record updated")
        .querySelector("time")
        ?.getAttribute("datetime"),
    ).toBe(new Date(updated.updated_at_ms).toISOString());
    expect(panel(view).getByText("Succeeded · Exit code 0")).toBeDefined();
    expect(selectedRow().getByText("Succeeded · Exit code 0")).toBeDefined();
  });

  test("unchanged polls never mutate the selected-result announcement; a new selection announces its result", async () => {
    const view = mountDesktop();
    await settle();
    const status = panel(view).getByRole("status");
    const firstAnnouncement = `Selected job ${A}: Succeeded · Exit code 0 Job measurements: Legacy measurement unavailable Memory admission details are not supported by this daemon. Scheduling details are not supported by this daemon.`;
    expect(status.getAttribute("aria-live")).toBe("polite");
    expect(status.textContent).toBe(firstAnnouncement);
    const mutations: MutationRecord[] = [];
    const observer = new MutationObserver((records) =>
      mutations.push(...records),
    );
    observer.observe(status, {
      childList: true,
      characterData: true,
      characterDataOldValue: true,
      subtree: true,
    });
    const drainMutations = () => {
      const records = [...mutations, ...observer.takeRecords()];
      mutations.length = 0;
      return records;
    };
    try {
      for (let poll = 1; poll <= 2; poll++) {
        const pending = deferred<unknown>();
        view.native.enqueue("job_by_id", () => pending.promise);
        await advance(view.clock, 5_000);
        expect(view.native.count("job_by_id")).toBe(poll + 1);
        expect(panel(view).getByText("Loading job details…")).toBeDefined();
        expect(drainMutations()).toHaveLength(0);
        expect(panel(view).getByRole("status")).toBe(status);
        expect(status.textContent).toBe(firstAnnouncement);
        await act(async () => {
          pending.resolve(found(detailA(), T + poll * 5_000));
        });
        await settle();
        expect(panel(view).queryByText("Loading job details…")).toBeNull();
        expect(drainMutations()).toHaveLength(0);
        expect(panel(view).getByRole("status")).toBe(status);
        expect(status.textContent).toBe(firstAnnouncement);
      }

      const nextSelection = deferred<unknown>();
      view.native.enqueue("job_by_id", () => nextSelection.promise);
      const rowB = view.getAllByRole("option")[1];
      if (!rowB) throw new Error("Expected B");
      await view.user.click(rowB);
      await settle();
      expect(status.textContent).toBe("");
      expect(drainMutations().length).toBeGreaterThan(0);
      await act(async () => {
        nextSelection.resolve(golden.exit_seventeen);
      });
      await settle();
      expect(panel(view).getByRole("status")).toBe(status);
      expect(status.textContent).toBe(
        `Selected job ${B}: Failed · Exit code 17 Job measurements: Legacy measurement unavailable Memory admission details are not supported by this daemon. Scheduling details are not supported by this daemon.`,
      );
      expect(drainMutations().length).toBeGreaterThan(0);
      expect(panel(view).getByText("Failed · Exit code 17")).toBeDefined();
      expect(view.native.maxInFlight).toBe(1);
    } finally {
      observer.disconnect();
    }
  });

  test("automatic page replacement restores focus when the selected row leaves recent history", async () => {
    const view = mountDesktop();
    await settle();
    const first = view.getAllByRole("option")[0];
    if (!first) throw new Error("Expected selected row");
    await view.user.click(first);
    expect(document.activeElement === first).toBe(true);
    view.native.recent = page([B]);
    await advance(view.clock, 5_000);
    expect(
      panel(view).getByText("Selected job is outside the recent list."),
    ).toBeDefined();
    expect(
      document.activeElement ===
        view.getByRole("heading", { name: "Selected job" }),
    ).toBe(true);
  });

  test("unknown, running, signal and spawn outcomes retain their actual meaning", async () => {
    const native = new NativeFixture();
    native.jobs.set(A, clone(golden.unknown));
    const view = mountDesktop(native);
    await settle();
    expect(panel(view).getAllByText("Outcome unknown").length).toBeGreaterThan(
      0,
    );
    expect(view.queryByText("Cancelled")).toBeNull();
    view.unmount();

    const variants: { job: JobDetail; label: string; extra: string }[] = [
      {
        job: {
          ...detailA(),
          phase: "RUNNING",
          reason: "CHILD_VERIFIED",
          completed_at_ms: null,
          outcome: null,
          observation: {
            target: "CHILD",
            status: "VERIFIED_MATCH",
            observed_at_ms: T - 20_000,
            reason: "CHILD_VERIFIED",
          },
        },
        label: "Outcome unknown",
        extra: "Running · last recorded",
      },
      {
        job: {
          ...detailA(),
          phase: "SIGNALED",
          reason: "OBSERVED_SIGNAL",
          outcome: { kind: "SIGNALED", signal: 15, core_dumped: false },
          observation: { ...detailA().observation, reason: "OBSERVED_SIGNAL" },
        },
        label: "Terminated by signal 15",
        extra: "Core dump recorded: No",
      },
      {
        job: {
          ...detailA(),
          phase: "SPAWN_FAILED",
          reason: "OBSERVED_SPAWN_FAILURE",
          outcome: { kind: "SPAWN_FAILED", reason: "NOT_FOUND", os_code: null },
          observation: {
            target: "NONE",
            status: "NOT_ATTACHED",
            observed_at_ms: T - 20_000,
            reason: "OBSERVED_SPAWN_FAILURE",
          },
        },
        label: "Spawn failed",
        extra: "Executable not found",
      },
    ];
    for (const variant of variants) {
      const fixture = new NativeFixture();
      fixture.jobs.set(A, found(variant.job));
      const mounted = mountDesktop(fixture);
      await settle();
      expect(panel(mounted).getByText(variant.label)).toBeDefined();
      expect(panel(mounted).getByText(variant.extra)).toBeDefined();
      expect(panel(mounted).queryByText("Succeeded · Exit code 0")).toBeNull();
      if (variant.job.phase === "SPAWN_FAILED")
        expect(
          panel(mounted).getByText("OS error code").nextElementSibling
            ?.textContent,
        ).toBe("Unknown");
      mounted.unmount();
    }
  });

  test("About navigation preserves the selected job and restores meaningful list focus", async () => {
    const view = mountDesktop();
    await settle();
    const rowB = view.getAllByRole("option")[1];
    if (!rowB) throw new Error("Expected B");
    await view.user.click(rowB);
    await settle();
    await view.user.click(
      view.getByRole("button", { name: "About / Credits" }),
    );
    expect(
      view.getByRole("heading", { name: "Agent Resource Control", level: 2 }),
    ).toBeDefined();
    await view.user.click(view.getByRole("button", { name: "Dashboard" }));
    await settle();
    expect(panel(view).getByText(B)).toBeDefined();
    expect(panel(view).getByText("Failed · Exit code 17")).toBeDefined();
    expect(document.activeElement).toBe(rowB);
  });

  test("reconnect delays are capped and one owner cleans up its timers", async () => {
    const native = new NativeFixture();
    native.healthFailure = { code: "unavailable", retryable: true };
    const view = mountDesktop(native);
    await settle();
    let expected = 1;
    for (const delay of [1_000, 2_000, 4_000, 8_000, 16_000, 30_000, 30_000]) {
      await advance(view.clock, delay - 1);
      expect(native.count("daemon_health")).toBe(expected);
      await advance(view.clock, 1);
      expect(native.count("daemon_health")).toBe(++expected);
    }
    expect(native.maxInFlight).toBe(1);
    view.unmount();
    expect(view.clock.pending).toBe(0);
    expect(view.visibility.count).toBe(0);
    await advance(view.clock, 300_000);
    expect(native.count("daemon_health")).toBe(expected);
  });
});
