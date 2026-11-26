function cleanChoiceSource(source: string): string {
  return source
    .replace(/<memory\b[^>]*>[\s\S]*?(?:<\/memory\s*>|$)/gi, "")
    .replace(/```[\s\S]*?(?:```|$)|~~~[\s\S]*?(?:~~~|$)/g, "")
    .replace(/^\s*>.*$/gm, "")
    .replace(/`[^`\n]*`/g, "")
    .replace(/[*_]/g, "");
}

export function chatChoiceQuestion(source: string): string {
  const text = cleanChoiceSource(source);
  const end = text.indexOf("?");
  if (end === -1 || end !== text.lastIndexOf("?")) return "";
  return text.slice(0, end + 1).split(/[.!\n]/).at(-1)?.trim() ?? "";
}

export function stripChatChoiceQuestion(source: string): string {
  const question = chatChoiceQuestion(source);
  if (!question) return source;
  const index = source.lastIndexOf(question);
  if (index === -1) return source;
  return (source.slice(0, index) + source.slice(index + question.length)).trimEnd();
}

// Plain yes/no replies remain supported alongside structured question cards.
export function chatChoices(source: string): string[] {
  const text = cleanChoiceSource(source)
    .normalize("NFD").replace(/[\u0300-\u036f]/g, "").replace(/đ/g, "d").replace(/Đ/g, "D");
  // Multiple questions need separate answers, so a single pair of buttons would be ambiguous.
  if ((text.match(/\?/g) || []).length !== 1) return [];
  const question = text.slice(0, text.indexOf("?")).split(/[.!\n]/).at(-1)?.trim() || "";
  const explicit = (yes: string, no: string) =>
    new RegExp(`^\\s*(?:[-+•]|\\d+[.)])?\\s*${yes}\\s*:`, "im").test(text) &&
    new RegExp(`^\\s*(?:[-+•]|\\d+[.)])?\\s*${no}\\s*:`, "im").test(text);
  if (explicit("co", "khong") || /\b(?:ban|anh|chi|em)\b.*\b(?:muon|dong y|can)\b.*\bkhong\s*$/i.test(question) || /\bco\b.+\bkhong\s*$/i.test(question)) return ["Có", "Không"];
  if (explicit("yes", "no") || /^(?:do|does|did|would|will|can|could|should|are|is|may)\s+(?:you|i|we)\b/i.test(question)) return ["Yes", "No"];
  return [];
}
