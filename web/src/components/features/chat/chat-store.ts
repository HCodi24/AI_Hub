import { create } from "zustand";
import { ChatMessageSchema, PermissionSchema, type ChatMessage, type ChatRecord, type ChatRequest, type CheckerActivity, type StreamData, type TaskRow } from "@/schema/chat.schema";
type Settings = Omit<ChatRequest, "chat_id" | "messages">;
type ModelSelection = Pick<Settings, "provider_id" | "model"> & Partial<Pick<Settings, "permission">>;
/** One live Luna card: a check that is running (or failed) beside the conversation, never inside a chat message. */
export type LiveChecker = {
  id: string; title: string; attempt: number; model: string; provider_name: string;
  state: "running" | "failed" | "stopped";
  /** Latest meaningful checker lines, oldest first, bounded. */
  progress: string[];
};
/** Retained progress lines per card: enough to see what Luna is doing, never an unbounded log. */
const MAX_CHECKER_PROGRESS = 6;
/** Applies one `checker_activity` event to the cards: start/progress/error merge by id, done removes only that card. */
export function applyCheckerActivity(cards: LiveChecker[], activity: CheckerActivity): LiveChecker[] {
  if (activity.phase === "done") return cards.filter(card => card.id !== activity.id);
  const index = cards.findIndex(card => card.id === activity.id);
  const base: LiveChecker = index >= 0 ? cards[index] : {
    id: activity.id, title: activity.title, attempt: activity.attempt, model: activity.model,
    provider_name: activity.provider_name || "Luna", state: "running", progress: [],
  };
  const next: LiveChecker = {
    ...base,
    title: activity.title || base.title, attempt: activity.attempt || base.attempt,
    model: activity.model || base.model, provider_name: activity.provider_name || base.provider_name,
    state: activity.phase === "error" ? "failed" : base.state === "failed" ? "failed" : "running",
  };
  if (activity.text) next.progress = [...base.progress, activity.text].slice(-MAX_CHECKER_PROGRESS);
  return index >= 0 ? cards.map((card, at) => at === index ? next : card) : [...cards, next];
}
/** A card still running when the stream ends will never move again: mark it instead of leaving it spinning. */
function stopCheckers(cards: LiveChecker[]): LiveChecker[] {
  return cards.map(card => card.state === "running" ? { ...card, state: "stopped" } : card);
}
type ChatState = {
  id: string; messages: ChatMessage[]; settings: Settings; streaming: boolean; drawer: boolean;
  status: StreamData; error: string; initialized: boolean;
  /** Live task board of a MutiS High delegation (not saved with the chat). */
  tasks: TaskRow[];
  /** Live Luna checks, keyed by the checker's own id and independent of messages, status and tasks. */
  checkers: LiveChecker[];
  configure: (settings: Partial<Settings>) => void;
  initialize: (folder: string, provider_id: string, model: string) => void;
  load: (chat: ChatRecord) => void; reset: () => void;
  begin: (text: string, attachments?: { id: string; mime: string }[], initialProvider?: string) => void; event: (event: string, data: StreamData) => void;
  finish: (error?: string) => void; setDrawer: (open: boolean) => void;
};
const defaults: Settings = { provider_id: "", model: "", folder: "", permission: "write", effort: "medium", file_tools: "none", dev_chat: false };
const DRAFT_ID = "draft";
export const ACTIVE_CHAT_KEY = "ai-hub-active-chat";
function readModelSelection(id: string): ModelSelection | undefined {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(`ai-hub-chat-selection:${id}`) || "null");
    if (typeof value !== "object" || value === null) return;
    const selection = value as Partial<ModelSelection>;
    if (typeof selection.provider_id !== "string" || typeof selection.model !== "string") return;
    const permission = PermissionSchema.safeParse(selection.permission);
    return { provider_id: selection.provider_id, model: selection.model,
      ...(permission.success ? { permission: permission.data } : {}) };
  } catch { return; }
}
function saveModelSelection(id: string, settings: ModelSelection) {
  try { localStorage.setItem(`ai-hub-chat-selection:${id}`, JSON.stringify(settings)); } catch { /* Storage unavailable. */ }
}
function rememberActiveChat(id: string) {
  try {
    if (id) sessionStorage.setItem(ACTIVE_CHAT_KEY, id);
    else sessionStorage.removeItem(ACTIVE_CHAT_KEY);
  } catch { /* Storage unavailable. */ }
}
export const useChatStore = create<ChatState>((set) => ({
  id: "", messages: [], settings: defaults, streaming: false, drawer: false, status: {}, error: "", initialized: false, tasks: [], checkers: [],
  configure: (settings) => set((state) => {
    const next = { ...state.settings, ...settings };
    saveModelSelection(state.id || DRAFT_ID, next);
    return { settings: next };
  }),
  initialize: (folder, provider_id, model) => set((state) => {
    if (state.initialized) return {};
    const selected = readModelSelection(DRAFT_ID);
    return { initialized: true, settings: { ...state.settings, folder,
      provider_id: selected?.provider_id ?? provider_id, model: selected?.model ?? model,
      permission: selected?.permission ?? state.settings.permission } };
  }),
  // Chats created by a plan or debate have no source of their own: keep the one currently selected.
  load: (chat) => {
    rememberActiveChat(chat.id);
    set((state) => {
      const selected = readModelSelection(chat.id);
      const settings = {
        provider_id: selected?.provider_id ?? (chat.provider_id || state.settings.provider_id),
        model: selected?.model ?? (chat.provider_id ? chat.model : state.settings.model),
        folder: chat.folder, permission: selected?.permission ?? chat.permission,
        effort: chat.effort === "default" ? "medium" as const : chat.effort, file_tools: chat.file_tools, dev_chat: chat.dev_chat,
      };
      saveModelSelection(chat.id, settings);
      return { id: chat.id, messages: chat.messages, settings, status: {}, tasks: [], checkers: [], error: "", drawer: false, initialized: true };
    });
  },
  reset: () => {
    rememberActiveChat("");
    set((state) => {
      const settings = { ...state.settings, dev_chat: false };
      saveModelSelection(DRAFT_ID, settings);
      return { id: "", messages: [], status: {}, tasks: [], checkers: [], error: "", drawer: false, settings };
    });
  },
  begin: (text, attachments = [], initialProvider = "") => set((state) => {
    const id = state.id || crypto.randomUUID();
    rememberActiveChat(id);
    if (!state.id) saveModelSelection(id, state.settings);
    return { id, streaming: true, status: {}, tasks: [], checkers: [], error: "",
      messages: [...state.messages, ChatMessageSchema.parse({ role: "user", content: text, attachments }),
        ChatMessageSchema.parse({ role: "assistant", content: "", provider: initialProvider })] };
  }),
  event: (event, data) => set((state) => {
    if (event === "turn") {
      const provider = data.provider_kind;
      if (provider !== "codex_cli" && provider !== "deepseek") return {};
      const last = state.messages.at(-1);
      const empty = last?.role === "assistant" && !last.content && !last.reasoning && !last.logs.length &&
        !last.diff && !last.files.length && !last.attachments.length && last.error == null && last.plan_id == null && last.debate_id == null;
      // A new turn starts a fresh reply: the placeholder's previous identity is dropped, never inferred from settings.
      const next = ChatMessageSchema.parse({ role: "assistant", content: "", provider, model: data.model || "" });
      return { messages: empty ? [...state.messages.slice(0, -1), next] : [...state.messages, next],
        status: { ...state.status, provider_kind: provider, provider_name: data.provider_name || (provider === "deepseek" ? "DeepSeek" : "Codex"),
          model: data.model || "", idle_s: 0, last_activity: "" } };
    }
    if (event === "heartbeat") return { status: { ...state.status, ...data } };
    if (event === "tasks") return { tasks: data.tasks ?? [] };
    // Live Luna cards are handled before any trailing-message update: a checker event never touches the
    // streaming worker's message, and a worker event never touches a card.
    if (event === "checker_activity") {
      return data.checker_activity ? { checkers: applyCheckerActivity(state.checkers, data.checker_activity) } : {};
    }
    if (event === "status") {
      const last = state.messages.at(-1);
      // The selected provider seeds the assistant identity; backend status may replace that prediction.
      const provider = data.provider_kind && last?.role === "assistant" && state.streaming ? data.provider_kind : undefined;
      // The model is stamped on the reply that is actually streaming; heartbeats never relabel it.
      const model = data.model && last?.role === "assistant" ? data.model : undefined;
      const messages = last && (provider || model)
        ? [...state.messages.slice(0, -1), { ...last, ...(provider ? { provider } : {}), ...(model ? { model } : {}) }]
        : state.messages;
      return { status: { ...state.status, ...data }, messages };
    }
    const messages = [...state.messages];
    const last = messages[messages.length - 1];
    if (!last) return {};
    const message = { ...last };
    if (event === "delta") message.content += data.text || "";
    if (event === "reasoning") message.reasoning += data.text || "";
    if (event === "log") message.logs = [...message.logs, data.text || ""];
    if (event === "memory") message.logs = [...message.logs, [data.scope, data.status, data.action].filter(Boolean).join(" · ")];
    if (event === "diff") message.diff = data.text || "";
    if (event === "files") message.files = data.files || [];
    if (event === "error") message.error = data.text;
    messages[messages.length - 1] = message;
    return { messages };
  }),
  // A row still waiting or busy when the stream ends will never move again (stopped, or the connection dropped).
  finish: (error = "") => set((state) => ({ streaming: false, error,
    tasks: state.tasks.map(task => ["waiting", "coding", "checking", "fixing"].includes(task.state) ? { ...task, state: "stopped" } : task),
    checkers: stopCheckers(state.checkers),
    messages: state.messages.map((message, index) => error && index === state.messages.length - 1 ? { ...message, error } : message) })),
  setDrawer: (drawer) => set({ drawer }),
}));
