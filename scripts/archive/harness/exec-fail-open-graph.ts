import { resolve } from "node:path";
import { requireValue, equal, hash, directory, root as productRoot } from "./exec-fail-open-inputs";
export type Node = { id: string; requires: string[] };
export function validateGraph(nodes: Node[]) {
  const seen = new Set<string>();
  for (const node of nodes) {
    requireValue(!seen.has(node.id) && node.requires.every(p => seen.has(p)), "cycle, duplicate or successor dependency");
    seen.add(node.id);
  }
}
// Domain validators bind only predecessors. Terminal observations are a sink.
export function predecessorBindings(nodes: Node[], id: string, observations: Record<string, { sha256: string }>) {
  validateGraph(nodes);
  const node = nodes.find(n => n.id === id);
  requireValue(node && equal(Object.keys(observations).sort(), [...node.requires].sort()), "missing predecessor or added successor dependency");
  for (const value of Object.values(observations)) requireValue(/^[a-f0-9]{64}$/.test(value.sha256), "unbound predecessor");
  return hash(JSON.stringify(observations));
}
const futureArtifacts = ["qa/review.md", "qa/review.json", "final.md", "final.json", "terminal.json"];
const terminalCommands = ["exec-fail-open-gates.test.ts", "exec-fail-open-evidence.test.ts"];
function terminalCommand(value: string, cwd: string, terminals: Set<string>): boolean {
  // Tokenize the recorded command; never evaluate shell text.
  const words = value.match(/"[^"]*"|'[^']*'|&&|\|\||\d*(?:>>?|<<?)(?:&\d+)?|[;|()]|(?:\\.|[^\s\\"';&|()<>])+/g) ?? [];
  let start = true;
  for (let i = 0; i < words.length; i++) {
    const word = words[i].trim(), token = word.replace(/^(["'])(.*)\1$/, "$2").replace(/\\(.)/g, "$1");
    if (["&&", "||", ";", "|", "(", ")"].includes(token)) { start = true; continue; }
    if (/^\d*(?:>>?|<<?)(?:&\d+)?$/.test(token)) { if (!/&\d+$/.test(token)) i++; continue; }
    if (start && token === "cd" && words[i + 1]) {
      cwd = resolve(cwd, words[++i].replace(/^(["'])(.*)\1$/, "$2")); start = false; continue;
    }
    if (terminals.has(resolve(cwd, token))) return true;
    if (token !== word && /\s/.test(token) && terminalCommand(token, cwd, terminals)) return true;
    start = false;
  }
  return false;
}
export function rejectFutureReferences(value: unknown, message = "gate requires successor or its own terminal validation", root = productRoot) {
  const successors = new Set(futureArtifacts.map(path => resolve(root, directory, path)));
  const terminals = new Set(terminalCommands.map(path => resolve(root, "scripts/harness", path)));
  const visit = (v: any, key?: string, commandContext = false, cwd = resolve(root)) => {
    if (typeof v === "string") {
      const artifact = key === "path" && successors.has(resolve(root, v));
      const commandField = key === "path" || key === "argv" || key === "command" || key === "commands" || key === "args" && commandContext;
      const command = commandField && terminalCommand(v, key === "path" && !commandContext ? resolve(root) : cwd, terminals);
      requireValue(!artifact && !command, message);
    } else if (Array.isArray(v)) v.forEach(item => visit(item, key, commandContext, cwd));
    else if (v && typeof v === "object") {
      const nextCwd = typeof v.cwd === "string" ? resolve(cwd, v.cwd) : cwd;
      Object.entries(v).forEach(([field, item]) => visit(item, field,
        commandContext || field === "command" || field === "commands" || field === "argv", nextCwd));
    }
  };
  visit(value);
}
