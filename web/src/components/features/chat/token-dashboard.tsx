import { useMemo } from "react";
import { chatText } from "@/text/chat";
import { chatTokenUsage, formatTokens } from "@/lib/chat-token-usage";
import type { TokenSample } from "@/lib/chat-token-timeline";
import { useChatStore } from "./chat-store";
import { useTokenTimeline, type TokenTimelineStatus } from "./use-token-timeline";
import styles from "./token-dashboard.module.css";

function appearance(model: string) {
  const hash = Array.from(model).reduce((v, c) => (v * 31 + c.charCodeAt(0)) >>> 0, 0);
  return { color: ["#1d4ed8", "#92400e", "#047857", "#7e22ce", "#be123c"][hash % 5],
    dash: ["", "5 3", "2 2", "8 3 2 3"][Math.floor(hash / 5) % 4] };
}
const timeLabel = (at: number) => new Date(at).toLocaleTimeString("vi-VN", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false });

export function TokenDashboardView({ messages, samples = [], status = "ready" }: {
  messages: Parameters<typeof chatTokenUsage>[0]; samples?: TokenSample[]; status?: TokenTimelineStatus;
}) {
  const usage = useMemo(() => chatTokenUsage(messages), [messages]);
  const t = chatText.tokenDashboard;
  const models = [...new Set(samples.flatMap(s => s.models.map(r => r.model)))].sort();
  const start = samples[0]?.at ?? 0;
  const span = Math.max(1000, (samples.at(-1)?.at ?? start) - start);
  const max = Math.max(1, ...samples.flatMap(s => s.models.map(r => r.total)));
  const x = (at: number) => 55 + (at - start) / span * 280;
  const y = (total: number) => 170 - total / max * 135;
  // Loading, empty server history and failures stay distinguishable even without local reports;
  // a failure keeps the last samples, so the chart stays visible next to the error note.
  const historyNote = samples.length ? (status === "error" ? t.historyError : null)
    : status === "loading" ? t.waiting : status === "error" ? t.historyError : t.historyEmpty;
  return <section className={styles.dashboard} aria-label={t.title}>
    <h2 className={styles.title}>{t.title}</h2>
    {usage.reports === 0 && <p className={styles.empty}>{t.empty}</p>}
    {usage.reports > 0 && <p className={styles.total}>{t.total(formatTokens(usage.input), formatTokens(usage.output))}</p>}
    {!samples.length ? <p className={styles.empty}>{historyNote}</p> : <>
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
          const { color, dash } = appearance(model);
          const points = samples.flatMap(sample => {
            const row = sample.models.find(row => row.model === model);
            return row ? [{ at: sample.at, ...row }] : [];
          });
          const d = points.map((p, i) => i ? `H${x(p.at)}V${y(p.total)}` : `M${x(p.at)} ${y(p.total)}`).join(" ");
          return <g key={model}>
            <path d={d} fill="none" stroke={color} strokeWidth="2" strokeDasharray={dash} />
            {points.map(p => <circle key={p.at} cx={x(p.at)} cy={y(p.total)} r="3" fill={color}>
              <title>{`${new Date(p.at).toLocaleString("vi-VN")} — ${t.modelRow(model || t.unknownModel, formatTokens(p.input), formatTokens(p.output))}; ${t.tokenAxis}: ${formatTokens(p.total)}`}</title>
            </circle>)}
          </g>;
        })}
      </svg>
      <ul className={styles.legend}>{models.map(model => {
        const { color, dash } = appearance(model);
        return <li key={model}><svg width="24" height="12" aria-hidden="true"><path d="M0 6H24" stroke={color} strokeWidth="2" strokeDasharray={dash} /></svg><span>{model || t.unknownModel}</span></li>;
      })}</ul>
      <details className={styles.details}><summary>{t.details}</summary>
        <div className={styles.tableWrap}><table><caption>{t.timeline}</caption>
          <thead><tr><th>{t.timeAxis}</th><th>Model</th><th>{t.input}</th><th>{t.output}</th><th>{t.tokenAxis}</th></tr></thead>
          <tbody>{samples.flatMap(s => s.models.map(r => <tr key={`${s.at}:${r.model}`}>
            <td>{new Date(s.at).toLocaleString("vi-VN")}</td><th scope="row">{r.model || t.unknownModel}</th>
            <td>{formatTokens(r.input)}</td><td>{formatTokens(r.output)}</td><td>{formatTokens(r.total)}</td>
          </tr>))}</tbody>
        </table></div><p className={styles.note}>{t.note}</p>
      </details>
      {historyNote && <p className={styles.note}>{historyNote}</p>}
    </>}
    <p className={styles.note}>{t.recording}</p>
  </section>;
}

export function TokenDashboard() {
  const id = useChatStore(state => state.id);
  const messages = useChatStore(state => state.messages);
  const usage = useMemo(() => chatTokenUsage(messages), [messages]);
  const { samples, status } = useTokenTimeline(id, usage);
  return <TokenDashboardView messages={messages} samples={samples} status={status} />;
}
