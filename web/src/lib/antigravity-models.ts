import { CODEX_EFFORTS } from "@/constants/chat";

export const ANTIGRAVITY_EFFORTS = [...CODEX_EFFORTS, "thinking"] as const;

export function antigravityModel(model: string) {
  const cut = model.lastIndexOf("-");
  const effort = cut > 0 ? ANTIGRAVITY_EFFORTS.find(level => level === model.slice(cut + 1).toLowerCase()) : undefined;
  return { id: effort ? model.slice(0, cut) : model, effort };
}
