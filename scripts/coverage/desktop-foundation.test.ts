import { expect, test } from "bun:test";
import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { desktopEvidence } from "./desktop-evidence";
import { gitBytes } from "./hygiene";

test("historical desktop evidence retains committed provenance and explicit raw-log availability", () => {
  const root=resolve(import.meta.dir,"../..");
  const names=readdirSync(resolve(root,".arc-verify/desktop-foundation"));
  // The accepted source baseline contains the previous mission's delivery.
  // This checks unchanged provenance; it does not rerun or recertify that mission.
  for(const name of names) {
    const path=`.arc-verify/desktop-foundation/${name}`;
    expect(readFileSync(resolve(root,path)).equals(gitBytes(root,["show",`efedf0f4eb9b97eed641c82f3c4766bb16f2e3de:${path}`]))).toBe(true);
  }
  const result=desktopEvidence(root,{historicalAvailability:true});
  expect(names.length).toBeLessThanOrEqual(7);
  expect(Object.keys(result.gateOutcomes)).toHaveLength(6);
  expect(result.currentAcceptance).toBe(false);
  console.info(`Desktop evidence: ${JSON.stringify(result)}`);
});
