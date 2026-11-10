import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
const baseline = resolve(process.cwd(), ".arc-verify/root-profile-hygiene/baseline.json");
if (existsSync(baseline) && JSON.parse(readFileSync(baseline, "utf8")).workspace === process.cwd())
  await import("../../.arc-verify/root-profile-hygiene/profile-preservation.mission.test");
