import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  realpathSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, resolve } from "node:path";

interface CargoPackage {
  id: string;
  name: string;
  version: string;
  source: string | null;
  manifest_path: string;
  license: string | null;
  license_file: string | null;
}
interface CargoMetadata {
  packages: CargoPackage[];
  resolve: { nodes: { id: string; dependencies: string[] }[] };
}
const desktop = resolve(import.meta.dir, "..");
const root = resolve(desktop, "../..");
function execute(argv: string[]): string {
  const result = Bun.spawnSync(argv, { cwd: root });
  if (result.exitCode !== 0)
    throw new Error(`Notices prerequisite failed: ${argv[0]}`);
  return result.stdout.toString();
}
const host = /^host: (.+)$/m.exec(execute(["rustc", "-vV"]))?.[1];
if (!host) throw new Error("Rust host is unavailable");
const metadata: CargoMetadata = JSON.parse(
  execute([
    "cargo",
    "metadata",
    "--format-version",
    "1",
    "--locked",
    "--filter-platform",
    host,
  ]),
);
const entry = metadata.packages.find((pkg) => pkg.name === "arc-desktop");
if (!entry) throw new Error("Desktop package is unavailable");
const graph = new Map(
  metadata.resolve.nodes.map((node) => [node.id, node.dependencies]),
);
const visited = new Set<string>();
function visit(id: string) {
  if (visited.has(id)) return;
  visited.add(id);
  for (const child of graph.get(id) ?? []) visit(child);
}
visit(entry.id);
const sections = [
  "ARC desktop — third-party notices",
  "Product license: unavailable. Dependency licenses below do not establish the ARC product license.",
  `Resolved native dependencies for ${host}, plus installed frozen Bun dependencies (including build/test tools).`,
];
const missing: string[] = [];
function append(
  name: string,
  version: string,
  license: unknown,
  directory: string,
  declaredFile?: string | null,
) {
  const candidates = readdirSync(directory)
    .filter((file) => /^(licen[cs]e|notice|copying)([._-]|$)/i.test(file))
    .map((file) => resolve(directory, file));
  if (declaredFile) candidates.push(resolve(directory, declaredFile));
  const texts = [...new Set(candidates)]
    .sort()
    .filter((file) => statSync(file).isFile())
    .map((file) => readFileSync(file, "utf8"));
  const heading = `${name}@${version}`;
  sections.push(
    `\n===== ${heading} =====\nDeclared license: ${typeof license === "string" ? license : "unavailable"}`,
  );
  if (texts.length) sections.push(...texts);
  else {
    sections.push(
      "Upstream license text was not present in the installed package; full notice text unavailable.",
    );
    missing.push(heading);
  }
}
for (const pkg of metadata.packages
  .filter((pkg) => visited.has(pkg.id) && pkg.source !== null)
  .sort((a, b) =>
    `${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`),
  )) {
  append(
    `Rust ${pkg.name}`,
    pkg.version,
    pkg.license,
    dirname(pkg.manifest_path),
    pkg.license_file,
  );
}
const installed = new Set<string>();
function inventoryModules(modules: string) {
  if (!existsSync(modules)) return;
  for (const name of readdirSync(modules)
    .sort()
    .filter((name) => !name.startsWith("."))) {
    const paths = name.startsWith("@")
      ? readdirSync(resolve(modules, name))
          .sort()
          .map((child) => resolve(modules, name, child))
      : [resolve(modules, name)];
    for (const directory of paths) {
      if (existsSync(resolve(directory, "package.json")))
        installed.add(realpathSync(directory));
    }
  }
}
const modules = resolve(root, "node_modules");
inventoryModules(modules);
for (const folder of readdirSync(resolve(modules, ".bun")).sort()) {
  inventoryModules(resolve(modules, ".bun", folder, "node_modules"));
}
for (const directory of [...installed].sort()) {
  const pkg = JSON.parse(
    readFileSync(resolve(directory, "package.json"), "utf8"),
  );
  if (!pkg.private)
    append(`npm ${pkg.name}`, pkg.version, pkg.license, directory);
}
sections.push(
  "\n===== basecn Button recipe =====",
  readFileSync(resolve(desktop, "src-tauri/assets/basecn-LICENSE.txt"), "utf8"),
);
const output = resolve(desktop, "src-tauri/assets/open-source-notices.txt");
mkdirSync(dirname(output), { recursive: true });
writeFileSync(output, `${sections.join("\n\n")}\n`);
console.log(
  JSON.stringify(
    {
      target: host,
      output: "src-tauri/assets/open-source-notices.txt",
      installedBunPackages: installed.size,
      missingLicenseText: missing,
    },
    null,
    2,
  ),
);
