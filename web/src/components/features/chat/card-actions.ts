import { debatesApi } from "@/apis/debates.api";
import { ChatMessageSchema } from "@/schema/chat.schema";
import { DebateSchema } from "@/schema/debates.schema";
import { debateReducer, type DebateLive } from "@/components/features/debate/debate-reducer";
import { useChatStore } from "./chat-store";
import { useCardStore } from "./card-store";
import { cardText as t } from "@/text/cards";
const controllers = new Map<string, AbortController>();
export function cancelActiveCard() { if (!controllers.size) return false; for (const controller of new Set(controllers.values())) controller.abort(); return true; }
export function cancelCard(id: string) { controllers.get(id)?.abort(); }
/** Starts a Codex ↔ Claude debate card in the current chat. */
export async function startDebate(request: string) {
  const state = useChatStore.getState();
  if (state.streaming || useCardStore.getState().running) return;
  if (!request.trim() || !state.settings.folder.trim()) throw new Error(t.request);
  const chatId = state.id || crypto.randomUUID();
  const temp = crypto.randomUUID(); const controller = new AbortController();
  controllers.set(temp, controller);
  const assistant = ChatMessageSchema.parse({ role: "assistant", content: "", debate_id: temp });
  useChatStore.setState({ id: chatId, streaming: true, error: "",
    messages: [...state.messages, ChatMessageSchema.parse({ role: "user", content: request }), assistant] });
  let live: DebateLive = { record: DebateSchema.parse({ id: temp, request, folder: state.settings.folder, created_at: Date.now() / 1000 }), activity: "", phase: "round", round: 1, live: true };
  useCardStore.getState().debate(temp, live);
  let currentId = temp;
  const attach = (id: string) => {
    controllers.set(id, controller); currentId = id;
    useChatStore.setState({ messages: useChatStore.getState().messages.map(m => m === assistant || m.debate_id === temp ? { ...m, debate_id: id } : m) });
  };
  try {
    await debatesApi.start({ request, folder: state.settings.folder, chat_id: chatId }, controller.signal, (event, raw) => {
      live = debateReducer(live, event, raw); attach(live.record.id); useCardStore.getState().debate(live.record.id, live);
      if (event === "done") {
        const content = Array.from(live.record.verdict ? `${t.verdict}\n\n${live.record.verdict.decision}\n${live.record.verdict.plan_steps.map((step, i) => `${i + 1}. ${step}`).join("\n")}` : live.record.error || t.stopped).slice(0, 1500).join("");
        useChatStore.setState({ messages: useChatStore.getState().messages.map(m => m.debate_id === live.record.id ? { ...m, content } : m) });
      }
    });
  } catch (error) {
    const message = controller.signal.aborted ? t.stopped : error instanceof Error ? error.message : t.error;
    useCardStore.getState().debate(currentId, { ...live, live: false, record: { ...live.record, error: message, status: controller.signal.aborted ? "stopped" : "error" } });
    useChatStore.setState({ error: message });
  } finally { controllers.delete(temp); controllers.delete(currentId); useChatStore.setState({ streaming: false }); }
}
