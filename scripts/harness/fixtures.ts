import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, chmodSync, unlinkSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve, dirname, join } from "node:path";
import { root, sync, oracle, sha } from "./test-support";

export type Fixture = { id: string; content?: Record<string, string | Buffer>; args?: string[]; coverageExit?: number;
  missingBun?: boolean; skip?: boolean; missing?: string[]; links?: Record<string, string>; staged?: boolean;
  unstaged?: Record<string, string>; deleteStaged?: string[]; emptyStaged?: boolean; unreadable?: string[] };
const secret = (prefix: string, n: number) => prefix + "A".repeat(n);
const key = (name: string) => `${name}="${"A".repeat(20)}"\n`;
const remove = ["rm", "-rf", "/tmp/probe"].join(" ");
const push = (tail: string) => ["git", "push", tail].join(" ");
export function fixtures(): Fixture[] {
  const cases: Fixture[] = [
    { id: "empty", content: {} },
    { id: "tracked-harness-exclusion", content: { "scripts/harness/validate.sh": "initial\n", "a": "clean\n" } },
    { id: "unreadable-grep-input", content: { "a.txt": secret(["lin", "api", ""].join("_"), 24), "z.txt": "clean\n" }, unreadable: ["a.txt"] },
    { id: "clean", content: Object.fromEntries(["txt", "ts", "tsx", "js", "jsx", "mjs", "cjs", "py", "go", "rs", "sh", "bash", "zsh", "yml"].map(e => [`clean.${e}`, "// clean\n"])) },
    { id: "unknown-mode", content: { "a": "clean\n" }, args: ["custom-mode"] },
    { id: "coverage-failure", content: { "a": "clean\n" }, coverageExit: 7 },
    { id: "missing-bun", content: { "a": "clean\n" }, missingBun: true },
    { id: "explicit-skip", content: { "a": "clean\n" }, skip: true },
  ];
  const secrets = [secret(["lin", "api", ""].join("_"), 24), ...["API_KEY", "API_SECRET", "ACCESS_TOKEN", "PRIVATE_KEY"].map(key),
    secret("AK" + "IA", 16), secret("gh" + "p_", 36), secret("github" + "_pat_", 82), "Bearer " + "A".repeat(40)];
  for (const [i, value] of secrets.entries()) cases.push({ id: `secret-${i + 1}`, content: { "a.txt": value + "\n" } });
  cases.push({ id: "secret-file-rule-line-order", content: Object.fromEntries(["a.txt", "z.txt"].map(p => [p, `${secrets[6]}:${secrets[6]}\n${secrets[0]}\n${secrets[0]}\n`])) });
  // Option-like paths are passed unchanged by the accepted per-file grep calls.
  // Include ordinary violations too: flags must never leak into their batches.
  for (const name of ["--definitely-invalid", "-q", "--", "-", "-H", "-l", "-v", "-e", "-f", "--help", "-V", "--include=*.txt"]) {
    cases.push({ id: `option-name-${name}`, content: { [name]: secrets[0] + "\n", "a.txt": secrets[0] + "\n", "z.txt": secrets[6] + "\n" } });
  }
  for (const name of ["--definitely-invalid.sh", "-e.sh", "-q.sh", "-dir/domain/a.ts"]) {
    cases.push({ id: `option-path-${name}`, content: { [name]: secrets[0] + "\n" + remove + "\n", "a.sh": secrets[0] + "\n" + remove + "\n", "domain/a.ts": "import x from '../tracker/x'\n" } });
  }
  cases.push({ id: "option-name-staged-invalid", staged: true, content: { "--definitely-invalid": secrets[0], "a.txt": secrets[6] } });
  cases.push({ id: "option-name-excluded", content: { "--definitely-invalid.png": secrets[0], "a.txt": secrets[6] } });
  cases.push({ id: "placeholders", content: { "a.txt": 'API_KEY="short"\nACCESS_TOKEN=""\n' } });
  const shell = [remove, push("--force origin main"), push("-f origin master"), push("--force"), push("-f")];
  shell.forEach((text, i) => cases.push({ id: `shell-${i}`, content: { "a.sh": text + "\n" } }));
  cases.push({ id: "force-report-without-trigger", content: { "a.sh": push("--force origin/main") + "\n" } });
  cases.push({ id: "force-trigger-and-report", content: { "a.sh": push("--force origin/main") + "\n" + push("--force origin main") + "\n" } });
  cases.push({ id: "force-no-cross-file-trigger", content: { "a.sh": push("--force origin/main") + "\n", "z.sh": push("--force origin main") + "\n" } });
  cases.push({ id: "shell-file-rule-line-order", content: Object.fromEntries(["a.sh", "z.bash"].map(p => [p, [...shell].reverse().concat(shell).join("\n") + "\n"])) });
  const layers = ["tracker", "workspace", "sessions", "observability", "config", "orchestrator", "relay", "server", "infrastructure"];
  layers.forEach(layer => cases.push({ id: `layer-${layer}`, content: { "domain/a.ts": `import x from '../${layer}/x'\n` } }));
  const arch = ["from infrastructure import x", 'import "outer/infrastructure/x"', ...["sqlalchemy", "django.db", "peewee", "tortoise", "motor", "pymongo", "linear_sdk"].map(p => `import ${p}`)];
  arch.forEach((value, i) => cases.push({ id: `architecture-${i}`, content: { "nested/domain/a.txt": value + "\n" } }));
  cases.push({ id: "within-domain-and-overlap", content: { "domain/a.ts": ["import x from './ports/tracker'", "import x from './infrastructure/x'", 'import x from "../infrastructure/x"', "import x from '../tracker/x'", ...arch].join("\n") + "\n", "outside/a.ts": arch.join("\n") } });
  cases.push({ id: "all-checks-coverage-after-violation", content: { "domain/a.sh": [secrets[0], ...shell, ...arch].join("\n") + "\n" }, coverageExit: 7 });
  const excludedSecret = [".env.example", "package-lock.json", "yarn.lock", "go.sum", "poetry.lock", ...["png", "jpg", "jpeg", "gif", "ico", "woff", "woff2", "ttf", "eot"].map(e => `image.${e}`)];
  cases.push({ id: "secret-exclusions", content: Object.fromEntries(excludedSecret.map(p => [p, secrets[0] + "\n"])) });
  cases.push({ id: "per-check-exclusion-controls", content: { "nested/.env.example": secrets[0], "a.test.ts": secrets[0] + "\n" + "\n".repeat(501), "ordinary.yml": shell.join("\n"), ".github/workflows/a.yaml": shell.join("\n"), "ordinary.txt": shell.join("\n") } });
  const lengthExcluded = ["x/node_modules/a.ts", "x/dist/a.ts", "x/build/a.ts", "a.d.ts", "x/__tests__/a.ts", "a.test.ts", "a.test.tsx", "a.test.js", ...[".agents", ".claude", ".codex", ".cursor", ".qwen", ".gemini", ".grok", ".kimi-code"].map(p => `${p}/a.ts`), "apps/dashboard/src/lib/canvas/a.ts", "x/apps/dashboard/src/lib/canvas/a.ts"];
  cases.push({ id: "length-exclusions", content: Object.fromEntries(lengthExcluded.map(p => [p, "\n".repeat(501)])) });
  cases.push({ id: "length-newline-boundaries", content: { "a.ts": "\n".repeat(500), "b.py": "\n".repeat(501), "c.rs": "\n".repeat(500) + "unterminated", "d.ts": "", "e.js": "\n".repeat(501) + "unterminated", "outside.md": "\n".repeat(501), "node_modules/a.ts": "\n".repeat(501), "dist/a.ts": "\n".repeat(501), "build/a.ts": "\n".repeat(501), "__tests__/a.ts": "\n".repeat(501) } });
  cases.push({ id: "filename-framing-binary-and-empty", content: { "a": secrets[0] + "\n", "a:1:b": secrets[0] + "\n", "spaces name.txt": secrets[6] + "\\:tail\n", "binary.dat": Buffer.from("\0" + secrets[0] + "\n"), "middle-binary.dat": Buffer.from(secrets[0] + "\n\0" + secrets[6] + "\n"), "z-binary.dat": Buffer.from("\0" + secrets[6] + "\n"), "domain/binary.dat": Buffer.from("\0" + arch.join("\n")), "empty.txt": "", "no-final-newline.txt": secrets[0] } });
  cases.push({ id: "symlinks", content: { "target.txt": secrets[0] + "\n", "folder/clean": "clean\n" }, links: { "link.txt": "target.txt", "dir-link": "folder", "missing-link": "absent" } });
  cases.push({ id: "missing-file", content: { "a.txt": secrets[0], "b.sh": remove }, missing: ["a.txt"] });
  cases.push({ id: "all-selected-missing", content: { "a.txt": secrets[0] }, missing: ["a.txt"] });
  cases.push({ id: "staged-add", content: { "a.sh": remove }, staged: true });
  cases.push({ id: "staged-working-violation", content: { "a.sh": "clean\n" }, staged: true, unstaged: { "a.sh": remove + "\n" } });
  cases.push({ id: "staged-working-repair", content: { "a.sh": remove }, staged: true, unstaged: { "a.sh": "clean\n" } });
  cases.push({ id: "staged-modification-deletion-copy", content: { "a.sh": "clean\n", "deleted.sh": remove, "copy.sh": "clean\n" }, staged: true, deleteStaged: ["deleted.sh"] });
  cases.push({ id: "empty-staged", content: { "a": "clean\n" }, emptyStaged: true });
  return cases;
}
export function fixtureRoot(f: Fixture) {
  const dir = mkdtempSync(join(tmpdir(), "arc-harness-fixture-"));
  sync(["git", "-c", "init.templateDir=", "init", "--quiet"], dir);
  for (const [key, value] of [["core.excludesFile", "/dev/null"], ["core.autocrlf", "false"], ["core.hooksPath", "/dev/null"], ["diff.renames", "false"]]) sync(["git", "config", key, value], dir);
  sync(["git", "config", "core.quotePath", "true"], dir);
  for (const [p, bytes] of Object.entries(f.content ?? {})) {
    mkdirSync(dirname(resolve(dir, p)), { recursive: true }); writeFileSync(resolve(dir, p), bytes);
  }
  for (const [p, target] of Object.entries(f.links ?? {})) symlinkSync(target, resolve(dir, p));
  sync(["git", "add", "--all"], dir);
  if (f.emptyStaged || f.deleteStaged) {
    sync(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "--quiet", "-m", "fixture"], dir);
  }
  if (f.deleteStaged) {
    sync(["git", "config", "diff.renames", "copies"], dir);
    for (const p of f.deleteStaged) unlinkSync(resolve(dir, p));
    writeFileSync(resolve(dir, "a.sh"), "// modified\n");
    writeFileSync(resolve(dir, "added.sh"), readFileSync(resolve(dir, "copy.sh")));
    sync(["git", "add", "--all"], dir);
  }
  for (const [p, text] of Object.entries(f.unstaged ?? {})) writeFileSync(resolve(dir, p), text);
  for (const p of f.missing ?? []) unlinkSync(resolve(dir, p));
  for (const p of f.unreadable ?? []) chmodSync(resolve(dir, p), 0);
  const bin = resolve(dir, ".fixture-bin"); mkdirSync(bin);
  // Only these existing commands are available: missing Bun is real PATH absence.
  for (const name of ["bash", "git", "grep", "wc", "tr", "mktemp", "awk", "rm", "getconf", "env", "cat", "xargs"]) {
    const path = Bun.which(name); if (path) symlinkSync(path, join(bin, name));
  }
  if (!f.missingBun) {
    writeFileSync(join(bin, "bun"), '#!/bin/bash\nprintf "coverage stdout\\n"\nprintf "coverage stderr\\n" >&2\nprintf "called\\n" >> .fixture-calls\nexit "${FIXTURE_COVERAGE_EXIT}"\n');
    chmodSync(join(bin, "bun"), 0o755);
  }
  const realGrep = Bun.which("grep")!;
  unlinkSync(join(bin, "grep"));
  writeFileSync(join(bin, "grep"), `#!/bin/bash\nprintf '%s\\0' "$@" >> .fixture-grep\nprintf '\\n' >> .fixture-grep\nexec '${realGrep}' "$@"\n`);
  chmodSync(join(bin, "grep"), 0o755);
  return { dir, env: { ...process.env, PATH: bin, LC_ALL: "C", SKIP_COVERAGE: f.skip ? "1" : undefined,
    WORKSPACE_ROOT: resolve(dir, ".fixture-workspaces"), FIXTURE_COVERAGE_EXIT: String(f.coverageExit ?? 0) } };
}
export function executeFixture(f: Fixture, dir: string, env: Record<string, string | undefined>, candidate: boolean) {
  const path = resolve(dir, "scripts/harness/validate.sh"); mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, candidate ? readFileSync(resolve(root, "scripts/harness/validate.sh")) : oracle()); chmodSync(path, 0o755);
  for (const p of [".fixture-calls", ".fixture-grep"]) { try { unlinkSync(resolve(dir, p)); } catch {} }
  const result = Bun.spawnSync(["/bin/bash", path, ...(f.args ?? (f.staged || f.emptyStaged ? ["--staged"] : []))], { cwd: dir, env, stdout: "pipe", stderr: "pipe" });
  let calls = "", trace = Buffer.alloc(0);
  try { calls = readFileSync(resolve(dir, ".fixture-calls"), "utf8"); } catch {}
  try { trace = readFileSync(resolve(dir, ".fixture-grep")); } catch {}
  const label = candidate ? "candidate" : "oracle";
  writeFileSync(resolve(dir, `${label}.stdout.txt`), result.stdout);
  writeFileSync(resolve(dir, `${label}.stderr.txt`), result.stderr);
  writeFileSync(resolve(dir, `${label}.trace`), trace);
  return { stdout: result.stdout, stderr: result.stderr, exit: result.exitCode, calls, trace,
    hashes: { stdout: sha(result.stdout), stderr: sha(result.stderr), trace: sha(trace) } };
}
