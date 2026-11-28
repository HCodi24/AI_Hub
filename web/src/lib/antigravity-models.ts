import { CODEX_EFFORTS } from "@/constants/chat";

export const ANTIGRAVITY_EFFORTS = [...CODEX_EFFORTS, "thinking"] as const;
type VariantEffort = "default" | (typeof ANTIGRAVITY_EFFORTS)[number];

export function antigravityModel(model: string) {
  const cut = model.lastIndexOf("-");
  const effort = cut > 0 ? ANTIGRAVITY_EFFORTS.find(level => level === model.slice(cut + 1).toLowerCase()) : undefined;
  return { id: effort ? model.slice(0, cut) : model, effort };
}

export function antigravityVariants(models: readonly string[]) {
  const groups = new Map<string, Map<VariantEffort, string>>();
  for (const model of models) {
    const { id, effort = "default" } = antigravityModel(model);
    const variants = groups.get(id) ?? new Map<VariantEffort, string>();
    if (!variants.has(effort)) variants.set(effort, model);
    groups.set(id, variants);
  }
  return groups;
}
