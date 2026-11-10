import { expect, test } from "bun:test";
import { lstatSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { gitBytes } from "../../scripts/coverage/hygiene";
import { hash } from "../../scripts/coverage/source";

const root = resolve(import.meta.dir, "../..");
const destination = "/Users/gracefullight/workspace/arc";
const baseline = JSON.parse(readFileSync(resolve(import.meta.dir, "baseline.json"), "utf8"));

test("mission profile preservation covers destination, backups and accepted blobs", () => {
  const head = "e0875e7449c4e5d6752dfd1b6508c408ebd42ed3";
  expect(baseline.acceptedHead).toBe(head);
  expect(baseline.workspace).toBe(root);
  expect(gitBytes(root, ["rev-parse", "HEAD"]).toString().trim()).toBe(head);
  expect(baseline.profiles.map((p: any) => p.path).sort()).toEqual(
    ["29001", "29002", "29029", "29031"].map(n => `default_16827093085728555556_0_${n}.profraw`).sort(),
  );
  const index = gitBytes(root, ["ls-files", "-z"]).toString().split("\0");
  for (const profile of baseline.profiles) {
    expect(index).not.toContain(profile.path);
    const tree = gitBytes(root, ["ls-tree", head, "--", profile.path]).toString().trim();
    expect(tree).toBe(`${profile.acceptedHeadBlob.mode} blob ${profile.acceptedHeadBlob.oid}\t${profile.path}`);
    const blob = gitBytes(root, ["cat-file", "blob", `${head}:${profile.path}`]);
    expect(blob.length).toBe(profile.sizeBytes);
    expect(hash(blob)).toBe(profile.sha256);
    expect(profile.backup.sha256).toBe(profile.sha256);
    for (const file of [resolve(root, profile.path), resolve(destination, profile.path), resolve(root, profile.backup.path)]) {
      const stat = lstatSync(file);
      expect(stat.isFile()).toBe(true);
      expect(stat.mode & 0o777).toBe(parseInt(profile.mode, 8));
      expect(stat.size).toBe(profile.sizeBytes);
      expect(hash(readFileSync(file))).toBe(profile.sha256);
    }
  }
});

test("mission scoped product and coverage protections match accepted HEAD", () => {
  const paths = ["crates", "scripts/harness", "scripts/coverage", ".cargo", "Cargo.toml", "Cargo.lock",
    "rust-toolchain.toml", "mise.toml", "package.json", "bun.lock", "bun.lockb", "docs/rust-coverage.md",
    ":(exclude)scripts/coverage/hygiene.test.ts"];
  expect(gitBytes(root, ["diff", "--name-status", baseline.acceptedHead, "--", ...paths]).toString()).toBe("");
  expect(gitBytes(root, ["ls-files", "--others", "--exclude-standard", "--", ...paths]).toString()).toBe("");
});
