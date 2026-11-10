import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { directory, hash, requireValue, root as productRoot } from "./exec-fail-open-inputs";

// The live QA and final files are replaced by later mission tasks. These are
// independently pinned copies of the accepted transition and its raw inputs.
export function installHistoricalQaFixture(root: string) {
  const frozen = resolve(productRoot, "target/exec-fail-open/task19/frozen");
  const checked = (path: string, expected: string) => {
    const bytes = readFileSync(resolve(frozen, path));
    requireValue(hash(bytes) === expected, `historical fixture digest differs: ${path}`);
    return bytes;
  };
  const install = (path: string, expected: string) => {
    const bytes = checked(path, expected), destination = resolve(root, path);
    mkdirSync(resolve(destination, ".."), { recursive: true });
    writeFileSync(destination, bytes);
    return bytes;
  };
  const final = JSON.parse(install(`${directory}/final.json`, "5fecbfc96353272b61732e8fe85b29177db7508a006adbf3140121b7e7bdcb8d").toString());
  requireValue(final.sourceHash === "33a540623527224ced9ad5569446aeda301c9c9bfc04dad8137332ba33da4a34" &&
    Object.keys(final.currentInputs).length === 229 && hash(JSON.stringify(final.currentInputs)) === final.sourceHash,
  "historical transition inputs differ");
  const qa = JSON.parse(install(`${directory}/qa/review.json`, "c0b7cfb81f1de855436cab07ea32817bb2cc41e919a2c282451e013c9bc17dd4").toString());
  install(`${directory}/qa/review.md`, "ded9cd8ef940dc46081ecedf6f3cc2659926d7e7a9391b812db4839d93d4b1a7");
  for (const path of ["crates/arc-cli/src/main.rs", "docs/managed-exec.md"])
    install(path, final.currentInputs[path].sha256);
  const base = "target/exec-fail-open/task13-preserved";
  const audit = JSON.parse(install(`${base}/archive-audit.json`, "6c741b17e3679a9828781b30529f0dcd01aa207faa5939b09a2b439bb0a4e1b4").toString());
  requireValue(audit.original.exactBytes === true && audit.original.sha256 === "4456ce62e9949a3a8e1b208983ec67318cf0ff73e596c76640f7a9149f6e9366" &&
    audit.original.selectedEntries === 53 && audit.attempt13.exactBytes === true, "historical archive audit differs");
  install(`${base}/collection-next-original.json`, audit.original.sha256);
  for (const name of ["gates", "timing", "historical", "preservation"]) {
    const logical = `${directory}/implementation/${name}.json`;
    const row = audit.archives.find((item: any) => item.path === logical);
    requireValue(row && row.currentSelectionRejected === true, `historical archive audit missing: ${name}`);
    install(`${base}/prior-canonical/${name}.json`, row.sha256);
  }
  const command = qa.commands[0];
  install(command.output.stdoutPath, command.stdoutSha256);
  install(command.output.stderrPath, command.stderrSha256);
  return { qa, ctx: { sourceHash: final.sourceHash, currentInputs: final.currentInputs } };
}
