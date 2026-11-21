export type PreviewSelection = { label: string; text: string };

export function previewSelectionText(items: PreviewSelection[]): string {
  return items.map(item => {
    const longest = (item.text.match(/`+/g) || []).reduce((max, run) => Math.max(max, run.length), 2);
    const fence = "`".repeat(longest + 1);
    return `Preview selection ${JSON.stringify(item.label)}:\n${fence}\n${item.text}\n${fence}`;
  }).join("\n\n");
}

// Keep complete selection payloads in history and provider prompts; collapse only their display.
export function splitPreviewSelectionText(content: string): { text: string; selections: PreviewSelection[] } {
  const selections: PreviewSelection[] = [];
  const text = content.replace(/^Preview selection ("(?:[^"\\\r\n]|\\.)*"):\r?\n(`{3,})\r?\n([\s\S]*?)\r?\n\2(?=\r?$)/gm, (block, label: string, _fence: string, text: string) => {
    try { selections.push({ label: JSON.parse(label), text }); return ""; }
    catch { return block; }
  });
  return { text: text.trim(), selections };
}
