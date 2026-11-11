import { Sparkles } from "lucide-react";
import { Button } from "@/components/button/button";
import { StatusBadge } from "@/components/status-badge/status-badge";
import { Skeleton } from "@/components/skeleton/skeleton";
import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { plansApi } from "@/apis/plans.api";
import { configApi } from "@/apis/config.api";
import { mutisApi } from "@/apis/mutis.api";
import { TaskSchema, type Plan } from "@/schema/plans.schema";
import { useCardStore } from "@/components/features/chat/card-store";
import { useChatStore } from "@/components/features/chat/chat-store";
import { startCard, cancelCard } from "@/components/features/chat/card-actions";
import { useClock } from "@/components/features/chat/use-clock";
import { notify } from "@/components/toast/toast";
import { createPlanRunner, stopActivePlan } from "./plan-runner";
import { planRunOrder, planSummary } from "./plan-order";
import { PlanTaskRow } from "./plan-task";
import { cardText as t } from "@/text/cards";
import styles from "./plan-card.module.css";
export function PlanCard({ id }: { id: string }) {
  const cached = useCardStore(s => s.plans[id]); const running = useCardStore(s => s.running); const confirm = useCardStore(s => s.confirm === id);
  const logs = useCardStore(s => s.logs[id]); const streaming = useChatStore(s => s.streaming);
  const query = useQuery({ queryKey: ["plan", id], queryFn: ({ signal }) => plansApi.get(id, signal), enabled: !cached });
  const config = useQuery({ queryKey: ["config"], queryFn: () => configApi.get() });
  const mutis = useQuery({ queryKey: ["mutis"], queryFn: () => mutisApi.get() });
  const [localBusy, setLocalBusy] = useState(false); const [dirty, setDirty] = useState(false);
  const [started] = useState(Date.now() / 1000); const plan = cached || query.data;
  const clock = useClock(Boolean(plan?.planning), plan?.updated_at || started);
  const runner = useRef<ReturnType<typeof createPlanRunner> | null>(null);
  useEffect(() => { if (query.data && !useCardStore.getState().plans[id]) useCardStore.getState().plan(query.data); }, [id, query.data]);
  const busy = Boolean(running) || streaming || localBusy;
  let invalid = !plan || !plan.tasks.length || plan.tasks.some(task => !task.title.trim() || !task.detail.trim());
  try { if (plan) planRunOrder(plan.tasks); } catch { invalid = true; }
  useEffect(() => {
    if (!dirty || !plan || busy || invalid) return;
    let active = true;
    const timer = setTimeout(() => {
      void plansApi.save(plan).then(() => { if (active) setDirty(false); }).catch(error => { if (active) notify(String(error)); });
    }, 400);
    return () => { active = false; clearTimeout(timer); };
  }, [plan, dirty, busy, invalid]);
  if (!plan) return query.error ? <p role="alert">{query.error.message}</p> : <Skeleton card />;
  const done = plan.tasks.filter(task => task.status === t.complete).length;
  async function action(fn: () => Promise<unknown>) { setLocalBusy(true); try { await fn(); } catch (error) { notify(error instanceof Error ? error.message : t.error); } finally { setLocalBusy(false); } }
  function updateSummary(value: Plan) {
    useChatStore.setState(state => ({ messages: state.messages.map(message => message.plan_id === value.id ? { ...message, content: planSummary(value) } : message) }));
  }
  function edit(update: (plan: Plan) => void) {
    const next = structuredClone(plan!); update(next); updateSummary(next); useCardStore.getState().plan(next); setDirty(true);
  }
  async function save(value = plan!) { await plansApi.save(value); updateSummary(value); useCardStore.getState().plan(value); setDirty(false); }
  function getRunner() {
    if (!config.data || !mutis.data) throw new Error(t.wait);
    runner.current = createPlanRunner(mutis.data, config.data.providers); return runner.current;
  }
  return <section className={styles.card} aria-label={t.plan}><header><strong><Sparkles size={18} aria-hidden="true" /> {t.plan} · {plan.title}</strong><StatusBadge state={plan.error ? "failed" : plan.planning || running === id ? "running" : done === plan.tasks.length && done ? "done" : "draft"} label={plan.error ? t.error : plan.planning ? t.planning : running === id ? t.running : done === plan.tasks.length && done ? t.done : plan.approved ? t.approved : t.draft} /><time>{plan.planning ? `${clock}s` : `${done}/${plan.tasks.length}`}</time></header>
    <progress value={done} max={Math.max(1, plan.tasks.length)} aria-label={`${done}/${plan.tasks.length} ${t.done}`} />
    {plan.error && <p role="alert">{plan.error}</p>}
    {plan.planning ? <><pre aria-live="polite">{logs?.join("\n")}</pre><button onClick={() => cancelCard(id)}>{t.cancel}</button></> : <>
      <label>{t.title}<input value={plan.title} disabled={busy} onChange={event => edit(p => { p.title = event.target.value; })} /></label>
      <div className={styles.tasks}>{plan.tasks.map((task, index) => <PlanTaskRow key={task.id} task={task} index={index} plan={plan} config={config.data} mutis={mutis.data} busy={busy} edit={edit}
        run={() => void action(async () => { await save(); await getRunner().runPlanTask(task, plan); })} review={() => void action(async () => { await save(); await getRunner().review(task, plan); })} />)}</div>
      {invalid && <p role="alert">{t.invalid}</p>}
      <footer><button disabled={busy} onClick={() => edit(p => { p.tasks.push(TaskSchema.parse({ id: Math.max(0, ...p.tasks.map(t => t.id)) + 1, title: t.newTask, detail: t.newDetail, assignee: config.data?.providers[0]?.id || "", kind: "code" })); })}>{t.addTask}</button>
        <button disabled={busy || invalid || !dirty} onClick={() => void action(() => save())}>{t.save}</button>
        {!plan.approved && <button disabled={busy || invalid} onClick={() => void action(() => save({ ...plan, approved: true }))}>{t.approve}</button>}
        {running === id ? <Button variant="danger" size="sm" onClick={() => void stopActivePlan()}>{t.stop}</Button> : <Button variant="primary" size="sm" disabled={busy || invalid || done === plan.tasks.length} onClick={() => useCardStore.setState({ confirm: id })}>{plan.tasks.some(task => [t.failed, t.halted].includes(task.status)) ? t.resume : t.runAll}</Button>}
        {confirm && <div role="group" aria-label={t.confirm(plan.tasks.length - done)}><p>{t.confirm(plan.tasks.length - done)}</p><button disabled={busy || invalid} onClick={() => void action(async () => { const approved = { ...plan, approved: true }; await save(approved); useCardStore.setState({ confirm: null }); await getRunner().runWholePlan(approved); })}>{t.run}</button><button onClick={() => useCardStore.setState({ confirm: null })}>{t.preview}</button></div>}
        <button disabled={busy || !mutis.data?.enabled || !mutis.data.verify_gate} onClick={() => void action(async () => { const report = await plansApi.verify(plan.folder, new AbortController().signal, notify); notify(report.summary_lines.join(" · ")); })}>{t.verify}</button>
        <button disabled={busy} onClick={() => void action(() => startCard(plan.request, "plan", undefined, false))}>{t.replan}</button>
        <label><input type="checkbox" checked={plan.auto_continue} disabled={busy} onChange={event => edit(p => { p.auto_continue = event.target.checked; })} />{t.autoContinue}</label>
        <small>{plan.tasks.reduce((sum, task) => sum + task.tokens, 0).toLocaleString("vi-VN")} token</small>
      </footer></>}
  </section>;
}
