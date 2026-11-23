import { modelGroup } from "./model-group";

export const PET_FAMILIES = ["codex", "claude", "antigravity", "deepseek"] as const;
type PetFamily = (typeof PET_FAMILIES)[number];
export const PET_FAMILY: Partial<Record<string, PetFamily>> = { codex_cli: "codex", claude_cli: "claude", anthropic: "claude", antigravity_cli: "antigravity", deepseek: "deepseek", deepseek_agent: "deepseek" };
/** Provider kind implied by a model id alone (a task row knows its model, not its source); "" when unknown. */
export const kindFromModel = (model: string) =>
  /deepseek/i.test(model) ? "deepseek" : /luna|sol|terra|gpt/i.test(model) ? "codex_cli" : /opus|sonnet|haiku|fable|claude/i.test(model) ? "claude_cli" : "";
export function petKind(provider: string, model = ""): string {
  const family = PET_FAMILY[provider];
  if (!family || family === "deepseek") return family ?? "";
  if (!model) return family === "antigravity" ? family : "";
  return `${family}-${modelGroup(model, provider).toLowerCase()}`;
}

export function petForSlot(pets: Record<string, string>, slot: string): string | undefined {
  // Older settings used one Antigravity pet for all groups. An explicit empty slot disables it for that group.
  return (pets[slot] ?? (slot.startsWith("antigravity-") ? pets.antigravity : undefined)) || undefined;
}
