import { ReviewSchema, type Plan, type PlanTask, type Review } from "@/schema/plans.schema";
import type { RunnerContext } from "./plan-runner";
import { cardText as t } from "@/text/cards";
import { rolesFor } from "@/lib/mutis-level";
export function nextReviewAction(review: Review, used: number, max: number) {
  if (review.verdict === "SKIPPED") return "skip";
  if (review.verdict !== "FAIL") return "complete";
  return review.issues.some(i => ["high", "medium"].includes(i.severity)) && used < max ? "create_fix" : "fail";
}
export function extractReview(text: string): Review {
  for (let start = text.indexOf("{"); start >= 0; start = text.indexOf("{", start + 1)) {
    let depth = 0, quoted = false, escape = false;
    for (let i = start; i < text.length; i++) {
      const c = text[i];
      if (quoted) { if (escape) escape = false; else if (c === "\\") escape = true; else if (c === '"') quoted = false; continue; }
      if (c === '"') quoted = true;
      else if (c === "{") depth++;
      else if (c === "}" && --depth === 0) {
        try { return ReviewSchema.parse(JSON.parse(text.slice(start, i + 1))); } catch { break; }
      }
    }
  }
  return ReviewSchema.parse({ verdict: "FAIL", issues: [{ severity: "high", problem: t.invalidReview }], notes: text.slice(-1000) });
}
export function buildFixTask(plan: Plan, task: PlanTask, label: string, issues: string): PlanTask {
  const title = task.fix_root_title || task.title, detail = task.fix_root_detail ?? task.detail;
  return { ...structuredClone(task), id: Math.max(0, ...plan.tasks.map(t => t.id)) + 1,
    title: `${label}: ${title} (vòng ${task.fix_rounds_used})`, detail: `Nhiệm vụ gốc: ${title}\n${detail}\n\n${label}:\n${issues}`,
    fix_root_title: title, fix_root_detail: detail, depends_on: [task.id], status: t.pending, summary: "", diff: "", log: [], tokens: 0, chat_id: null, verify: null, review: null };
}
export async function runTaskGates(ctx: RunnerContext, task: PlanTask, plan: Plan): Promise<void> {
  const cfg = ctx.config;
  if (!cfg.enabled || ctx.signal.aborted) return;
  if (cfg.verify_gate) {
    task.verify = await ctx.services.verify(plan.folder, ctx.signal, text => { task.log.push(text); ctx.progress(plan); });
    await ctx.record(plan, task, "verify", task.verify.ok, task.verify.summary_lines.join(" | "));
    if (!task.verify.ok) { await fix(t.fixVerify, ReviewSchema.parse({ verdict: "FAIL", issues: task.verify.summary_lines.map(problem => ({ severity: "high", problem })) })); return; }
  }
  if (cfg.auto_review && ["code", "test", "docs", "small_edit"].includes(task.kind)) {
    const resolved = await ctx.services.resolve();
    const role = rolesFor(cfg, plan.level)?.lead || cfg.roles.lead;
    const reviewer = resolved.lead.used_provider;
    const reviewerId = plan.level === "high" ? role.provider_id : reviewer?.id;
    if (!reviewerId) task.review = { verdict: "SKIPPED", issues: [], notes: t.noReview };
    else {
      const diff = task.diff || (await ctx.services.diff(plan.folder)).diff;
      let answer = "";
      ctx.chatId = crypto.randomUUID();
      await ctx.services.chat({ chat_id: ctx.chatId, plan_id: plan.id, task_id: String(task.id), task_kind: task.kind,
        provider_id: reviewerId, model: plan.level === "high" ? role.model : reviewerId === role.provider_id ? role.model || reviewer?.models[0] || "" : reviewer?.models[0] || "",
        effort: role.effort, folder: plan.folder, permission: "read", file_tools: "none", dev_chat: plan.dev_chat,
        messages: [{ role: "user", content: `Review task và trả CHỈ JSON {"verdict":"PASS|FAIL","issues":[{"severity":"high|medium|low","file":"","line":0,"problem":"","fix":""}],"notes":""}. Task: ${task.title}\nChi tiết: ${task.detail}\nTóm tắt: ${task.summary}\nVerify: ${(task.verify?.summary_lines || []).join("\n")}\nDiff:\n${diff.slice(0, 30000)}` }] }, ctx.signal, (event, data) => { if (event === "delta") answer += data.text || ""; if (event === "error") throw new Error(data.text || t.error); });
      task.review = extractReview(answer);
    }
    if (ctx.signal.aborted) return;
    await ctx.record(plan, task, "review", task.review.verdict !== "FAIL", JSON.stringify(task.review));
    if (task.review.verdict === "FAIL") await fix(t.fixReview, task.review);
  }
  await ctx.save(plan);
  async function fix(label: string, review: Review) {
    if (ctx.signal.aborted) return;
    if (cfg.auto_fix && nextReviewAction(review, task.fix_rounds_used, cfg.fix_rounds_max) === "create_fix") {
      task.status = t.fixing; task.fix_rounds_used++;
      const child = buildFixTask(plan, task, label, review.issues.map(i => `${i.severity} ${i.file}:${i.line} ${i.problem} — ${i.fix}`).join("\n"));
      plan.tasks.push(child); await ctx.save(plan); await ctx.runTask(child, plan);
      await ctx.record(plan, child, "fix_task", child.status === t.complete, label);
      task.status = child.status === t.complete ? t.complete : t.failed;
    } else task.status = t.failed;
    await ctx.save(plan);
  }
}
