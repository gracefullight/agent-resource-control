import { test, expect } from "bun:test";
import { mkdtempSync, symlinkSync, rmSync, realpathSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { checkCurrentPreservation } from "../../../scripts/coverage/current-preservation";

test("aliased native checkout root retains all current preservation checks", () => {
  const root=resolve(import.meta.dir,"../../.."),temp=mkdtempSync(join(tmpdir(),"arc-guard-alias-"));
  const alias=join(temp,"checkout");
  try {
    symlinkSync(root,alias,"dir");
    expect(realpathSync(alias)).toBe(realpathSync(root));
    expect(checkCurrentPreservation(alias)).toEqual(checkCurrentPreservation(root));
    expect(checkCurrentPreservation(alias).pass).toBe(true);
  } finally {rmSync(temp,{recursive:true,force:true});}
},3600000);
