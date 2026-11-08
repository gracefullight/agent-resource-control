export const BASELINE = "fbe76a5f15101ad415f355ef06f1deeb1f97d568";
export const PINS = { bun: "1.4.4", rust: "1.95.0", cargoLlvmCov: "0.6.21" };
export const CRATES = ["arc-core", "arc-detector", "arc-storage", "arc-ipc", "arc-daemon", "arc-cli"] as const;
export const EVIDENCE = ".arc-verify/rust-coverage";
export const HARNESS = "scripts/harness/validate.sh";
// Retain historical references; harness bytes are guarded by differential tests.
export const PROTECTED = [HARNESS, "crates/arc-storage/migrations/0001_phase0.sql"];
