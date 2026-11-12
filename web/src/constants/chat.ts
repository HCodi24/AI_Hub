export const EFFORTS = ["default", "low", "medium", "high", "xhigh", "max"] as const;
export const PERMISSIONS = ["read", "write", "full"] as const;
export const FILE_TOOLS = ["none", "read", "write"] as const;

export function isDeepSeekFlash(model: string, kind = "") {
  const name = model.toLowerCase().replace(/[-_.\s]/g, "");
  return !kind.endsWith("_cli") && (name.includes("deepseekv41flash") || name === "deepseekflash");
}
export function effortsFor(model: string, kind = ""): readonly (typeof EFFORTS)[number][] {
  return isDeepSeekFlash(model, kind) ? ["low", "high", "max"] : EFFORTS;
}
export function effortFor(model: string, effort: (typeof EFFORTS)[number], kind = "") {
  return isDeepSeekFlash(model, kind) && !["low", "high", "max"].includes(effort) ? "high" : effort;
}
