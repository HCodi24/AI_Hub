import { z } from "zod";
import { plansApi } from "@/apis/plans.api";
import { mutisApi } from "@/apis/mutis.api";
import { chatApi } from "@/apis/chat.api";
import { postSse } from "@/lib/sse";
import { StreamDataSchema } from "@/schema/chat.schema";
import { PlanChatRequestSchema, type Plan, type PlanTask } from "@/schema/plans.schema";
import type { Mutis } from "@/schema/mutis.schema";
import type { ProviderSummary } from "@/schema/config.schema";
import { hideMemory } from "@/lib/markdown";
import { useCardStore } from "@/components/features/chat/card-store";
import { useChatStore } from "@/components/features/chat/chat-store";
import { planRunOrder, planSummary, taskCanWrite } from "./plan-order";
import { runTaskGates } from "./plan-gates";
import { taskPrompt } from "@/text/plan";
import { cardText as t } from "@/text/cards";
export const runnerServices = {
  save: plansApi.save, verify: plansApi.verify, diff: plansApi.diff, resolve: mutisApi.resolve, event: mutisApi.event, stop: chatApi.stop,
  chat: (body: unknown, signal: AbortSignal, event: (name: string, data: z.infer<typeof StreamDataSchema>) => void) => postSse("/chat", PlanChatRequestSchema.parse(body), signal, (name, raw) => {
    if (name === "memory") useToastMemory(raw, body);
    event(name, StreamDataSchema.parse(raw));
  }),
};
// Keep toast UI out of the testable runner dependencies.
let memoryHandler: (raw: unknown, folder: string) => void = () => {};
export function setRunnerMemoryHandler(handler: typeof memoryHandler) { memoryHandler = handler; }
function useToastMemory(raw: unknown, body: unknown) {
  const request = z.object({ folder: z.string() }).parse(body);
  memoryHandler(raw, request.folder);
}
export type RunnerContext = {
  config: Mutis; signal: AbortSignal; chatId: string; services: typeof runnerServices;
  progress: (plan: Plan) => void; save: (plan: Plan) => Promise<void>; runTask: (task: PlanTask, plan: Plan) => Promise<void>;
  record: (plan: Plan, task: PlanTask, kind: "verify" | "review" | "fix_task", ok: boolean, detail: string) => Promise<void>;
};
let activeStop: (() => Promise<void>) | null = null;
export async function stopActivePlan() { await activeStop?.(); }
export function createPlanRunner(config: Mutis, providers: ProviderSummary[], services = runnerServices, progress = (plan: Plan) => useCardStore.getState().plan(plan)) {
  let controller = new AbortController();
  let busy = false;
  let activeTask: PlanTask | undefined;
  const ctx: RunnerContext = { config, signal: controller.signal, chatId: "", services, progress,
    async save(plan) { await services.save(plan); progress(plan);
      const state = useChatStore.getState();
      useChatStore.setState({ messages: state.messages.map(message => message.plan_id === plan.id ? { ...message, content: planSummary(plan) } : message) });
    },
    async record(plan, task, kind, ok, detail) { await services.event({ plan_id: plan.id, task_id: String(task.id), task_kind: task.kind, provider_id: task.assignee, kind, ok, detail: detail.slice(0, 4000) }); },
    runTask: async (task, plan) => {
      if (ctx.signal.aborted) return;
      activeTask = task;
      if (!plan.approved) throw new Error(t.notApproved);
      const provider = providers.find(p => p.id === task.assignee) || (task.assignee === "deepseek-agent-deploy" ? providers.find(p => p.agent_capable) : undefined);
      if (!provider) throw new Error(t.missingSource);
      if (config.enabled) await services.resolve();
      task.summary = ""; task.diff = ""; task.log = []; task.status = t.active;
      task.chat_id ||= crypto.randomUUID(); ctx.chatId = task.chat_id;
      await ctx.save(plan);
      let failed = false;
      try {
        await services.chat({ chat_id: task.chat_id, plan_id: plan.id, task_id: String(task.id), task_kind: task.kind,
          dev_chat: plan.dev_chat, provider_id: provider.id, model: task.model || provider.models[0] || "", effort: task.effort,
          folder: plan.folder, permission: task.permission || (taskCanWrite(task) ? "write" : "read"),
          file_tools: provider.agent_capable ? (taskCanWrite(task) ? "write" : "read") : "none",
          messages: [{ role: "user", content: taskPrompt(plan, task, config) }] }, ctx.signal, (event, data) => {
          if (ctx.signal.aborted) return;
          if (event === "error") { failed = true; task.log.push(data.text || t.error); }
          if (event === "delta") task.summary += data.text || "";
          if (event === "diff") task.diff += data.text || "";
          if (event === "log") { task.log.push(data.text || ""); const usage = /^Token: vào (\d+).*?ra (\d+)/u.exec(data.text || ""); if (usage) task.tokens += Number(usage[1]) + Number(usage[2]); }
          progress(plan);
        });
        // A coding task that changed no file did not do its job, whatever the AI said.
        if (!failed && !ctx.signal.aborted && ["code", "small_edit"].includes(task.kind) && !task.diff.trim()) { failed = true; task.log.push(t.noChange); }
        task.status = ctx.signal.aborted ? t.halted : failed ? t.failed : t.complete;
      } catch (error) { failed = true; task.status = ctx.signal.aborted ? t.halted : t.failed; task.log.push(error instanceof Error ? error.message : t.error); }
      task.summary = hideMemory(task.summary); await ctx.save(plan);
      if (!failed && !ctx.signal.aborted) await runTaskGates(ctx, task, plan);
      if (ctx.signal.aborted) { task.status = t.halted; await ctx.save(plan); }
    },
  };
  async function run(plan: Plan, task?: PlanTask, gatesOnly = false) {
    if (busy || useCardStore.getState().running || useChatStore.getState().streaming) throw new Error(t.busy);
    if (!plan.approved) throw new Error(t.notApproved);
    planRunOrder(plan.tasks); busy = true; controller = new AbortController(); ctx.signal = controller.signal;
    useCardStore.setState({ running: plan.id });
    activeStop = async () => { controller.abort(); if (ctx.chatId) await services.stop(ctx.chatId); };
    try {
      if (task) {
        if (!gatesOnly && !task.depends_on.every(id => plan.tasks.find(t => t.id === id)?.status === t.complete)) throw new Error(t.missingDep);
        if (gatesOnly) { await runTaskGates(ctx, task, plan); return; }
        await ctx.runTask(task, plan);
        if (!plan.auto_continue || task.status !== t.complete) return;
      }
      while (!ctx.signal.aborted) {
        const id = planRunOrder(plan.tasks)[0];
        if (id === undefined) break;
        const next = plan.tasks.find(t => t.id === id)!;
        await ctx.runTask(next, plan);
        if (next.status !== t.complete) break;
      }
    } catch (error) {
      const current = plan.tasks.find(task => task.status === t.active || task.status === t.fixing) || task || activeTask;
      if (current) { current.status = ctx.signal.aborted ? t.halted : t.failed; current.log.push(error instanceof Error ? error.message : t.error); await ctx.save(plan); }
      if (!ctx.signal.aborted) throw error;
    } finally { busy = false; activeStop = null; useCardStore.setState({ running: null }); progress(plan); }
  }
  return { runWholePlan: (plan: Plan) => run(plan), runPlanTask: (task: PlanTask, plan: Plan) => run(plan, task), review: (task: PlanTask, plan: Plan) => run(plan, task, true),
    async stop() { controller.abort(); if (ctx.chatId) await services.stop(ctx.chatId); } };
}
