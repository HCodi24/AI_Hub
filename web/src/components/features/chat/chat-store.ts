import { create } from "zustand";
import { ChatMessageSchema, PermissionSchema, type ChatMessage, type ChatRecord, type ChatRequest, type StreamData } from "@/schema/chat.schema";
type Settings = Omit<ChatRequest, "chat_id" | "messages">;
type ModelSelection = Pick<Settings, "provider_id" | "model"> & Partial<Pick<Settings, "permission">>;
type ChatState = {
  id: string; messages: ChatMessage[]; settings: Settings; streaming: boolean; drawer: boolean;
  status: StreamData; error: string; initialized: boolean;
  configure: (settings: Partial<Settings>) => void;
  initialize: (folder: string, provider_id: string, model: string) => void;
  load: (chat: ChatRecord) => void; reset: () => void;
  begin: (text: string, attachments?: { id: string; mime: string }[]) => void; event: (event: string, data: StreamData) => void;
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
  id: "", messages: [], settings: defaults, streaming: false, drawer: false, status: {}, error: "", initialized: false,
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
      return { id: chat.id, messages: chat.messages, settings, status: {}, error: "", drawer: false, initialized: true };
    });
  },
  reset: () => {
    rememberActiveChat("");
    set((state) => {
      const settings = { ...state.settings, dev_chat: false };
      saveModelSelection(DRAFT_ID, settings);
      return { id: "", messages: [], status: {}, error: "", drawer: false, settings };
    });
  },
  begin: (text, attachments = []) => set((state) => {
    const id = state.id || crypto.randomUUID();
    rememberActiveChat(id);
    if (!state.id) saveModelSelection(id, state.settings);
    return { id, streaming: true, status: {}, error: "",
      messages: [...state.messages, ChatMessageSchema.parse({ role: "user", content: text, attachments }),
        ChatMessageSchema.parse({ role: "assistant", content: "" })] };
  }),
  event: (event, data) => set((state) => {
    if (event === "turn") {
      const provider = data.provider_kind;
      if (provider !== "codex_cli" && provider !== "deepseek") return {};
      const last = state.messages.at(-1);
      const empty = last?.role === "assistant" && !last.content && !last.reasoning && !last.logs.length &&
        !last.diff && !last.files.length && !last.attachments.length && last.error == null && last.plan_id == null && last.debate_id == null;
      const next = ChatMessageSchema.parse({ role: "assistant", content: "", provider });
      return { messages: empty ? [...state.messages.slice(0, -1), next] : [...state.messages, next],
        status: { ...state.status, provider_kind: provider, provider_name: data.provider_name || (provider === "deepseek" ? "DeepSeek" : "Codex"),
          model: data.model || "", idle_s: 0, last_activity: "" } };
    }
    if (event === "heartbeat") return { status: { ...state.status, ...data } };
    if (event === "status") {
      const last = state.messages.at(-1);
      // The reply's provider decides its avatar while it streams (the saved copy has it from the server).
      const messages = last && data.provider_kind && !last.provider
        ? [...state.messages.slice(0, -1), { ...last, provider: data.provider_kind }] : state.messages;
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
  finish: (error = "") => set((state) => ({ streaming: false, error,
    messages: state.messages.map((message, index) => error && index === state.messages.length - 1 ? { ...message, error } : message) })),
  setDrawer: (drawer) => set({ drawer }),
}));
