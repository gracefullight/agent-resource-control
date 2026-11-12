// Exact schema-1 projection of arc_core::meter, carried over protocol 1.
export type MeterReading<T> =
  | { availability: "available"; value: T }
  | { availability: "first_interval" }
  | { availability: "unsupported" }
  | { availability: "denied" }
  | { availability: "failed"; diagnostic: string };
export type MeterLevel = "normal" | "warning" | "critical";
export interface MeterMeasurement<T> {
  unit: "percent" | "bytes" | "core_seconds" | "level";
  provenance: "native" | "derived";
  observed_at_ms: number | null;
  observed_monotonic_ms: number;
  reading: MeterReading<T>;
}
export interface MeterCpuInterval {
  started_at_ms: number | null;
  ended_at_ms: number | null;
  started_monotonic_ms: number;
  ended_monotonic_ms: number;
  elapsed_ms: number;
  logical_cpu_count: number;
  normalization: "host_capacity";
}
export interface MeterFilesystemIdentity {
  path: string;
  mount_point: string;
  filesystem_type: string;
  volume_id: string;
  is_local: true;
}
export interface MeterSnapshot {
  schema_version: 1;
  daemon_instance: { pid: number; start_token: string; boot_id: string };
  sequence: number;
  sampled_at_ms: number | null;
  sampled_monotonic_ms: number;
  age_ms: number;
  cadence: {
    mode: "idle" | "active" | "pressure";
    interval_ms: number;
    stale_after_ms: number;
  };
  host: {
    cpu_utilization: MeterMeasurement<number>;
    cpu_interval: MeterCpuInterval | null;
    memory_available_bytes: MeterMeasurement<number>;
    memory_pressure: MeterMeasurement<MeterLevel>;
    swap_used_bytes: MeterMeasurement<number>;
    filesystem: {
      identity: MeterFilesystemIdentity | null;
      capacity_bytes: MeterMeasurement<number>;
      free_bytes: MeterMeasurement<number>;
      available_bytes: MeterMeasurement<number>;
      pressure: MeterMeasurement<MeterLevel>;
    };
  };
  daemon: {
    cpu_accounting: "process_lifetime";
    peak_observation_started_at_ms: number | null;
    peak_observation_started_monotonic_ms: number;
    cpu_core_seconds: MeterMeasurement<number>;
    rss_bytes: MeterMeasurement<number>;
    observed_peak_rss_bytes: MeterMeasurement<number>;
  };
}
