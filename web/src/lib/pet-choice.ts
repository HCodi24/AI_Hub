/** Slots a pet can be chosen for: one per Codex and Claude model, one for DeepSeek; API and CLI share a slot. No pet ships with the app: the user adds sprite sheets (8 x 6 cells) in Settings. */
export const PET_KINDS = ["codex-luna", "codex-sol", "codex-terra", "claude-opus", "claude-sonnet", "claude-haiku", "claude-fable", "deepseek"] as const;
const TIERS = { codex: ["luna", "sol", "terra"], claude: ["opus", "sonnet", "haiku", "fable"] } as const;
const FAMILY: Record<string, keyof typeof TIERS | "deepseek"> = { codex_cli: "codex", claude_cli: "claude", anthropic: "claude", deepseek: "deepseek", deepseek_agent: "deepseek" };
/** Provider kind implied by a model id alone (a task row knows its model, not its source); "" when unknown. */
export const kindFromModel = (model: string) =>
  /deepseek/i.test(model) ? "deepseek" : /luna|sol|terra|gpt/i.test(model) ? "codex_cli" : /opus|sonnet|haiku|fable|claude/i.test(model) ? "claude_cli" : "";
/** Slot of a message's source and model (e.g. `codex-luna` for `gpt-6-luna`); "" when none fits (other sources, unknown model), so no pet shows. */
export function petKind(provider: string, model = "") {
  const family = FAMILY[provider];
  if (family === "deepseek") return family;
  const name = model.toLowerCase();
  const tier = family && TIERS[family].find(tier => new RegExp(`(^|[^a-z])${tier}([^a-z]|$)`).test(name));
  return tier ? `${family}-${tier}` : "";
}
