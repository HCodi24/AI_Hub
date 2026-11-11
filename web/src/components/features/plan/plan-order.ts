import type { PlanTask, Plan } from "@/schema/plans.schema";
import { cardText as t } from "@/text/cards";
export function planRunOrder(tasks: Pick<PlanTask, "id" | "status" | "depends_on">[]): number[] {
  const ids = new Set(tasks.map(task => task.id));
  if (ids.size !== tasks.length) throw new Error(t.duplicate);
  for (const task of tasks) for (const id of task.depends_on) if (!ids.has(id)) throw new Error(`${t.missingDep} ${id}`);
  // Validate the entire graph, including completed tasks.
  const seen = new Set<number>();
  const order: number[] = [];
  while (seen.size < tasks.length) {
    const task = tasks.find(item => !seen.has(item.id) && item.depends_on.every(id => seen.has(id)));
    if (!task) throw new Error(t.cycle);
    seen.add(task.id);
    if (task.status !== t.complete) order.push(task.id);
  }
  return order;
}
export function planSummary(plan: Plan): string {
  const lines = [`${t.plan} · ${plan.title}`, ...plan.tasks.map(task => `- [${task.status}] ${task.title} · ${task.assignee}`)];
  if (plan.error) lines.push(`${t.error}: ${plan.error}`);
  return Array.from(lines.join("\n")).slice(0, 1500).join("");
}
export function taskCanWrite(task: PlanTask) { return task.permission ? task.permission !== "read" : ["code", "test", "docs", "small_edit"].includes(task.kind); }
