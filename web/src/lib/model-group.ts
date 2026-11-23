/** The top-level model group shown in source settings. */
export function modelGroup(model: string, kind = ""): string {
  const id = model.replace(/\[.*\]$/, "");
  if (kind === "antigravity_cli") return /^gemini/i.test(id) ? "gemini" : /^claude/i.test(id) ? "claude" : /^(gpt|chatgpt|openai|o\d)/i.test(id) ? "chatgpt" : "";
  if (/^claude-/i.test(id)) return id.split("-").slice(1).find(part => /^[a-z]+$/i.test(part)) ?? "";
  if (/^[a-z]+$/i.test(id)) return id;
  const cut = id.lastIndexOf("-");
  return cut > 0 && cut < id.length - 1 ? id.slice(cut + 1) : "";
}
