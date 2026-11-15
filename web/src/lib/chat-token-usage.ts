import type { ChatMessage } from "@/schema/chat.schema";

/** One recorded token report: input/output only. Cache and reasoning are subsets/metadata and never added again. */
export type TokenReport = { input: number; output: number };
export type ModelTokenUsage = { model: string; input: number; output: number; total: number; reports: number };
export type ChatTokenUsage = { models: ModelTokenUsage[]; input: number; output: number; total: number; reports: number };

/** Anchored to the existing log lines, e.g. `Token: vào 1200 (cache 300), ra 45 (suy luận 10)`. */
const REPORT = /^\s*Token:\s*vào\s+(\d+)(?:\s*\([^)]*\))?\s*,\s*ra\s+(\d+)(?:\s*\([^)]*\))?\s*(?:,\s*chi phí\s*\$?\s*[\d.,]+)?\s*$/i;
const COUNT = /^\d+$/;

function count(raw: string): number | undefined {
  if (!COUNT.test(raw)) return;
  const value = Number(raw);
  return Number.isFinite(value) && value >= 0 ? value : undefined;
}

/** The last valid report of one message, matching the summary the chat UI already shows. */
export function lastTokenReport(logs: string[]): TokenReport | undefined {
  for (let index = logs.length - 1; index >= 0; index -= 1) {
    const match = REPORT.exec(logs[index]);
    if (!match) continue;
    const input = count(match[1]);
    const output = count(match[2]);
    if (input === undefined || output === undefined) continue;
    return { input, output };
  }
  return;
}

/** Aggregates recorded reply reports of the current chat by the exact model stored on each assistant message. */
export function chatTokenUsage(messages: ChatMessage[]): ChatTokenUsage {
  const groups = new Map<string, ModelTokenUsage>();
  for (const message of messages) {
    if (message.role !== "assistant") continue;
    const report = lastTokenReport(message.logs);
    if (!report) continue;
    const model = message.model || "";
    const group = groups.get(model) ?? { model, input: 0, output: 0, total: 0, reports: 0 };
    group.input += report.input;
    group.output += report.output;
    group.total = group.input + group.output;
    group.reports += 1;
    groups.set(model, group);
  }
  const models = [...groups.values()].sort((a, b) => b.total - a.total || a.model.localeCompare(b.model));
  return {
    models,
    input: models.reduce((sum, group) => sum + group.input, 0),
    output: models.reduce((sum, group) => sum + group.output, 0),
    total: models.reduce((sum, group) => sum + group.total, 0),
    reports: models.reduce((sum, group) => sum + group.reports, 0),
  };
}

/** Exact, locale-stable numbers so the chart and its text equivalents always agree. */
export function formatTokens(value: number): string {
  return Math.round(value).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ".");
}
