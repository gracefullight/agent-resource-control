import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { hash } from "./source";
import { boundBytes, equal, requireTraffic, trafficInputs } from "./traffic-receipts";

type Target = { id: string; kind: "docker" | "kubernetes"; container: string; context?: string; namespace?: string; pod?: string };
type Result = { status: number | null; stdout: Buffer; stderr: Buffer; error?: unknown };
type Runner = (argv: string[]) => Result;
type InventoryRunner = (cwd: string) => { status: number | null; stdout: string; error?: unknown };
const safe = (s: unknown) => typeof s === "string" && /^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/.test(s);
function normalizeMission(row: any) {
  const object = (value: any) => value && typeof value === "object" && !Array.isArray(value);
  requireTraffic(object(row) && (!Object.hasOwn(row, "mission") || object(row.mission)), "saved mission policy unavailable");
  const missions = [row, ...(row.mission ? [row.mission] : [])];
  const ids = missions.flatMap(mission => ["id", "missionId"].filter(key => Object.hasOwn(mission, key)).map(key => mission[key]));
  requireTraffic(ids.length > 0 && ids.every(id => typeof id === "string" && id.length > 0 && id === ids[0]),
    "saved mission identity unavailable or inconsistent");
  for (const mission of missions) requireTraffic(mission.operatingPolicy === undefined || object(mission.operatingPolicy),
    "saved mission policy unavailable");
  const scopes = missions.flatMap(mission => [mission, mission.operatingPolicy ?? {}]);
  const keys = ["containerObservation", "container_observation", "serviceObservation", "service_observation"];
  const entries = scopes.flatMap(scope => keys.filter(key => Object.hasOwn(scope, key)).map(key => scope[key]));
  if (entries.length === 0) return { id: ids[0] as string, policy: null };
  requireTraffic(entries.length === 1 && entries[0] && typeof entries[0].enabled === "boolean" &&
    Array.isArray(entries[0].targets), "saved target policy unavailable");
  return { id: ids[0] as string, policy: entries[0] };
}
export function missionObservationPolicy(mission: any) { return normalizeMission(mission).policy; }
export function readSavedMission(root: string, missionId: string, explicit = process.env.ARC_TRAFFIC_MISSION_PATH) {
  requireTraffic(typeof missionId === "string" && /^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$/.test(missionId), "invalid saved mission identity");
  let bytes: string | undefined;
  if (explicit) bytes = readFileSync(resolve(explicit), "utf8");
  else {
    let dir = resolve(root);
    while (true) {
      try {
        bytes = readFileSync(resolve(dir, ".agent-valley/missions", `${missionId}.json`), "utf8");
        break;
      } catch (error: any) {
        if (error?.code !== "ENOENT" && error?.code !== "ENOTDIR") throw error;
      }
      const parent = dirname(dir);
      if (parent === dir) break;
      dir = parent;
    }
  }
  requireTraffic(bytes, "saved mission unavailable; set ARC_TRAFFIC_MISSION_PATH");
  const mission = JSON.parse(bytes);
  requireTraffic(normalizeMission(mission).id === missionId, "saved mission identity differs");
  return mission;
}
export function policyFromConfig(config: any) {
  requireTraffic(config && typeof config === "object" && config.chief && typeof config.chief === "object", "target configuration unavailable");
  const entries = ["containerObservation", "container_observation", "serviceObservation", "service_observation"]
    .filter(key => Object.hasOwn(config.chief, key)).map(key => config.chief[key]);
  requireTraffic(entries.length === 1 && entries[0] && typeof entries[0].enabled === "boolean" &&
    Array.isArray(entries[0].targets), "supported target configuration unavailable");
  return entries[0];
}
function run(argv: string[]): Result {
  const result = spawnSync(argv[0], argv.slice(1), { encoding: "buffer", timeout: 10_000, maxBuffer: 1_000_000 });
  return { status: result.status, stdout: result.stdout ?? Buffer.alloc(0), stderr: result.stderr ?? Buffer.alloc(0), error: result.error };
}
function checked(runner: Runner, argv: string[]) {
  const result = runner(argv);
  requireTraffic(result.status === 0 && !result.error, `target observation unavailable: ${argv[0]}`);
  return result;
}
export function configuredPolicy(root: string, missionId: string, inventory?: InventoryRunner) {
  // Both the live configuration and the saved mission must explicitly expose
  // a supported inventory. An omitted, denied or unreadable config is unknown.
  const configBytes = readFileSync(resolve(root, "av.yaml"));
  const configured = policyFromConfig(Bun.YAML.parse(configBytes.toString()));
  const result = inventory?.(root);
  if (inventory) requireTraffic(result && result.status === 0 && !result.error, "saved mission inventory unavailable");
  const rows = inventory ? JSON.parse(result!.stdout) : [readSavedMission(root, missionId)];
  requireTraffic(Array.isArray(rows), "invalid saved mission inventory");
  const matches = rows.filter((row: any) => (row?.id ?? row?.missionId ?? row?.mission?.id ?? row?.mission?.missionId) === missionId);
  requireTraffic(matches.length === 1, "saved mission target policy unavailable");
  const { id, policy } = normalizeMission(matches[0]);
  requireTraffic(id === missionId && policy, "saved target policy unavailable");
  requireTraffic(equal(policy, configured), "saved and configured target inventories differ");
  return { policy, configSha256: hash(configBytes) };
}
export function observePolicy(policy: any, runner: Runner = run) {
  requireTraffic(policy && typeof policy.enabled === "boolean" && Array.isArray(policy.targets), "target inventory unavailable");
  const targets: Target[] = policy.targets;
  requireTraffic(targets.every(t => t && typeof t.id === "string" && /^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$/.test(t.id) &&
    ["docker", "kubernetes"].includes(t.kind) && safe(t.container) &&
    (t.context === undefined || safe(t.context)) && (t.kind !== "kubernetes" || (safe(t.namespace) && safe(t.pod)))) &&
    new Set(targets.map(t => t.id)).size === targets.length, "invalid or duplicate configured target");
  const since = policy.log_since_sec ?? policy.logSinceSec ?? 300, tail = policy.log_tail ?? policy.logTail ?? 50;
  requireTraffic(Number.isSafeInteger(since) && since > 0 && since <= 86400 && Number.isSafeInteger(tail) && tail > 0 && tail <= 10000, "invalid log window");
  return targets.map(target => {
    let state: { identity: string; healthy: boolean; restarts: number }, logs: Result;
    if (target.kind === "docker") {
      const base = ["docker", ...(target.context ? ["--context", target.context] : [])];
      const inspect = [...base, "container", "inspect", "--format", "{{.Id}} {{.RestartCount}} {{json .State}}", "--", target.container];
      const dockerState = () => {
        const data = checked(runner, inspect).stdout.toString().trim(), match = data.match(/^([a-f0-9]{12,64}) (\d+) (\{.*\})$/s);
        requireTraffic(match, "invalid Docker state");
        const identity = match[1], restarts = Number(match[2]), value = JSON.parse(match[3]);
        requireTraffic(Number.isSafeInteger(restarts) && value && typeof value.Running === "boolean" &&
          value.Health && typeof value.Health.Status === "string", "Docker health observation unavailable");
        return { identity, healthy: value.Running === true && value.Status === "running" && value.OOMKilled === false &&
          value.Health.Status === "healthy", restarts };
      };
      state = dockerState();
      logs = checked(runner, [...base, "container", "logs", "--timestamps", "--since", `${since}s`, "--tail", String(tail), "--", target.container]);
      const after = dockerState();
      requireTraffic(state.identity === after.identity && state.restarts === after.restarts && state.healthy && after.healthy, "Docker state changed or unhealthy");
    } else {
      const base = ["kubectl", ...(target.context ? ["--context", target.context] : [])];
      const inspect = [...base, "get", "pod", target.pod!, "--namespace", target.namespace!, "-o", "json"];
      const podState = () => {
        const pod = JSON.parse(checked(runner, inspect).stdout.toString());
        const statuses = pod?.status?.containerStatuses;
        requireTraffic(typeof pod?.metadata?.uid === "string" && Array.isArray(statuses), "invalid Kubernetes identity or status");
        const matches = statuses.filter((s: any) => s.name === target.container);
        requireTraffic(matches.length === 1 && Number.isSafeInteger(matches[0].restartCount) && matches[0].restartCount >= 0, "container status unavailable");
        const status = matches[0];
        return { identity: pod.metadata.uid, healthy: status.ready === true && !!status.state?.running, restarts: status.restartCount };
      };
      state = podState();
      logs = checked(runner, [...base, "logs", target.pod!, "--namespace", target.namespace!, "--container", target.container,
        "--timestamps", `--since=${since}s`, `--tail=${tail}`]);
      const after = podState();
      requireTraffic(state.identity === after.identity && state.restarts === after.restarts && state.healthy && after.healthy, "Kubernetes state changed or unhealthy");
    }
    const bytes = Buffer.concat([logs.stdout, Buffer.from("\n"), logs.stderr]);
    // Log text is data only; it is never executed or used as instructions.
    const errors = bytes.toString().split(/\r?\n/).filter(line => /\b(?:error|fatal|panic|exception)\b/i.test(line)).length;
    requireTraffic(errors === 0, `recent error evidence for target ${target.id}`);
    return { id: target.id, kind: target.kind, targetHash: hash(JSON.stringify(target)), identity: state.identity,
      healthy: state.healthy, restarts: state.restarts, recentErrors: errors, logBytes: bytes };
  });
}
export function observeTargets(root: string, missionId: string, inventory?: InventoryRunner) {
  const { policy, configSha256 } = configuredPolicy(root, missionId, inventory), observed = observePolicy(policy), runId = crypto.randomUUID();
  const directory = `target/traffic/targets/${runId}`;
  mkdirSync(resolve(root, directory), { recursive: true });
  const targets = observed.map(({ logBytes, ...item }) => {
    const path = `${directory}/${item.id}.log`;
    writeFileSync(resolve(root, path), logBytes, { flag: "wx" });
    return { ...item, log: { path, sha256: hash(logBytes) } };
  });
  return { inventoryVerified: true, allQualified: true, missionId, sourceHash: trafficInputs(root).sha256, configSha256, policySha256: hash(JSON.stringify(policy)),
    observedAt: new Date().toISOString(), targets };
}
export function validateTargets(root: string, report: any, live: ReturnType<typeof observeTargets>) {
  const age = Date.now() - Date.parse(report?.observedAt);
  requireTraffic(report?.inventoryVerified === true && report.allQualified === true && report.missionId === live.missionId &&
    report.sourceHash === live.sourceHash && /^[a-f0-9]{64}$/.test(report.configSha256) && report.configSha256 === live.configSha256 &&
    report.policySha256 === live.policySha256 && Number.isFinite(age) && age >= 0 && age <= 300_000 &&
    Array.isArray(report.targets) && report.targets.length === live.targets.length, "target inventory missing, stale or inconsistent");
  const fields = (t: any) => [t.id, t.kind, t.targetHash, t.identity, t.healthy, t.restarts, t.recentErrors];
  requireTraffic(equal(report.targets.map(fields), live.targets.map(fields)), "configured target or current state differs");
  for (const target of report.targets) {
    const log = boundBytes(root, target.log, "target/traffic").toString();
    requireTraffic(!/\b(?:error|fatal|panic|exception)\b/i.test(log) && target.recentErrors === 0, "recent target errors or altered logs");
  }
  return true;
}
export function validateMissionTargets(root: string, mission: any) {
  const { id, policy } = normalizeMission(mission);
  // A mission without an observation policy has no configured targets to
  // inspect. Do not claim that an inventory or service was observed.
  if (policy === null) return { applicable: false, targets: 0, reason: "no container or service observation policy configured" };
  const live = observeTargets(root, id, () => ({ status: 0, stdout: JSON.stringify([mission]) }));
  requireTraffic(live.policySha256 === hash(JSON.stringify(policy)), "saved target policy changed");
  const report = JSON.parse(readFileSync(resolve(root, ".arc-verify/traffic/targets.json"), "utf8"));
  validateTargets(root, report, live);
  return { applicable: true, targets: live.targets.length };
}
if (import.meta.main) {
  const root = resolve(import.meta.dir, "../.."), id = "af4d6fdb-505c-4e6d-8cc2-c5d58682b3ea";
  const mission = readSavedMission(root, id);
  const result = missionObservationPolicy(mission) === null ?
    { applicable: false, targets: 0, reason: "no container or service observation policy configured" } : observeTargets(root, id);
  console.log(JSON.stringify(result, null, 2));
}
