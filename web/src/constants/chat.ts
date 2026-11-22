export const EFFORTS = ["default", "low", "medium", "high", "xhigh", "max"] as const;
export const CODEX_EFFORTS = ["none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra"] as const;
export const CHAT_EFFORTS = ["default", ...CODEX_EFFORTS] as const;
type ChatEffort = (typeof CHAT_EFFORTS)[number];
export const PERMISSIONS = ["read", "write", "full"] as const;
export const FILE_TOOLS = ["none", "read", "write"] as const;

export function isDeepSeekFlash(model: string, kind = "") {
  const name = model.toLowerCase().replace(/[-_.\s]/g, "");
  return !kind.endsWith("_cli") && (name.includes("deepseekv41flash") || name === "deepseekflash");
}
const isSol62 = (model: string) => model.toLowerCase() === "gpt-6.2-sol";
export function effortsFor(model: string, kind = ""): readonly (typeof EFFORTS)[number][] {
  if (isDeepSeekFlash(model, kind)) return ["low", "high", "max"];
  return isSol62(model) ? ["low", "medium", "high"] : EFFORTS;
}
export function effortFor(model: string, effort: ChatEffort, kind = "") {
  const matching = effortsFor(model, kind).find(choice => choice === effort);
  return matching ?? (isDeepSeekFlash(model, kind) ? "high" : isSol62(model) ? "medium" : "default");
}

export function modelEffortOptions(metadata: { supported: string[]; default?: string } | undefined, selected: ChatEffort, kind: string) {
  if (kind === "claude_cli") {
    const choices = EFFORTS.filter(effort => effort === "default" || metadata?.supported.includes(effort));
    return { choices, selected: choices.find(effort => effort === selected) ?? "default", reset: "default" } as const;
  }
  const choices = CODEX_EFFORTS.filter(effort => metadata?.supported.includes(effort));
  const reportedDefault = CODEX_EFFORTS.find(effort => effort === metadata?.default);
  if (!choices.length && reportedDefault) choices.push(reportedDefault);
  const reset = choices.find(effort => effort === reportedDefault) ?? choices[0] ?? "default";
  return { choices, selected: choices.find(effort => effort === selected) ?? reset, reset };
}
