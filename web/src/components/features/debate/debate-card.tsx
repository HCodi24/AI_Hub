import { Scale, CheckCircle2, Circle, Loader2 } from "lucide-react";
import { StatusBadge } from "@/components/status-badge/status-badge";
import { Skeleton } from "@/components/skeleton/skeleton";
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { debatesApi } from "@/apis/debates.api";
import { plansApi } from "@/apis/plans.api";
import { useCardStore } from "@/components/features/chat/card-store";
import { cancelCard } from "@/components/features/chat/card-actions";
import { useClock } from "@/components/features/chat/use-clock";
import { MarkdownView } from "@/components/markdown-view/markdown-view";
import { DebateActions } from "./debate-actions";
import { cardText as t } from "@/text/cards";
import styles from "./debate-card.module.css";
export function DebateCard({ id }: { id: string }) {
  const cached = useCardStore(s => s.debates[id]);
  const query = useQuery({ queryKey: ["debate", id], queryFn: ({ signal }) => debatesApi.get(id, signal), enabled: !cached });
  const record = cached?.record || query.data;
  const linked = useCardStore(s => record?.plan_id ? s.plans[record.plan_id] : undefined);
  const planQuery = useQuery({ queryKey: ["plan", record?.plan_id], queryFn: ({ signal }) => plansApi.get(record!.plan_id!, signal), enabled: Boolean(record?.plan_id && !linked) });
  const plan = linked || planQuery.data;
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const [selected, setSelected] = useState<number | null>(null); const [side, setSide] = useState("codex");
  const clock = useClock(Boolean(cached?.live), record?.created_at || 0, record?.elapsed_s);
  if (!record) return query.error ? <p role="alert">{query.error.message}</p> : <Skeleton card />;
  const round = record.rounds.find(r => r.index === (selected || cached?.round)) || record.rounds.at(-1);
  const verdict = record.verdict;
  const steps = [...Array.from({ length: record.max_rounds }, (_, index) => ({ label: `${t.round} ${index + 1} ${index ? t.rebuttal : t.proposal}`, done: cached?.live ? Boolean(cached.finished?.includes(`codex:${index + 1}`)) : Boolean(record.rounds.find(r => r.index === index + 1)?.codex.text), active: cached?.live && cached.phase === "round" && cached.round === index + 1 })),
    { label: t.judge, done: Boolean(verdict), active: cached?.live && cached.phase === "judge" }, { label: t.plan, done: Boolean(plan && !plan.planning), active: plan?.planning },
    { label: t.execute, done: Boolean(plan?.tasks.length && plan.tasks.every(task => task.status === t.complete)), active: plan?.tasks.some(task => task.status === t.active) }];
  return <section className={styles.card} aria-label={t.debate}><header><strong><Scale size={18} aria-hidden="true" /> {t.debate} · {record.title || record.request.slice(0, 72)}</strong><StatusBadge state={cached?.live ? "running" : record.error ? "failed" : record.status === "done" ? "done" : "draft"} label={cached?.live ? t.running : record.error ? t.error : record.status === "done" ? t.done : t.stopped} /><time>{clock}s</time></header>
    <p className={styles.meta}>{t.codex} {record.codex_model} · {t.claude} {record.claude_model} · {t.judge}: {record.judge_model}</p>
    <div className={styles.chips}>{steps.map(step => <span key={step.label} data-active={step.active}>{step.done ? <CheckCircle2 size={14} aria-hidden="true" /> : step.active ? <Loader2 size={14} aria-hidden="true" className="spin" /> : <Circle size={14} aria-hidden="true" />} {step.label}</span>)}</div>
    {verdict && <section className={styles.verdict}><h3>{t.verdict} · {t.chosen[verdict.chosen as keyof typeof t.chosen] || t.chosen.merged}</h3><MarkdownView source={verdict.decision} />
      <div className={styles.chips}>{(["agreed_points", "disagreements", "risks", "plan_steps"] as const).map(key => <details key={key}><summary>{t[key]} ({verdict[key].length})</summary><ul>{verdict[key].map((text, i) => <li key={i}>{text}</li>)}</ul></details>)}</div>
      {record.status === "done" && <DebateActions debate={record} />}</section>}
    <div role="tablist" aria-label={t.round} className={styles.chips}>{record.rounds.map(r => <button key={r.index} role="tab" aria-selected={round?.index === r.index} tabIndex={round?.index === r.index ? 0 : -1} onKeyDown={event => {
      if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
      event.preventDefault(); const index = record.rounds.findIndex(item => item.index === r.index);
      const next = event.key === "Home" ? 0 : event.key === "End" ? record.rounds.length - 1 : (index + (event.key === "ArrowRight" ? 1 : -1) + record.rounds.length) % record.rounds.length;
      setSelected(record.rounds[next].index); event.currentTarget.parentElement?.querySelectorAll("button")[next]?.focus();
    }} aria-controls={`round-${id}`} onClick={() => setSelected(r.index)}>{t.round} {r.index}</button>)}</div>
    <div className={styles.toggle}>{["codex", "claude"].map(name => <button key={name} aria-pressed={side === name} onClick={() => setSide(name)}>{name === "codex" ? t.codex : t.claude}</button>)}</div>
    {round && <div className={styles.columns} id={`round-${id}`} role="tabpanel">{(["codex", "claude"] as const).map(name => {
      const turn = round[name]; const key = `${round.index}:${name}`;
      return <section key={name} className={styles.side} data-selected={side === name}><strong>{name === "codex" ? t.codex : t.claude}</strong>
        {typeof turn.agree === "boolean" && <span className={styles.badge}>{turn.agree ? t.agree : t.disagree}</span>}
        <div className={styles.preview} data-expanded={expanded[key]} id={`turn-${id}-${key}`}><MarkdownView source={turn.text || (cached?.live ? t.wait : t.empty)} /></div>
        <button aria-expanded={Boolean(expanded[key])} aria-controls={`turn-${id}-${key}`} onClick={() => setExpanded(previous => ({ ...previous, [key]: !previous[key] }))}>{expanded[key] ? t.collapse : t.expand}</button>
        {turn.disagreements.length > 0 && <p>{turn.disagreements.join("; ")}</p>}
        <small>{turn.tokens.toLocaleString("vi-VN")} token · {turn.elapsed_s}s</small>
        {turn.tokens > 500000 && <p role="status">{t.warning}</p>}{turn.error && <p role="alert">{turn.error}</p>}</section>;
    })}</div>}
    <p aria-live="polite" className={styles.meta}>{cached?.live ? cached.activity || t.wait : record.status === "stopped" ? t.stopped : ""}</p>
    {record.error && <p role="alert">{record.error}</p>}{cached?.live && <button onClick={() => cancelCard(id)}>{t.stop}</button>}
  </section>;
}
