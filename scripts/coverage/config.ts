export const BASELINE = "fbe76a5f15101ad415f355ef06f1deeb1f97d568";
export const PINS = { bun: "1.4.4", rust: "1.95.0", cargoLlvmCov: "0.6.21" };
export const HISTORICAL_CRATES = ["arc-core", "arc-detector", "arc-storage", "arc-ipc", "arc-daemon", "arc-cli"] as const;
export const TERMINAL_CRATES = [...HISTORICAL_CRATES, "arc-traffic"] as const;
export const CRATES = [...TERMINAL_CRATES, "arc-desktop", "arc-platform", "arc-meter", "arc-policy"] as const;
export type CrateName = typeof CRATES[number];
export type CrateRoots = { manifest: string; source: string; tests: string };
export const CRATE_ROOTS: Record<CrateName, CrateRoots> = {
  "arc-core": { manifest: "crates/arc-core/Cargo.toml", source: "crates/arc-core/src", tests: "crates/arc-core/tests" },
  "arc-detector": { manifest: "crates/arc-detector/Cargo.toml", source: "crates/arc-detector/src", tests: "crates/arc-detector/tests" },
  "arc-storage": { manifest: "crates/arc-storage/Cargo.toml", source: "crates/arc-storage/src", tests: "crates/arc-storage/tests" },
  "arc-ipc": { manifest: "crates/arc-ipc/Cargo.toml", source: "crates/arc-ipc/src", tests: "crates/arc-ipc/tests" },
  "arc-daemon": { manifest: "crates/arc-daemon/Cargo.toml", source: "crates/arc-daemon/src", tests: "crates/arc-daemon/tests" },
  "arc-cli": { manifest: "crates/arc-cli/Cargo.toml", source: "crates/arc-cli/src", tests: "crates/arc-cli/tests" },
  "arc-traffic": { manifest: "crates/arc-traffic/Cargo.toml", source: "crates/arc-traffic/src", tests: "crates/arc-traffic/tests" },
  "arc-desktop": { manifest: "apps/desktop/src-tauri/Cargo.toml", source: "apps/desktop/src-tauri/src", tests: "apps/desktop/src-tauri/tests" },
  "arc-platform": { manifest: "crates/arc-platform/Cargo.toml", source: "crates/arc-platform/src", tests: "crates/arc-platform/tests" },
  "arc-meter": { manifest: "crates/arc-meter/Cargo.toml", source: "crates/arc-meter/src", tests: "crates/arc-meter/tests" },
  "arc-policy": { manifest: "crates/arc-policy/Cargo.toml", source: "crates/arc-policy/src", tests: "crates/arc-policy/tests" },
};
export const testPath = (path: string) => CRATES.some(crate => path.startsWith(CRATE_ROOTS[crate].tests + "/"));
export const EVIDENCE = ".arc-verify/rust-coverage";
export const HARNESS = "scripts/harness/validate.sh";
export const METER_INPUTS = ["docs/requirements/ARC_SRS_2026-11-06.md", ".arc-verify/host-meter/contract.md"] as const;
export const METER_SRS_SHA256 = "a8ef945251f9f3fd7c3e337d4efc77ff9914f95114212ca9e6c90ae2ce3e3d9e";
// Only immutable mission inputs enter source identity; generated reports do not.
export const MEMORY_PRESSURE_INPUTS = [".arc-verify/memory-pressure/contract.md"] as const;
export const FROZEN_INPUTS = [...METER_INPUTS, ...MEMORY_PRESSURE_INPUTS] as const;
// Retain historical references; harness bytes are guarded by differential tests.
export const PROTECTED = [HARNESS, "crates/arc-storage/migrations/0001_phase0.sql"];
