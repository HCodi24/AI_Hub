// Card summaries are the only plan context sent to later chat turns.
export function planSummary(plan) {
  const lines = [`✦ Chia việc · ${plan.title}`];
  for (const task of plan.tasks || []) lines.push(`- [${task.status}] ${task.title} · ${task.assignee}`);
  if (plan.error) lines.push(`Lỗi: ${plan.error}`);
  return Array.from(lines.join("\n")).slice(0, 1500).join("");
}
