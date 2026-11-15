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

/** One cumulative point of a model series: totals after the reply at `step` (step 0 is the shared origin). */
export type TokenSeriesPoint = { step: number; input: number; output: number; total: number };
export type ModelTokenSeries = { model: string; points: TokenSeriesPoint[] };
export type ChatTokenSeries = { models: string[]; steps: number; series: ModelTokenSeries[]; max: number };

/**
 * Cumulative per-model series in reply order, built only from the chat logs.
 * Every model starts at the shared step 0 with zero tokens, including models that first report later.
 * Each reported assistant reply adds its own input+output to its model and carries the other models
 * forward, so the last point of every series equals chatTokenUsage(messages) for that model.
 * Repeated reports inside one message are replacement snapshots: only the last valid one counts.
 * No wall-clock timestamp is derived and the original logs are never modified.
 */
export function chatTokenSeries(messages: ChatMessage[]): ChatTokenSeries {
  const reports: { model: string; input: number; output: number }[] = [];
  for (const message of messages) {
    if (message.role !== "assistant") continue;
    const report = lastTokenReport(message.logs);
    if (!report) continue;
    reports.push({ model: message.model || "", input: report.input, output: report.output });
  }
  const models: string[] = [];
  for (const report of reports) if (!models.includes(report.model)) models.push(report.model);
  const running = new Map<string, { input: number; output: number }>();
  for (const model of models) running.set(model, { input: 0, output: 0 });
  const series: ModelTokenSeries[] = models.map(model => ({
    model, points: [{ step: 0, input: 0, output: 0, total: 0 }],
  }));
  reports.forEach((report, index) => {
    const own = running.get(report.model);
    if (own) {
      own.input += report.input;
      own.output += report.output;
    }
    for (const entry of series) {
      const totals = running.get(entry.model) ?? { input: 0, output: 0 };
      entry.points.push({ step: index + 1, input: totals.input, output: totals.output, total: totals.input + totals.output });
    }
  });
  const max = series.reduce((value, entry) => entry.points.reduce((inner, point) => Math.max(inner, point.total), value), 0);
  return { models, steps: reports.length, series, max };
}

/** Exact, locale-stable numbers so the chart and its text equivalents always agree. */
export function formatTokens(value: number): string {
  return Math.round(value).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ".");
}

/** One reported assistant reply in chat order (same reports as chatTokenSeries), for per-reply views. */
export type TokenReply = { step: number; model: string; input: number; output: number; total: number };
export function chatTokenReplies(messages: ChatMessage[]): TokenReply[] {
  const replies: TokenReply[] = [];
  for (const message of messages) {
    if (message.role !== "assistant") continue;
    const report = lastTokenReport(message.logs);
    if (report) replies.push({ step: replies.length + 1, model: message.model || "", ...report, total: report.input + report.output });
  }
  return replies;
}

/** Short axis/tile number: 950, 12,5K, 1,03M. */
export function formatCompact(value: number): string {
  const trim = (n: number, digits: number) => n.toFixed(digits).replace(/\.?0+$/, "").replace(".", ",");
  if (value >= 1e6) return `${trim(value / 1e6, 2)}M`;
  if (value >= 1e3) return `${trim(value / 1e3, 1)}K`;
  return String(Math.round(value));
}
