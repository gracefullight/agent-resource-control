import { expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { commandEvidence, readEvidence, referenceFile, requireEvidenceRef } from "./evidence";
import { hash } from "./source";

function fixture(run: (root: string) => void) {
  const root = mkdtempSync(join(tmpdir(), "arc-evidence-"));
  try {
    mkdirSync(join(root, "target"));
    run(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("evidence reads require the recorded bytes even after a same-size replacement", () => {
  fixture(root => {
    writeFileSync(join(root, "target/output"), "before");
    const ref = referenceFile(root, "target/output");
    expect(readEvidence(root, ref)?.toString()).toBe("before");
    writeFileSync(join(root, "target/output"), "edited");
    expect(() => readEvidence(root, ref)).toThrow("hash/size mismatch");
    expect(() => readEvidence(root, { ...referenceFile(root, "target/output"), bytes: 5 })).toThrow("hash/size mismatch");
  });
});

test("optional historical absence differs from corrupt or required evidence", () => {
  fixture(root => {
    const ref = { path: "target/absent", sha256: hash(""), bytes: 0 };
    expect(readEvidence(root, ref, { optional: true })).toBeUndefined();
    expect(() => readEvidence(root, ref)).toThrow();
    writeFileSync(join(root, "target/absent"), "corrupt");
    expect(() => readEvidence(root, ref, { optional: true })).toThrow("hash/size mismatch");
  });
});

test("evidence references reject escapes, unsupported roots and noncanonical files", () => {
  fixture(root => {
    for (const path of ["/tmp/escape", "target/../escape", "target/./output", "target//output", "target\\output", "source/output"]) {
      expect(() => requireEvidenceRef({ path, sha256: hash(""), bytes: 0 })).toThrow("invalid reference");
    }
    expect(() => requireEvidenceRef({ path: "target/output", sha256: hash("") })).toThrow("missing byte size");
    writeFileSync(join(root, "target/output"), "data");
    symlinkSync("output", join(root, "target/link"));
    const ref = { path: "target/link", sha256: hash("data"), bytes: 4 };
    expect(() => readEvidence(root, ref)).toThrow("non-canonical regular file");
    expect(() => referenceFile(root, ref.path)).toThrow("non-canonical regular file");
    expect(() => referenceFile(root, "target")).toThrow("invalid reference");
  });
});

test("command evidence binds exact argv, full input manifests and successful completion", () => {
  fixture(root => {
    const files = { "Cargo.toml": hash("manifest") };
    const identity = { sha256: hash(JSON.stringify(files)), files };
    const save = (name: string, data: string) => {
      const path = `target/${name}`;
      writeFileSync(join(root, path), data);
      return referenceFile(root, path);
    };
    const manifest = save("inputs.json", JSON.stringify(identity));
    const receipt = {
      status: "passed", exitCode: 0, termination: null, argv: ["cargo", "test", "--workspace"], cwd: root,
      timeoutMs: 7_200_000, startedAt: "2026-11-14T00:00:00Z", finishedAt: "2026-11-14T00:00:01Z", durationMs: 1000,
      inputsUnchanged: true, inputBefore: { sha256: identity.sha256, manifest }, inputAfter: { sha256: identity.sha256, manifest },
      stdout: save("stdout", "passed"), stderr: save("stderr", ""),
    };
    const read = (change = {}) => commandEvidence(root, save("command.json", JSON.stringify({ ...receipt, ...change })), identity.sha256, receipt.argv);
    expect(read().stdout).toBe("passed");
    expect(() => read({ exitCode: 17 })).toThrow("unsuccessful/incomplete command");
    expect(() => read({ timeoutMs: 120_000 })).toThrow("unsuccessful/incomplete command");
    expect(() => read({ argv: ["cargo", "test"] })).toThrow("unexpected argv");
    expect(() => read({ inputsUnchanged: false })).toThrow("changed command inputs");
    expect(() => read({ inputAfter: { sha256: hash("stale"), manifest } })).toThrow("stale command");
    writeFileSync(join(root, manifest.path), JSON.stringify({ ...identity, files: { "Cargo.toml": hash("tampered") } }));
    expect(() => read()).toThrow("hash/size mismatch");
  });
});
