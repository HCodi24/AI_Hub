import { ChevronRight, CheckCircle2, XCircle, Circle, Loader2 } from "lucide-react";
import type { Plan, PlanTask } from "@/schema/plans.schema";
import type { Config } from "@/schema/config.schema";
import type { Mutis } from "@/schema/mutis.schema";
import { taskCanWrite } from "./plan-order";
import { SourceControls } from "./source-controls";
import { MarkdownView } from "@/components/markdown-view/markdown-view";
import { cardText as t } from "@/text/cards";
import { rolesFor } from "@/lib/mutis-level";
export function PlanTaskRow({ task, index, plan, config, mutis, busy, edit, run, review }: { task: PlanTask; index: number; plan: Plan; config?: Config; mutis?: Mutis; busy: boolean;
  edit: (update: (plan: Plan) => void) => void; run: () => void; review: () => void }) {
  const patch = (value: Partial<PlanTask>) => edit(p => { Object.assign(p.tasks.find(item => item.id === task.id)!, value); });
  const ready = task.depends_on.every(id => plan.tasks.find(t => t.id === id)?.status === t.complete);
  const provider = config?.providers.find(p => p.id === task.assignee);
  const Icon = task.status === t.complete ? CheckCircle2 : task.status === t.active ? Loader2 : [t.failed, t.fixing].includes(task.status) ? XCircle : Circle;
  return <details><summary><ChevronRight size={16} aria-hidden="true" /><Icon size={16} aria-hidden="true" className={task.status === t.active ? "spin" : undefined} /><span>#{task.id} {task.title}</span> <small>{task.status}</small> <small>{provider?.name || task.assignee}</small> <small>{task.kind}</small> {task.depends_on.map(id => <small key={id}>↳ #{id} </small>)}</summary>
    <fieldset disabled={busy}><label>{t.taskTitle}<input value={task.title} onChange={event => patch({ title: event.target.value })} /></label>
      <label>{t.detail}<textarea value={task.detail} rows={4} onChange={event => patch({ detail: event.target.value })} /></label>
      <SourceControls providers={config?.providers || []} roles={rolesFor(mutis, plan.level)} value={{ ...task, permission: task.permission || (taskCanWrite(task) ? "write" : "read") }} change={patch} disabled={busy} />
      <label>{t.deps}<input value={task.depends_on.join(",")} onChange={event => patch({ depends_on: event.target.value.split(",").map(s => s.trim()).filter(Boolean).map(Number) })} /></label>
      <label>{t.kind}<select value={task.kind} onChange={event => patch({ kind: event.target.value as PlanTask["kind"] })}>{["plan", "small_edit", "code", "test", "docs", "review"].map(kind => <option key={kind}>{kind}</option>)}</select></label>
      <button disabled={!plan.approved || !ready} onClick={run}>{t.runTask}</button>
      <button disabled={!plan.approved} onClick={() => patch({ status: t.complete })}>{t.markDone}</button><button disabled={!plan.approved} onClick={review}>{t.review}</button>
      <button disabled={!index} onClick={() => edit(p => { [p.tasks[index - 1], p.tasks[index]] = [p.tasks[index], p.tasks[index - 1]]; })}>{t.up}</button>
      <button disabled={index === plan.tasks.length - 1} onClick={() => edit(p => { [p.tasks[index + 1], p.tasks[index]] = [p.tasks[index], p.tasks[index + 1]]; })}>{t.down}</button>
      <button onClick={() => edit(p => { p.tasks = p.tasks.filter(item => item.id !== task.id); })}>{t.deleteTask}</button>
    </fieldset>
    {task.summary && <details><summary>{t.output}</summary><MarkdownView source={task.summary} /></details>}
    {([ [t.logs, task.log.join("\n")], [t.diff, task.diff], [t.verifyResult, task.verify && JSON.stringify(task.verify, null, 2)], [t.reviewResult, task.review && JSON.stringify(task.review, null, 2)] ]).map(([label, text]) => text && <details key={String(label)}><summary>{label}</summary><pre>{text}</pre></details>)}
  </details>;
}
