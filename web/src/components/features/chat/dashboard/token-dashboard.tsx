import { useMemo, useState } from "react";
import { chatText } from "@/text/chat";
import { chatTokenReplies, chatTokenSeries, chatTokenUsage, formatCompact, formatTokens } from "@/lib/chat-token-usage";
import type { TokenSample } from "@/lib/chat-token-timeline";
import { useChatStore } from "../chat-store";
import { useTokenTimeline, type TokenTimelineStatus } from "./use-token-timeline";
import styles from "./token-dashboard.module.css";

// Fixed categorical palette; the colour follows the model's first-appearance order in the chat.
const COLORS = ["#2a78d6", "#eb6834", "#1baf7a", "#8b5cf6", "#e87ba4", "#eda100", "#0e7490", "#e34948"];
const appearance = (model: string, order: string[]) => ({ color: COLORS[Math.max(0, order.indexOf(model)) % COLORS.length] });
const timeLabel = (at: number) => new Date(at).toLocaleTimeString("vi-VN", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false });
const modelName = (model: string) => model || chatText.tokenDashboard.unknownModel;

/**
 * Main chart: cumulative tokens per model in reply order, built from the chat logs only.
 * Every series starts at the shared step 0 with zero tokens, so no wall-clock timestamp is needed
 * and the chart stays correct when the backend history has no samples.
 */
function CumulativeChart({ series }: { series: ReturnType<typeof chatTokenSeries> }) {
  const t = chatText.tokenDashboard;
  const steps = Math.max(1, series.steps);
  const max = Math.max(1, series.max);
  // Clamped, finite coordinates: zero counts and single-report charts never divide by zero.
  const x = (step: number) => 42 + (Number.isFinite(step) ? Math.min(Math.max(step, 0), steps) / steps : 0) * 300;
  const y = (total: number) => 175 - (Number.isFinite(total) ? Math.min(Math.max(total, 0), max) / max : 0) * 150;
  const ranked = series.series.map(entry => ({ entry, last: entry.points.at(-1), ...appearance(entry.model, series.models) }))
    .sort((a, b) => (b.last?.total ?? 0) - (a.last?.total ?? 0));
  return <>
    <svg className={styles.chart} viewBox="0 0 360 215" role="img" aria-label={t.cumulativeTitle}>
      <title>{t.cumulativeTitle}</title>
      <desc>{t.cumulativeDesc}</desc>
      <text x="0" y="10" className={styles.axisTitle}>{t.cumulativeAxis}</text>
      {[0, 1, 2, 3].map(i => <g key={i}>
        <path d={`M42 ${y(max * i / 3)}H342`} stroke={i ? "#e2e8f0" : "#94a3b8"} />
        <text x="36" y={y(max * i / 3) + 4} textAnchor="end">{formatCompact(max * i / 3)}</text>
      </g>)}
      <text x="42" y="192">{formatTokens(0)}</text>
      {series.steps > 0 && <text x="342" y="192" textAnchor="end">{formatTokens(series.steps)}</text>}
      <text x="192" y="210" textAnchor="middle" className={styles.axisTitle}>{t.replyAxis}</text>
      {[...ranked].reverse().map(({ entry, color, last }) => {
        const d = entry.points.map((point, i) => i ? `H${x(point.step)}V${y(point.total)}` : `M${x(point.step)} ${y(point.total)}`).join(" ");
        return <g key={`series:${entry.model}`}>
          <path d={d} fill="none" stroke={color} strokeWidth="2.5" strokeLinejoin="round" strokeLinecap="round" />
          {last && <circle cx={x(last.step)} cy={y(last.total)} r="4.5" fill={color} stroke="#fff" strokeWidth="2">
            <title>{`${modelName(entry.model)} — ${t.modelRow(modelName(entry.model), formatTokens(last.input), formatTokens(last.output))}; ${t.cumulativeAxis}: ${formatTokens(last.total)}`}</title>
          </circle>}
        </g>;
      })}
    </svg>
    <ul className={styles.legend}>{ranked.map(({ entry, color, last }) => <li key={`legend:${entry.model}`}>
      <i className={styles.swatch} style={{ background: color }} />
      <span className={styles.legendName} title={modelName(entry.model)}>{modelName(entry.model)}</span>
      <span className={styles.legendValue}>{formatCompact(last?.total ?? 0)}</span>
    </li>)}</ul>
  </>;
}

/** Exact per-model input/output/total table, collapsed below the main chart. */
function ExactTable({ usage }: { usage: ReturnType<typeof chatTokenUsage> }) {
  const t = chatText.tokenDashboard;
  return <details className={styles.details}>
    <summary>{t.exactDetails}</summary>
    <div className={styles.tableWrap}><table><caption>{t.exactTitle}</caption>
      <thead><tr><th>{t.modelColumn}</th><th>{t.input}</th><th>{t.output}</th><th>{t.tokenAxis}</th></tr></thead>
      <tbody>{usage.models.map(group => <tr key={`model:${group.model}`}>
        <th scope="row">{modelName(group.model)}</th>
        <td>{formatTokens(group.input)}</td><td>{formatTokens(group.output)}</td><td>{formatTokens(group.total)}</td>
      </tr>)}</tbody>
    </table></div>
    <p className={styles.note}>{t.note}</p>
  </details>;
}

/** Secondary, collapsed backend history: the timestamped snapshots recorded by the server. */
function ServerHistory({ samples, note }: { samples: TokenSample[]; note: string | null }) {
  const t = chatText.tokenDashboard;
  const models = [...new Set(samples.flatMap(s => s.models.map(r => r.model)))].sort();
  const start = samples[0]?.at ?? 0;
  const span = Math.max(1000, (samples.at(-1)?.at ?? start) - start);
  const max = Math.max(1, ...samples.flatMap(s => s.models.map(r => r.total)));
  const x = (at: number) => 55 + (at - start) / span * 280;
  const y = (total: number) => 170 - total / max * 135;
  return <details className={styles.details}>
    <summary>{t.serverDetails}</summary>
    {samples.length > 0 && <>
      <svg className={styles.chart} viewBox="0 0 355 225" role="img" aria-label={t.timeline}>
        <title>{t.timeline}</title><desc>{t.recording}</desc>
        <path d="M55 35V170H335" fill="none" stroke="currentColor" />
        <text x="5" y="18">{t.tokenAxis}</text>
        <text x="50" y="39" textAnchor="end">{formatTokens(max)}</text>
        <text x="50" y="174" textAnchor="end">0</text>
        <text x="55" y="190">{timeLabel(start)}</text>
        {samples.length > 1 && <text x="335" y="205" textAnchor="end">{timeLabel(samples.at(-1)!.at)}</text>}
        <text x="195" y="222" textAnchor="middle">{t.timeAxis}</text>
        {models.map(model => {
          const { color } = appearance(model, models);
          const points = samples.flatMap(sample => {
            const row = sample.models.find(row => row.model === model);
            return row ? [{ at: sample.at, ...row }] : [];
          });
          const d = points.map((p, i) => i ? `H${x(p.at)}V${y(p.total)}` : `M${x(p.at)} ${y(p.total)}`).join(" ");
          return <g key={model}>
            <path d={d} fill="none" stroke={color} strokeWidth="2" />
            {points.map(p => <circle key={p.at} cx={x(p.at)} cy={y(p.total)} r="3" fill={color}>
              <title>{`${new Date(p.at).toLocaleString("vi-VN")} — ${t.modelRow(modelName(model), formatTokens(p.input), formatTokens(p.output))}; ${t.tokenAxis}: ${formatTokens(p.total)}`}</title>
            </circle>)}
          </g>;
        })}
      </svg>
      <ul className={styles.legend}>{models.map(model => {
        const { color } = appearance(model, models);
        return <li key={model}><i className={styles.swatch} style={{ background: color }} /><span className={styles.legendName} title={modelName(model)}>{modelName(model)}</span></li>;
      })}</ul>
      <div className={styles.tableWrap}><table><caption>{t.timeline}</caption>
        <thead><tr><th>{t.timeAxis}</th><th>{t.modelColumn}</th><th>{t.input}</th><th>{t.output}</th><th>{t.tokenAxis}</th></tr></thead>
        <tbody>{samples.flatMap(s => s.models.map(r => <tr key={`${s.at}:${r.model}`}>
          <td>{new Date(s.at).toLocaleString("vi-VN")}</td><th scope="row">{modelName(r.model)}</th>
          <td>{formatTokens(r.input)}</td><td>{formatTokens(r.output)}</td><td>{formatTokens(r.total)}</td>
        </tr>))}</tbody>
      </table></div>
    </>}
    {note && <p className={styles.note}>{note}</p>}
    <p className={styles.note}>{t.recording}</p>
  </details>;
}

const IN_COLOR = "#2a78d6", OUT_COLOR = "#1baf7a";
type Usage = ReturnType<typeof chatTokenUsage>;
type Replies = ReturnType<typeof chatTokenReplies>;
const pct = (part: number, whole: number) => whole > 0 ? Math.round(part / whole * 100) : 0;

function Tile({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return <div className={styles.tile}><small>{label}</small><b>{value}</b>{hint && <span>{hint}</span>}</div>;
}

/** Per-model ranking bars with compact numbers and share of the total. */
function ModelBars({ usage, order }: { usage: Usage; order: string[] }) {
  return <ul className={styles.rank}>{usage.models.map(row => <li key={`rank:${row.model}`}>
    <span className={styles.legendName} title={modelName(row.model)}>{modelName(row.model)}</span>
    <span className={styles.track}><i style={{ width: `${pct(row.total, usage.total)}%`, background: appearance(row.model, order).color }} /></span>
    <span className={styles.legendValue}>{formatCompact(row.total)} · {pct(row.total, usage.total)}%</span>
  </li>)}</ul>;
}

/** Option A: one stacked column (input under output) per reply, so heavy replies stand out. */
function ReplyBars({ replies }: { replies: Replies }) {
  const t = chatText.tokenDashboard;
  const max = Math.max(1, ...replies.map(r => r.total));
  const slot = 280 / Math.max(1, replies.length), w = Math.min(24, slot * 0.7);
  const y = (v: number) => 170 - v / max * 135;
  return <svg className={styles.chart} viewBox="0 0 355 205" role="img" aria-label={t.perReplyTitle}>
    <title>{t.perReplyTitle}</title>
    {[0, 1, 2, 3].map(i => <g key={i}>
      <path d={`M55 ${y(max * i / 3)}H335`} stroke="#e1e0d9" />
      <text x="50" y={y(max * i / 3) + 4} textAnchor="end">{formatCompact(max * i / 3)}</text>
    </g>)}
    {replies.map((r, i) => {
      const x = 55 + slot * i + (slot - w) / 2;
      return <g key={r.step}>
        <rect x={x} y={y(r.input)} width={w} height={170 - y(r.input)} fill={IN_COLOR} rx="2" />
        <rect x={x} y={y(r.total)} width={w} height={y(r.input) - y(r.total)} fill={OUT_COLOR} rx="2" />
        {(replies.length <= 12 || i % Math.ceil(replies.length / 12) === 0) && <text x={x + w / 2} y="186" textAnchor="middle">{r.step}</text>}
        <title>{`${t.replyN(r.step)} — ${t.modelRow(modelName(r.model), formatTokens(r.input), formatTokens(r.output))}`}</title>
      </g>;
    })}
    <text x="195" y="202" textAnchor="middle">{t.replyAxis}</text>
  </svg>;
}

function RepliesView({ usage, replies, order }: { usage: Usage; replies: Replies; order: string[] }) {
  const t = chatText.tokenDashboard;
  return <>
    <div className={styles.tiles}>
      <Tile label={t.totalTokens} value={formatCompact(usage.total)} hint={`${usage.reports} ${t.replies.toLowerCase()}`} />
      <Tile label={t.input} value={formatCompact(usage.input)} hint={`${pct(usage.input, usage.total)}%`} />
      <Tile label={t.output} value={formatCompact(usage.output)} hint={`${pct(usage.output, usage.total)}%`} />
    </div>
    <h3 className={styles.modelsTitle}>{t.byModel}</h3>
    <ModelBars usage={usage} order={order} />
    <h3 className={styles.modelsTitle}>{t.perReplyTitle}</h3>
    <ul className={styles.legendRow}>
      <li><i style={{ background: IN_COLOR }} />{t.input}</li><li><i style={{ background: OUT_COLOR }} />{t.output}</li>
    </ul>
    <ReplyBars replies={replies} />
  </>;
}

/** Option B: share donut, model table with sparklines, and the heaviest replies. */
function OverviewView({ usage, series, replies }: { usage: Usage; series: ReturnType<typeof chatTokenSeries>; replies: Replies }) {
  const t = chatText.tokenDashboard;
  const R = 52, C = 2 * Math.PI * R;
  let offset = 0;
  const top = [...replies].sort((a, b) => b.total - a.total).slice(0, 3);
  const topMax = Math.max(1, top[0]?.total ?? 1);
  const spark = (model: string) => {
    const pts = series.series.find(s => s.model === model)?.points ?? [];
    const max = Math.max(1, series.max);
    return pts.map(p => `${p.step / Math.max(1, series.steps) * 90},${22 - p.total / max * 20}`).join(" ");
  };
  return <>
    <div className={styles.donutWrap}>
      <svg viewBox="0 0 140 140" width="140" height="140" role="img" aria-label={t.share}>
        <title>{t.share}</title>
        {usage.models.map(row => {
          const len = usage.total > 0 ? row.total / usage.total * C : 0;
          const el = <circle key={row.model} cx="70" cy="70" r={R} fill="none" stroke={appearance(row.model, series.models).color} strokeWidth="16"
            strokeDasharray={`${len} ${C - len}`} strokeDashoffset={-offset} transform="rotate(-90 70 70)" />;
          offset += len;
          return el;
        })}
        <text x="70" y="72" textAnchor="middle" fontSize="20" fill="currentColor">{formatCompact(usage.total)}</text>
        <text x="70" y="88" textAnchor="middle" fontSize="11" fill="currentColor">{usage.reports} {t.replies.toLowerCase()}</text>
      </svg>
    </div>
    <div className={styles.tableWrap}><table>
      <thead><tr><th>{t.modelColumn}</th><th>{t.trend}</th><th>{t.tokenAxis}</th><th>{t.share}</th></tr></thead>
      <tbody>{usage.models.map(row => {
        const { color } = appearance(row.model, series.models);
        return <tr key={`ov:${row.model}`}>
          <th scope="row"><i className={styles.swatch} style={{ background: color }} />{modelName(row.model)}</th>
          <td><svg width="90" height="24" viewBox="0 0 90 24" aria-hidden="true"><polyline points={spark(row.model)} fill="none" stroke={color} strokeWidth="2" strokeLinejoin="round" /></svg></td>
          <td>{formatCompact(row.total)}</td><td>{pct(row.total, usage.total)}%</td>
        </tr>;
      })}</tbody>
    </table></div>
    <h3 className={styles.modelsTitle}>{t.topReplies}</h3>
    <ul className={styles.rank}>{top.map(r => <li key={r.step}>
      <span className={styles.legendName}>{t.replyN(r.step)}</span>
      <span className={styles.track}><i style={{ width: `${pct(r.total, topMax)}%`, background: IN_COLOR }} /></span>
      <span className={styles.legendValue}>{formatCompact(r.total)}</span>
    </li>)}</ul>
  </>;
}

export function TokenDashboardView({ messages, samples = [], status = "ready", compact = false }: {
  messages: Parameters<typeof chatTokenUsage>[0]; samples?: TokenSample[]; status?: TokenTimelineStatus; compact?: boolean;
}) {
  // Recomputed from the current messages on every update: no accumulation of earlier render state.
  const usage = useMemo(() => chatTokenUsage(messages), [messages]);
  const series = useMemo(() => chatTokenSeries(messages), [messages]);
  const replies = useMemo(() => chatTokenReplies(messages), [messages]);
  const [view, setView] = useState<"replies" | "overview">("replies");
  const t = chatText.tokenDashboard;
  // Compact frame: only the short heading plus the cumulative chart and its legend. Totals, the exact
  // table, the backend history and the explanatory notes live in the large Dashboard dialog.
  if (compact) return <section className={`${styles.dashboard} ${styles.compact}`} aria-label={t.title}>
    <h2 className={styles.title}>{t.title}</h2>
    {usage.reports === 0
      ? <p className={styles.empty}>{t.empty}</p>
      : <CumulativeChart series={series} />}
  </section>;
  // Loading, empty server history and failures stay distinguishable; the note lives inside the
  // collapsed backend section so it never obscures the main chart.
  const historyNote = samples.length ? (status === "error" ? t.historyError : null)
    : status === "loading" ? t.waiting
      : status === "error" ? t.historyError
        : usage.reports > 0 ? t.historyUnavailable : t.historyEmpty;
  return <section className={styles.dashboard} aria-label={t.title}>
    <h2 className={styles.title}>{t.title}</h2>
    {usage.reports === 0 && <p className={styles.empty}>{t.empty}</p>}
    {usage.reports > 0 && <>
      <p className={styles.total}>{t.total(formatTokens(usage.input), formatTokens(usage.output))}</p>
      <div className={styles.seg} role="group" aria-label={t.viewsLabel}>
        {([["replies", t.viewReplies], ["overview", t.viewOverview]] as const).map(([key, label]) =>
          <button key={key} type="button" aria-pressed={view === key} className={view === key ? styles.segOn : undefined} onClick={() => setView(key)}>{label}</button>)}
      </div>
      {view === "replies" ? <RepliesView usage={usage} replies={replies} order={series.models} /> : <OverviewView usage={usage} series={series} replies={replies} />}
      <details className={styles.details}>
        <summary>{t.cumulativeDetails}</summary>
        <section className={styles.models} aria-label={t.cumulativeTitle}>
          <h3 className={styles.modelsTitle}>{t.cumulativeTitle}</h3>
          <CumulativeChart series={series} />
          <p className={styles.note}>{t.cumulativeNote}</p>
        </section>
      </details>
      <ExactTable usage={usage} />
    </>}
    <ServerHistory samples={samples} note={historyNote} />
  </section>;
}

/** Full view: reads the active chat and polls the backend history for it. */
function TokenDashboardFull() {
  const id = useChatStore(state => state.id);
  const messages = useChatStore(state => state.messages);
  const usage = useMemo(() => chatTokenUsage(messages), [messages]);
  const { samples, status } = useTokenTimeline(id, usage);
  return <TokenDashboardView messages={messages} samples={samples} status={status} />;
}

/** Compact frame: current messages only, no history request. */
function TokenDashboardCompact() {
  const messages = useChatStore(state => state.messages);
  return <TokenDashboardView messages={messages} compact />;
}

/**
 * Compact mode renders only the cumulative chart from the current messages, so the history polling
 * hook is never mounted there; the full mode keeps the backend history of the active chat.
 */
export function TokenDashboard({ compact = false }: { compact?: boolean } = {}) {
  if (compact) return <TokenDashboardCompact />;
  return <TokenDashboardFull />;
}
