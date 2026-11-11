import { boundBytes, boundJSON, equal, requireTraffic } from "./traffic-receipts";

/** Authenticate the independent review handoff without executing its helper. */
export function validateQAIndependentHandoff(root: string, qa: any) {
  const handoff = qa?.handoff, policy = qa?.independentRuns?.freshSnapshotPolicy;
  requireTraffic(handoff?.installed === true && policy && Number.isSafeInteger(policy.passed) && policy.passed > 0 &&
    policy.failed === 0 && policy.ignored === 0, "independent QA handoff or policy results incomplete");
  const receipt = boundJSON(root, policy.receipt, "target/traffic-bun");
  const manifest = boundJSON(root, handoff.preservedManifest, "target/traffic-bun");
  boundBytes(root, handoff.helper, "target/traffic-bun");
  requireTraffic(manifest?.schemaVersion === 1 && manifest.actorId === "independent-reviewer" &&
    typeof qa.sourceHash === "string" && /^[a-f0-9]{64}$/.test(qa.sourceHash) && manifest.sourceHash === qa.sourceHash &&
    equal(manifest.policyReceipt, policy.receipt) && Array.isArray(manifest.files) && manifest.files.length > 0,
    "independent QA manifest identity or files invalid");
  const originals = new Set<string>(), preserved = new Set<string>();
  for (const entry of manifest.files) {
    requireTraffic(entry?.original && entry?.preserved && entry.original.path !== entry.preserved.path &&
      !originals.has(entry.original.path) && !preserved.has(entry.preserved.path) &&
      entry.original.sha256 === entry.preserved.sha256, "invalid or duplicate independent QA manifest entry");
    const original = boundBytes(root, entry.original, "target/traffic-bun");
    const copy = boundBytes(root, entry.preserved, "target/traffic-bun");
    requireTraffic(original.equals(copy), "independent QA preserved file differs");
    originals.add(entry.original.path); preserved.add(entry.preserved.path);
  }
  for (const ref of [policy.receipt, receipt.output?.stdout, receipt.output?.stderr]) {
    requireTraffic(ref && manifest.files.some((entry: any) => equal(entry.original, ref)),
      "independent QA receipt or output absent from manifest");
  }
  return receipt;
}
