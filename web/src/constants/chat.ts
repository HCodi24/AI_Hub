export const EFFORTS = ["default", "low", "medium", "high", "xhigh", "max"] as const;
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
export function effortFor(model: string, effort: (typeof EFFORTS)[number], kind = "") {
  if (effortsFor(model, kind).includes(effort)) return effort;
  return isDeepSeekFlash(model, kind) ? "high" : isSol62(model) ? "medium" : effort;
}
