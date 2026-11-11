import { readFileSync, realpathSync } from "node:fs";
import { resolve } from "node:path";
import { commandEvidence, type EvidenceRef } from "./evidence";
import type { inputIdentity } from "./source";
import { parseCargoExecution } from "../archive/harness/managed-exec-test-output";

type Identity = ReturnType<typeof inputIdentity>;
type Section = { status?: string; receipt?: EvidenceRef; receipts?: EvidenceRef[] };
type Selector = { argv: readonly string[]; cwd: "." | "apps/desktop" };
type Sections = {
  smoke?: { frozenInstall?: Section & { bunLockBefore?: string; bunLockAfter?: string; lockUnchanged?: boolean };
    typescript?: Section & { major?: number; version?: string }; vite?: Section; tauri?: Section };
  tests?: { components?: Section; tauriHandlers?: Section; daemonBridge?: Section & { realDaemon?: boolean; cases?: Record<string,string> } };
  compatibility?: Section;
};
const WORKSPACE: Selector = { argv:["cargo","test","--workspace"],cwd:"." };
const DESKTOP: Selector = { argv:["cargo","test","-p","arc-desktop"],cwd:"." };
const COMPONENT_FILES=["dashboard","credits","contracts","lifecycle"].map(name=>`apps/desktop/tests/${name}.test.ts`);
const COMPONENT_COMMANDS: Selector[] = [
  {argv:["bun","test"],cwd:"."}, {argv:["bun","test","apps/desktop/tests"],cwd:"."},
  {argv:["bun","test",...COMPONENT_FILES],cwd:"."},
  {argv:["bun","test",COMPONENT_FILES[0],COMPONENT_FILES[1]],cwd:"."},
  ...COMPONENT_FILES.slice(2).map(path=>({argv:["bun","test",path],cwd:"." as const})),
  {argv:["bun","test","tests"],cwd:"apps/desktop"}, {argv:["bun","run","test"],cwd:"apps/desktop"},
];

// These are executed identities in the delivered component/handler contracts,
// not source-string evidence that the tests ran. Logs must contain them AND a
// successful summary whose count agrees with every logged test identity.
const COMPONENT_CASES = {
  dashboard: { group:"production Dashboard and typed bridge", names:[
    "initial loading exposes Unknown metrics without invented totals or outcomes",
    "a validated empty page is distinct from unavailable tracking",
    "Completed summaries remain outcome unknown until matching details record exit zero",
    "keyboard focus moves independently of selection and exit 17 remains a failed job",
    "record, identity observation and native receipt times have distinct exact-time disclosures",
    "selection A/B races discard the late A reply and serialize the bounded native reads",
    "disconnect retains the selected failure and timestamps; retry reconnects without executing work",
    "staleness uses monotonic age and does not fabricate a confirmed disconnect",
    "a detail lookup failure leaves fresh health/list independent and retains the earlier recorded result",
    "not-found without a cached match is different from errors and empty history",
    "not-found after a match keeps its outcome separately as last-known data",
    "malformed and mismatched replies are rejected, kept redacted and require explicit retry",
    "incompatible daemon status never appears connected and raw rejection text never appears",
    "older revisions and recorded-time regressions cannot overwrite accepted detail",
    "the same recorded revision remains valid when JSON field order changes",
    "the 50-record boundary discloses truncation and preserves selection outside the next page",
    "a newer selected summary keeps an older settled detail stale until a valid revision catches up",
    "unchanged polls never mutate the selected-result announcement; a new selection announces its result",
    "automatic page replacement restores focus when the selected row leaves recent history",
    "unknown, running, signal and spawn outcomes retain their actual meaning",
    "About navigation preserves the selected job and restores meaningful list focus",
    "reconnect delays are capped and one owner cleans up its timers",
  ]},
  credits: { group:"production About / Credits", names:[
    "exact SRS identity and three destinations use only the fixed native opener",
    "social tooltips appear on keyboard focus and links participate in normal Tab order",
    "null commit, dirty state and license remain unavailable while actual notices remain readable",
    "version, full build object, build state, architecture and notices come from local bridge metadata",
    "missing notices and metadata failure are explicit and never depend on daemon connectivity",
    "opener failure is redacted, retains keyboard focus and permits another explicit activation",
  ]},
  contracts: { group:"delivered production protocol-1 bridge contract", names:[
    "shared synthetic golden cases validate the exact Rust/TypeScript wire shapes",
    "the three data commands and snake_case selected-ID argument stay fixed",
    "health requires protocol 1 and supplies no substitute metric fields",
    "recent page bounds, uniqueness, timestamps and last-known liveness are enforced",
    "invalid job IDs are rejected before native I/O and a mismatched found ID is invalid",
    "safe-integer receipts, explicit nullable fields and terminal phase/outcome consistency are required",
    "unsupported result kinds and metadata with invented/missing fields are rejected",
    "opener and lifecycle expose exactly the three enums and local unit-returning commands",
    "rejected native values retain safe error codes and drop raw diagnostics",
  ]},
  lifecycle: { group:"production desktop component lifecycle", names:[
    "Hide Dashboard retires late native replies and Open Dashboard resumes the same React window",
    "reopen arriving before a retired read settles queues one fresh cycle and never overlaps reads",
    "desktop-only quit requests quit_desktop, stops refreshes and never sends a daemon action",
    "visibility hide/show retains selection and timestamped results, then refreshes once",
    "StrictMode leaves one subscription and unmount clears timers and ignores pending detail",
    "a failed local hide/quit action is visible and resumes inspection without leaking diagnostics",
    "the real document visibility adapter subscribes and cleans up without another lifecycle command",
  ]},
};
const HANDLER_CASES = [
  "close_request_hides_existing_window","reopen_reuses_window","quit_desktop_never_stops_daemon",
  "registered_quit_command_reaches_the_same_desktop_exit_port","system_quit_is_allowed_and_cancels_only_desktop_reads",
  "missing_window_errors_are_explicit_and_close_is_still_prevented",
  "registered_reads_have_exact_types_and_privacy_projection","registered_job_validation_rejects_invalid_inputs_before_socket_io",
  "registered_opener_accepts_exactly_the_three_enum_destinations","registered_commands_reject_malformed_envelopes_and_wrong_protocol",
  "registered_status_rejects_wrong_variants_duplicates_limits_and_unsafe_times","registered_status_validates_full_snapshot_before_projection",
  "registered_timeout_and_shutdown_are_bounded_and_redacted","unsafe_receipt_clock_is_local_and_does_not_forge_a_timestamp",
  "public_native_bridge_matches_handler_validation_and_safe_failure_mapping",
];
const DAEMON_CASES = {
  exit0:"records_exit_zero",exit17:"records_exit_seventeen",disconnectReconnect:"disconnect_reconnect",
  malformedResponse:"rejects_malformed_response",mismatchedJobId:"rejects_mismatched_job_id",
  daemonSurvival:"daemon_survives_desktop_exit",activeChildSurvival:"active_child_survives_desktop_exit",
};
const COMPATIBILITY_CASES = [
  "scheduled_exit_17_persists_failed_across_restart","lifecycle_exit_17_persists_failed_across_restart",
  "zero_exit_succeeds_after_restart_for_both_paths","recent_fifty_lifecycle_exits_are_resolved_with_the_shared_deadline",
  "slow_completed_lookup_shares_one_deadline_across_recent_rows","failed_completion_retry_releases_one_permit",
  "lifecycle_wire_shapes_are_protocol_one_and_deny_secret_fields","lifecycle_decoders_enforce_object_shape_size_semantics_and_required_nulls",
  "current_registry_default_open_preserves_wal_inventory_history_and_reopen","version_two_upgrade_preserves_existing_jobs_permits_and_both_checksums",
  "version_three_failure_rolls_back_ddl_history_and_witness","version_two_checksum_mismatch_never_applies_traffic_migration",
  "initial_endpoint_access_denial_is_unavailable_but_local_setup_is_not","sent_request_then_access_denials_stays_uncertain",
  "cancellation_and_authenticated_rejection_remain_nonlaunching",
];
export function commandCwd(root:string, actual:string, expected:"."|"apps/desktop") {
  return actual===expected || actual===resolve(realpathSync(root),expected);
}
function select(root:string, logs:ReturnType<typeof commandEvidence>, allowed:readonly Selector[]) {
  if(!allowed.some(s=>JSON.stringify(s.argv)===JSON.stringify(logs.receipt.argv) && commandCwd(root,logs.receipt.cwd,s.cwd))) {
    throw Error("EVIDENCE: command argv/cwd does not prove this desktop operation");
  }
  return logs;
}
const plain = (value:string) => value.replace(/\x1b\[[0-9;]*m/g,"");
export function bunExecution(stdout:string, stderr:string) {
  const text=plain(`${stdout}\n${stderr}`), tests:{file:string;name:string}[]=[];
  let file="";
  for(const line of text.split(/\r?\n/)) {
    const header=line.match(/^([\w./-]+\.(?:test|spec)\.[cm]?[jt]sx?):$/);
    if(header) file=header[1];
    const pass=line.match(/^\(pass\) (.+?)(?: \[[^\]\r\n]+\])?$/);
    if(pass) {
      if(!file || tests.some(test=>test.file===file && test.name===pass[1])) throw Error("EVIDENCE: unmatched/duplicate Bun identity");
      tests.push({file,name:pass[1]});
    }
  }
  const count=[...text.matchAll(/^\s*(\d+) pass\s*$/gm)], fail=[...text.matchAll(/^\s*(\d+) fail\s*$/gm)];
  if(count.length!==1 || fail.length!==1 || Number(fail[0][1])!==0 || !tests.length || Number(count[0][1])!==tests.length ||
    /^\((?:fail|skip|todo)\)/m.test(text) || /^\s*[1-9]\d* (?:skip|todo)\b/m.test(text)) throw Error("EVIDENCE: incomplete Bun executed identities/summary");
  return tests;
}
function references(section:Section): EvidenceRef[] {
  if(section.receipt && section.receipts || !section.receipt && !section.receipts?.length) throw Error("EVIDENCE: missing/ambiguous section receipts");
  return section.receipt?[section.receipt]:section.receipts!;
}
function script(root:string, key:string, expected:string, desktop=true) {
  const pkg=JSON.parse(readFileSync(resolve(root,desktop?"apps/desktop/package.json":"package.json"),"utf8"));
  if(pkg.scripts?.[key]!==expected) throw Error(`EVIDENCE: unsupported delivered script ${key}`);
  return pkg;
}
function smokeEvidence(root:string, label:string, section:Section, sourceHash:string) {
  const refs=references(section);
  if(refs.length!==1) throw Error("EVIDENCE: smoke needs one operation receipt");
  const logs=commandEvidence(root,refs[0],sourceHash);
  if(label==="frozenInstall") return select(root,logs,[{argv:["bun","install","--frozen-lockfile"],cwd:"."}]);
  const operations:Record<string,readonly [string,string,string]>={typescript:["typecheck","desktop:typecheck","tsc --noEmit -p tsconfig.json && tsc --noEmit -p tsconfig.test.json"],
    vite:["build","desktop:build","vite build"],tauri:["smoke:tauri","desktop:smoke:tauri","tauri build --debug --no-bundle"]};
  const operation=operations[label];
  if(!operation) throw Error(`EVIDENCE: unsupported smoke ${label}`);
  const [appName,rootName,body]=operation;
  select(root,logs,[{argv:["bun","run",rootName],cwd:"."},
    {argv:["bun","run","--cwd","apps/desktop",appName],cwd:"."},{argv:["bun","run",appName],cwd:"apps/desktop"}]);
  const pkg=script(root,appName,body);
  if(logs.receipt.argv[2]===rootName) script(root,rootName,`bun run --cwd apps/desktop ${appName}`,false);
  if(label==="typescript" && !/^7\.\d+\.\d+(?:[-+].*)?$/.test(pkg.devDependencies?.typescript)) throw Error("EVIDENCE: typecheck must use TypeScript 7");
  const text=plain(logs.stdout+logs.stderr);
  if(label==="vite" && (!/\bvite v\d+\./.test(text) || !/\bbuilt in\b/.test(text))) throw Error("EVIDENCE: missing Vite compilation summary");
  if(label==="tauri") {
    script(root,"build","vite build");
    const conf=JSON.parse(readFileSync(resolve(root,"apps/desktop/src-tauri/tauri.conf.json"),"utf8"));
    if(conf.build?.beforeBuildCommand!=="bun run build" || !/Finished `(?:dev|release)` profile/.test(text) || !/Built application at:/.test(text)) throw Error("EVIDENCE: missing native Tauri compilation");
  }
  return logs;
}
function nativeEvidence(root:string, section:Section, sourceHash:string, allowed:Selector[], required:readonly string[]) {
  const tests=references(section).flatMap(ref=>{
    const logs=select(root,commandEvidence(root,ref,sourceHash),allowed), execution=parseCargoExecution(plain(logs.stdout));
    if(execution.ignored || execution.tests.some(test=>test.status!=="ok") || /[1-9]\d* filtered out;/.test(logs.stdout)) throw Error("EVIDENCE: incomplete native execution");
    return execution.tests;
  });
  for(const name of required) if(!tests.some(test=>test.name===name || test.name.endsWith(`::${name}`))) throw Error(`EVIDENCE: required native test did not execute: ${name}`);
}

// Called explicitly for current-tree claims by the gate owner or reviewer.
// Completed mode requires every explicit role.
export function desktopSectionsEvidence(root:string, report:Sections, sourceHash:string, completed=false) {
  const sections = {
    frozenInstall:report.smoke?.frozenInstall,typescript:report.smoke?.typescript,vite:report.smoke?.vite,tauri:report.smoke?.tauri,
    components:report.tests?.components,tauriHandlers:report.tests?.tauriHandlers,daemonBridge:report.tests?.daemonBridge,
    compatibility:report.compatibility,
  };
  for(const [label,section] of Object.entries(sections)) {
    if(completed && section?.status!=="passed") throw Error(`EVIDENCE: missing/unfinished required section ${label}`);
    if(section?.status!=="passed") continue;
    if(["frozenInstall","typescript","vite","tauri"].includes(label)) {
      const logs=smokeEvidence(root,label,section,sourceHash);
      if(label==="frozenInstall") {
        const frozen=report.smoke!.frozenInstall!;
        const identity=JSON.parse(readFileSync(resolve(root,logs.receipt.inputBefore.manifest.path),"utf8"));
        if(frozen.lockUnchanged!==true || frozen.bunLockBefore!==identity.files["bun.lock"] || frozen.bunLockAfter!==frozen.bunLockBefore) throw Error("EVIDENCE: frozen lockfile changed/unbound");
      }
      if(label==="typescript" && (report.smoke!.typescript!.major!==7 || report.smoke!.typescript!.version!==undefined &&
        report.smoke!.typescript!.version!==JSON.parse(readFileSync(resolve(root,"apps/desktop/package.json"),"utf8")).devDependencies.typescript)) throw Error("EVIDENCE: unsupported TypeScript version");
    } else if(label==="components") {
      const tests=references(section).flatMap(ref=>{
        const logs=select(root,commandEvidence(root,ref,sourceHash),COMPONENT_COMMANDS);
        if(logs.receipt.argv.join(" ")==="bun run test") script(root,"test","bun test tests");
        const prefix=commandCwd(root,logs.receipt.cwd,"apps/desktop")?"apps/desktop/":"";
        return bunExecution(logs.stdout,logs.stderr).map(test=>({...test,file:prefix+test.file}));
      });
      for(const [name,group] of Object.entries(COMPONENT_CASES)) for(const leaf of group.names) {
        if(!tests.some(test=>test.file===`apps/desktop/tests/${name}.test.ts` && test.name===`${group.group} > ${leaf}`)) throw Error(`EVIDENCE: required component did not execute: ${name}: ${leaf}`);
      }
    } else if(label==="tauriHandlers") nativeEvidence(root,section,sourceHash,[WORKSPACE,DESKTOP,
      {argv:["cargo","test","-p","arc-desktop","--test","registered_commands","--test","lifecycle"],cwd:"."},
      {argv:["cargo","test","-p","arc-desktop","--test","lifecycle","--test","registered_commands"],cwd:"."}],HANDLER_CASES);
    else if(label==="daemonBridge") {
      nativeEvidence(root,section,sourceHash,[WORKSPACE,DESKTOP,{argv:["cargo","test","-p","arc-desktop","--test","daemon_bridge"],cwd:"."}],Object.values(DAEMON_CASES));
      if(report.tests!.daemonBridge!.realDaemon!==true || Object.keys(DAEMON_CASES).some(name=>report.tests!.daemonBridge!.cases?.[name]!=="passed")) throw Error("EVIDENCE: unfinished real-daemon cases");
    } else nativeEvidence(root,section,sourceHash,[WORKSPACE],COMPATIBILITY_CASES);
  }
}

export function activeBunEvidence(root:string, gate:Section & {fullDiscovery?:boolean}, identity:Identity) {
  if(gate?.status!=="passed" || gate.fullDiscovery!==true || !gate.receipt || gate.receipts) throw Error("EVIDENCE: incomplete active Bun gate");
  const logs=select(root,commandEvidence(root,gate.receipt,identity.sha256),[{argv:["bun","test"],cwd:"."}]);
  const executed=new Set(bunExecution(logs.stdout,logs.stderr).map(test=>test.file));
  // Matches the existing Bun discovery configuration; no new exclusions are
  // installed and the actual gate argv cannot carry filters/skip switches.
  const files=Object.keys(identity.files).filter(path=>!path.startsWith("scripts/archive/") && /[._-](?:test|spec)\.[cm]?[jt]sx?$/.test(path));
  if(!files.length || files.some(path=>!executed.has(path))) throw Error("EVIDENCE: active Bun omitted discovered test files");
  return logs;
}
