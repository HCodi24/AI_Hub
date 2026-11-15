import type { ChatTokenUsage } from "./chat-token-usage";

export type TokenSample = { at: number; models: { model: string; input: number; output: number; total: number }[] };
export const TOKEN_HISTORY_LIMIT = 500;
const key = (id: string) => `ai-hub-token-timeline:${id}`;
const validCount = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0;

/** Reject corrupt history rather than plotting invented measurements. */
export function parseTokenHistory(raw: string | null): TokenSample[] {
  try {
    const data: unknown = JSON.parse(raw || "[]");
    if (!Array.isArray(data)) return [];
    let previous = -1;
    const result: TokenSample[] = [];
    for (const sample of data) {
      if (!sample || !validCount(sample.at) || sample.at <= previous || !Array.isArray(sample.models)) return [];
      const names = new Set<string>();
      const models: TokenSample["models"] = [];
      for (const row of sample.models) {
        if (!row || typeof row.model !== "string" || names.has(row.model) ||
          !validCount(row.input) || !validCount(row.output) || !validCount(row.total) || row.total !== row.input + row.output) return [];
        names.add(row.model);
        models.push({ model: row.model, input: row.input, output: row.output, total: row.total });
      }
      if (!models.length) return [];
      models.sort((a, b) => a.model.localeCompare(b.model));
      result.push({ at: sample.at, models });
      previous = sample.at;
    }
    return result.slice(-TOKEN_HISTORY_LIMIT);
  } catch { return []; }
}

/** Snapshots contain current cumulative totals, never sums of earlier snapshots. */
export function appendTokenSample(history: TokenSample[], usage: ChatTokenUsage, now: number): TokenSample[] {
  if (!usage.reports || !validCount(now)) return history;
  const models = usage.models.map(({ model, input, output, total }) => ({ model, input, output, total }))
    .sort((a, b) => a.model.localeCompare(b.model));
  const last = history.at(-1);
  if (last && JSON.stringify(last.models) === JSON.stringify(models)) return history;
  const at = Math.max(now, last ? last.at + 1 : 0);
  if (!validCount(at)) return history;
  const sample = parseTokenHistory(JSON.stringify([{ at, models }]))[0];
  return sample ? [...history, sample].slice(-TOKEN_HISTORY_LIMIT) : history;
}

export function readTokenHistory(chatId: string): TokenSample[] {
  try {
    return chatId && typeof window !== "undefined" ? parseTokenHistory(window.localStorage.getItem(key(chatId))) : [];
  } catch { return []; }
}

export function writeTokenHistory(chatId: string, history: TokenSample[]): void {
  try {
    if (chatId && typeof window !== "undefined") window.localStorage.setItem(key(chatId), JSON.stringify(history.slice(-TOKEN_HISTORY_LIMIT)));
  } catch { /* Keep observations in memory when browser storage is unavailable. */ }
}
