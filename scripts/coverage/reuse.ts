import { reuseCoverage } from "./coverage-receipts";

try {
  if(process.argv.length!==3 || !process.argv[2]) throw Error("COVERAGE_REUSE: supply one workspace-relative receipt path");
  const reused=reuseCoverage(process.cwd(),process.argv[2]);
  console.log(`Coverage: validated fresh receipt ${reused.runId}; no collection`);
  for(const crate of reused.result.crates) console.log(`PASS ${crate.crate}: ${crate.covered}/${crate.total} production lines (${crate.percent.toFixed(2)}%; requires 80%)`);
} catch(error) {
  console.error(`Coverage receipt rejected: ${String(error)}`);
  process.exitCode=1;
}
