// Subprocess boundary for deterministic gate tests. The real root command has
// no fixture mode, environment override or alternate implementation path.
import { readFileSync } from "node:fs";
import { validateReport, enforce, requireExit, requireVersion } from "./gate";
import { command } from "./commands";
try {
  const args=JSON.parse(readFileSync(process.argv[2],"utf8"));
  if(args.mode==="tool") {
    const receipt=await command(["/arc-fixture-intentionally-missing-tool"],args.root,args.root);
    requireExit(receipt.exit,"MISSING_TOOL");
  } else if(args.mode==="test") {
    const receipt=await command([process.execPath,"-e","process.exit(23)"],args.root,args.root);
    requireExit(receipt.exit,"TEST_FAILURE");
  } else if(args.mode==="version") {
    requireVersion("0.0.0","0.6.21","cargo-llvm-cov");
  } else {
    enforce(validateReport(args));
  }
  console.log("PASS controlled fixture");
} catch(error) {console.error(String(error));process.exitCode=1;}
