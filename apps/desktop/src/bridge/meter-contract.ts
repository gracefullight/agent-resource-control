import type {
  MeterCpuInterval,
  MeterFilesystemIdentity,
  MeterMeasurement,
  MeterSnapshot,
} from "@/bridge/meter.types";

const MAX = Number.MAX_SAFE_INTEGER;
const utf8 = new TextEncoder();
const integer = (v: unknown, min = 0, max = MAX): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= min && v <= max;
const wall = (v: unknown): v is number | null => v === null || integer(v);
const finite = (v: unknown, max = MAX): v is number =>
  typeof v === "number" && Number.isFinite(v) && v >= 0 && v <= max;
const text = (v: unknown, max: number): v is string =>
  typeof v === "string" &&
  v.length > 0 &&
  v.length <= max &&
  utf8.encode(v).length <= max &&
  !/[\uD800-\uDFFF]/u.test(v) &&
  !/\p{Cc}/u.test(v);
const object = (
  v: unknown,
  keys: readonly string[],
): v is Record<string, unknown> =>
  v !== null &&
  typeof v === "object" &&
  !Array.isArray(v) &&
  Object.keys(v).length === keys.length &&
  keys.every((k) => Object.hasOwn(v, k));
const level = (v: unknown) =>
  v === "normal" || v === "warning" || v === "critical";

function measurement(
  v: unknown,
  unit: string,
  source: string,
  sample: number,
  value: (v: unknown) => boolean,
  first = false,
): v is MeterMeasurement<unknown> {
  if (
    !object(v, [
      "unit",
      "provenance",
      "observed_at_ms",
      "observed_monotonic_ms",
      "reading",
    ]) ||
    v.unit !== unit ||
    v.provenance !== source ||
    !wall(v.observed_at_ms) ||
    !integer(v.observed_monotonic_ms, 0, sample)
  )
    return false;
  const r = v.reading;
  if (object(r, ["availability", "value"]) && r.availability === "available")
    return value(r.value);
  if (object(r, ["availability", "diagnostic"]) && r.availability === "failed")
    return text(r.diagnostic, 128);
  return (
    object(r, ["availability"]) &&
    (r.availability === "unsupported" ||
      r.availability === "denied" ||
      (first && r.availability === "first_interval"))
  );
}
function identity(v: unknown): v is MeterFilesystemIdentity {
  return (
    object(v, [
      "path",
      "mount_point",
      "filesystem_type",
      "volume_id",
      "is_local",
    ]) &&
    v.is_local === true &&
    text(v.path, 1024) &&
    v.path.startsWith("/") &&
    text(v.mount_point, 1024) &&
    v.mount_point.startsWith("/") &&
    text(v.filesystem_type, 64) &&
    text(v.volume_id, 128)
  );
}
function interval(v: unknown, observed: number): v is MeterCpuInterval {
  return (
    object(v, [
      "started_at_ms",
      "ended_at_ms",
      "started_monotonic_ms",
      "ended_monotonic_ms",
      "elapsed_ms",
      "logical_cpu_count",
      "normalization",
    ]) &&
    wall(v.started_at_ms) &&
    wall(v.ended_at_ms) &&
    integer(v.started_monotonic_ms) &&
    integer(v.ended_monotonic_ms) &&
    v.ended_monotonic_ms === observed &&
    integer(v.elapsed_ms, 1) &&
    v.ended_monotonic_ms - v.started_monotonic_ms === v.elapsed_ms &&
    integer(v.logical_cpu_count, 1, 4_294_967_295) &&
    v.normalization === "host_capacity"
  );
}
function processIdentity(v: unknown): boolean {
  return (
    object(v, ["pid", "start_token", "boot_id"]) &&
    integer(v.pid, 1, 2_147_483_647) &&
    typeof v.start_token === "string" &&
    /^[1-9][0-9]{0,19}$/.test(v.start_token) &&
    BigInt(v.start_token) <= 18_446_744_073_709_551_615n &&
    typeof v.boot_id === "string" &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(
      v.boot_id,
    )
  );
}
function cadence(v: unknown): boolean {
  if (
    !object(v, ["mode", "interval_ms", "stale_after_ms"]) ||
    !integer(v.interval_ms)
  )
    return false;
  const valid =
    v.mode === "idle"
      ? v.interval_ms >= 10_000 && v.interval_ms <= 30_000
      : (v.mode === "active" || v.mode === "pressure") &&
        v.interval_ms >= 1000 &&
        v.interval_ms <= 3000;
  return valid && v.stale_after_ms === v.interval_ms * 2 + 1000;
}

// Schema validation happens before the controller retains anything. No arrays,
// arbitrary keys, raw diagnostic UI, unsafe integers or guessed identities.
export function isMeterSnapshot(v: unknown): v is MeterSnapshot {
  if (
    !object(v, [
      "schema_version",
      "daemon_instance",
      "sequence",
      "sampled_at_ms",
      "sampled_monotonic_ms",
      "age_ms",
      "cadence",
      "host",
      "daemon",
    ]) ||
    v.schema_version !== 1 ||
    !processIdentity(v.daemon_instance) ||
    !integer(v.sequence, 1) ||
    !wall(v.sampled_at_ms) ||
    !integer(v.sampled_monotonic_ms) ||
    !integer(v.age_ms) ||
    v.age_ms > MAX - v.sampled_monotonic_ms ||
    !cadence(v.cadence)
  )
    return false;
  const sample = v.sampled_monotonic_ms;
  const h = v.host,
    d = v.daemon;
  if (
    !object(h, [
      "cpu_utilization",
      "cpu_interval",
      "memory_available_bytes",
      "memory_pressure",
      "swap_used_bytes",
      "filesystem",
    ]) ||
    !object(d, [
      "cpu_accounting",
      "peak_observation_started_at_ms",
      "peak_observation_started_monotonic_ms",
      "cpu_core_seconds",
      "rss_bytes",
      "observed_peak_rss_bytes",
    ]) ||
    d.cpu_accounting !== "process_lifetime" ||
    !wall(d.peak_observation_started_at_ms) ||
    !integer(d.peak_observation_started_monotonic_ms, 0, sample)
  )
    return false;
  const f = h.filesystem;
  if (
    !object(f, [
      "identity",
      "capacity_bytes",
      "free_bytes",
      "available_bytes",
      "pressure",
    ]) ||
    !(f.identity === null || identity(f.identity))
  )
    return false;
  for (const field of [
    h.memory_available_bytes,
    h.swap_used_bytes,
    f.capacity_bytes,
    f.free_bytes,
    f.available_bytes,
    d.rss_bytes,
  ])
    if (!measurement(field, "bytes", "native", sample, integer)) return false;
  if (
    !measurement(
      h.cpu_utilization,
      "percent",
      "derived",
      sample,
      (n) => finite(n, 100),
      true,
    ) ||
    !measurement(h.memory_pressure, "level", "native", sample, level) ||
    !measurement(f.pressure, "level", "derived", sample, level) ||
    !measurement(
      d.cpu_core_seconds,
      "core_seconds",
      "native",
      sample,
      finite,
    ) ||
    !measurement(d.observed_peak_rss_bytes, "bytes", "derived", sample, integer)
  )
    return false;
  if (
    h.cpu_utilization.reading.availability === "available"
      ? !interval(h.cpu_interval, h.cpu_utilization.observed_monotonic_ms)
      : h.cpu_interval !== null
  )
    return false;
  // All nested values are now validated; keep correspondence checks typed.
  const snapshot = v as unknown as MeterSnapshot;
  const fs = snapshot.host.filesystem,
    daemon = snapshot.daemon;
  const val = (m: MeterMeasurement<number>) =>
    m.reading.availability === "available" ? m.reading.value : null;
  const capacity = val(fs.capacity_bytes),
    free = val(fs.free_bytes),
    available = val(fs.available_bytes);
  const pressure = fs.pressure.reading;
  if (
    fs.identity === null &&
    (capacity !== null ||
      free !== null ||
      available !== null ||
      pressure.availability === "available")
  )
    return false;
  if (
    (capacity !== null && free !== null && free > capacity) ||
    (free !== null && available !== null && available > free) ||
    (capacity !== null && available !== null && available > capacity)
  )
    return false;
  if (pressure.availability === "available") {
    if (capacity === null || available === null || capacity === 0) return false;
    const ratio = available / capacity;
    if (
      pressure.value !==
      (ratio <= 0.05 ? "critical" : ratio <= 0.1 ? "warning" : "normal")
    )
      return false;
  }
  const rss = val(daemon.rss_bytes),
    peak = val(daemon.observed_peak_rss_bytes);
  if (
    (rss !== null && peak !== null && rss > peak) ||
    daemon.observed_peak_rss_bytes.observed_monotonic_ms <
      daemon.peak_observation_started_monotonic_ms
  )
    return false;
  try {
    return utf8.encode(JSON.stringify(v)).length <= 16_384;
  } catch {
    return false;
  }
}
