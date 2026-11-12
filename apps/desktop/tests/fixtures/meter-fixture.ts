import type { MeterSnapshot } from "@/bridge/meter.types";
import { isMeterSnapshot } from "@/bridge/meter-contract";
import type { Received } from "@/bridge/types";
import { meterFields } from "@/dashboard/meter-read-state";
import shared from "../../../../crates/arc-core/tests/fixtures/meter-snapshot.json";
import bridge from "./bridge.json";

// Synthetic contract data only, never native calibration/performance evidence.
export function sharedMeterSnapshot(): MeterSnapshot {
  const s: unknown = structuredClone(shared);
  if (!isMeterSnapshot(s))
    throw new Error("Rust Meter golden fixture no longer matches TypeScript");
  return s;
}
export function meterFixture(): MeterSnapshot {
  const s = sharedMeterSnapshot();
  s.sequence = 7;
  s.sampled_at_ms = bridge.clock_ms;
  s.sampled_monotonic_ms = 30_000;
  for (const field of meterFields(s)) {
    field.observed_at_ms = bridge.clock_ms;
    field.observed_monotonic_ms = 30_000;
  }
  s.host.cpu_utilization.reading = { availability: "available", value: 25 };
  s.host.cpu_interval = {
    started_at_ms: bridge.clock_ms - 2000,
    ended_at_ms: bridge.clock_ms,
    started_monotonic_ms: 28_000,
    ended_monotonic_ms: 30_000,
    elapsed_ms: 2000,
    logical_cpu_count: 8,
    normalization: "host_capacity",
  };
  s.host.memory_available_bytes.reading = {
    availability: "available",
    value: 8 * 2 ** 30,
  };
  s.host.memory_pressure.reading = {
    availability: "available",
    value: "normal",
  };
  s.host.swap_used_bytes.reading = { availability: "available", value: 0 };
  const fs = s.host.filesystem;
  fs.identity = {
    path: "/",
    mount_point: "/",
    filesystem_type: "apfs",
    volume_id: "synthetic-local-volume",
    is_local: true,
  };
  fs.capacity_bytes.reading = {
    availability: "available",
    value: 512 * 2 ** 30,
  };
  fs.free_bytes.reading = { availability: "available", value: 128 * 2 ** 30 };
  fs.available_bytes.reading = {
    availability: "available",
    value: 120 * 2 ** 30,
  };
  fs.pressure.reading = { availability: "available", value: "normal" };
  s.daemon.cpu_core_seconds.reading = {
    availability: "available",
    value: 12.5,
  };
  s.daemon.rss_bytes.reading = {
    availability: "available",
    value: 40 * 2 ** 20,
  };
  s.daemon.observed_peak_rss_bytes.reading = {
    availability: "available",
    value: 44 * 2 ** 20,
  };
  s.daemon.peak_observation_started_at_ms = bridge.clock_ms - 30_000;
  return s;
}
export function meterReply(
  s = meterFixture(),
  receivedAt = bridge.clock_ms,
): Received<MeterSnapshot> {
  return { data: s, received_at_ms: receivedAt };
}
export function nextMeter(
  previous: MeterSnapshot,
  delta = 2000,
): MeterSnapshot {
  const s = structuredClone(previous);
  s.sequence++;
  s.sampled_monotonic_ms += delta;
  if (s.sampled_at_ms !== null) s.sampled_at_ms += delta;
  s.age_ms = 0;
  for (const field of meterFields(s)) {
    field.observed_monotonic_ms += delta;
    if (field.observed_at_ms !== null) field.observed_at_ms += delta;
  }
  const i = s.host.cpu_interval;
  if (i) {
    i.started_monotonic_ms += delta;
    i.ended_monotonic_ms += delta;
    if (i.started_at_ms !== null) i.started_at_ms += delta;
    if (i.ended_at_ms !== null) i.ended_at_ms += delta;
  }
  return s;
}
