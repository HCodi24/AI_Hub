import { debatesApi } from "@/apis/debates.api";
import { plansApi } from "@/apis/plans.api";
import { rolesFor } from "@/lib/mutis-level";
import { mutisApi } from "@/apis/mutis.api";
import { configApi } from "@/apis/config.api";
import { ChatMessageSchema, StreamDataSchema } from "@/schema/chat.schema";
import { DebateSchema } from "@/schema/debates.schema";
import { PlanSchema } from "@/schema/plans.schema";
import { debateReducer, type DebateLive } from "@/components/features/debate/debate-reducer";
import { useChatStore } from "./chat-store";
import { useCardStore } from "./card-store";
import { planSummary } from "@/components/features/plan/plan-order";
import { createPlanRunner } from "@/components/features/plan/plan-runner";
import type { Plan } from "@/schema/plans.schema";
import type { Mutis } from "@/schema/mutis.schema";
import type { ProviderSummary } from "@/schema/config.schema";
import { cardText as t } from "@/text/cards";
const controllers = new Map<string, AbortController>();
export function cancelActiveCard() { if (!controllers.size) return false; for (const controller of new Set(controllers.values())) controller.abort(); return true; }
export function cancelCard(id: string) { controllers.get(id)?.abort(); }
export async function startCard(request: string, kind: "debate" | "plan", afterDebateId?: string, appendRequest = !afterDebateId) {
  const state = useChatStore.getState();
  if (state.streaming || useCardStore.getState().running) return;
  if (!request.trim() || !state.settings.folder.trim()) throw new Error(t.request);
  const chatId = state.id || crypto.randomUUID();
  const temp = crypto.randomUUID(); const controller = new AbortController();
  controllers.set(temp, controller);
  const assistant = ChatMessageSchema.parse({ role: "assistant", content: "", [kind === "plan" ? "plan_id" : "debate_id"]: temp });
  const messages = [...state.messages];
  if (afterDebateId) {
    const index = messages.findIndex(m => m.debate_id === afterDebateId); messages.splice(index < 0 ? messages.length : index + 1, 0, assistant);
  } else { if (appendRequest) messages.push(ChatMessageSchema.parse({ role: "user", content: request })); messages.push(assistant); }
  useChatStore.setState({ id: chatId, streaming: true, messages, error: "" });
  let live: DebateLive = { record: DebateSchema.parse({ id: temp, request, folder: state.settings.folder, created_at: Date.now() / 1000 }), activity: "", phase: "round", round: 1, live: true };
  let plan = PlanSchema.parse({ id: temp, title: request.slice(0, 72), request, folder: state.settings.folder, planning: true });
  if (kind === "debate") useCardStore.getState().debate(temp, live); else useCardStore.getState().plan(plan);
  let currentId = temp;
  let autoRun: { plan: Plan; mutis: Mutis; providers: ProviderSummary[] } | undefined;
  const attach = (id: string) => {
    controllers.set(id, controller); currentId = id;
    const now = useChatStore.getState();
    useChatStore.setState({ messages: now.messages.map(m => m === assistant || m.plan_id === temp || m.debate_id === temp ? { ...m, [kind === "plan" ? "plan_id" : "debate_id"]: id } : m) });
  };
  try {
    if (kind === "debate") await debatesApi.start({ request, folder: state.settings.folder, chat_id: chatId }, controller.signal, (event, raw) => {
      live = debateReducer(live, event, raw); attach(live.record.id); useCardStore.getState().debate(live.record.id, live);
      if (event === "done") {
        const content = Array.from(live.record.verdict ? `${t.verdict}\n\n${live.record.verdict.decision}\n${live.record.verdict.plan_steps.map((step, i) => `${i + 1}. ${step}`).join("\n")}` : live.record.error || t.stopped).slice(0, 1500).join("");
        useChatStore.setState({ messages: useChatStore.getState().messages.map(m => m.debate_id === live.record.id ? { ...m, content } : m) });
      }
    });
    else {
      const [cfg, mutis] = await Promise.all([configApi.get(), mutisApi.get()]);
      if (mutis.enabled) await mutisApi.resolve(state.settings.effort);
      const activeRoles = rolesFor(mutis, state.settings.effort);
      await plansApi.generate({ request, folder: state.settings.folder, chat_id: chatId, append_request: appendRequest,
        after_debate_id: afterDebateId, dev_chat: state.settings.dev_chat,
        provider_id: mutis.enabled ? activeRoles?.lead.provider_id || "" : cfg.planner_provider_id,
        model: mutis.enabled ? activeRoles?.lead.model || "" : cfg.planner_model, effort: state.settings.effort }, controller.signal, (event, raw) => {
        if (event === "error") throw new Error(StreamDataSchema.parse(raw).message || StreamDataSchema.parse(raw).text || t.error);
        if (event === "log") useCardStore.getState().log(currentId, StreamDataSchema.parse(raw).text || "");
        if (event === "planning" || event === "plan") { plan = PlanSchema.parse(raw); attach(plan.id); useCardStore.getState().plan(plan); }
      });
      if (afterDebateId) {
        const linked = await debatesApi.link(afterDebateId, plan.id);
        const previous = useCardStore.getState().debates[afterDebateId];
        if (previous) useCardStore.getState().debate(afterDebateId, { ...previous, record: { ...previous.record, ...linked } });
        useCardStore.setState({ confirm: plan.id });
      }
      // Approval off in settings: approve now and run it once streaming ends (see below).
      if (!mutis.plan_approval && !afterDebateId && !plan.error && plan.tasks.length) {
        plan = { ...plan, approved: true }; await plansApi.save(plan); useCardStore.getState().plan(plan);
        autoRun = { plan, mutis, providers: cfg.providers };
      }
      useChatStore.setState({ messages: useChatStore.getState().messages.map(m => m.plan_id === plan.id ? { ...m, content: planSummary(plan) } : m) });
    }
  } catch (error) {
    const message = controller.signal.aborted ? t.stopped : error instanceof Error ? error.message : t.error;
    if (kind === "debate") useCardStore.getState().debate(currentId, { ...live, live: false, record: { ...live.record, error: message, status: controller.signal.aborted ? "stopped" : "error" } });
    else useCardStore.getState().plan({ ...plan, planning: false, error: message });
    useChatStore.setState({ error: message });
  } finally { controllers.delete(temp); controllers.delete(currentId); useChatStore.setState({ streaming: false }); }
  if (autoRun) {
    try { await createPlanRunner(autoRun.mutis, autoRun.providers).runWholePlan(autoRun.plan); }
    catch (error) { useChatStore.setState({ error: error instanceof Error ? error.message : t.error }); }
  }
}
