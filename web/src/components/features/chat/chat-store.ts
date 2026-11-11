import { create } from "zustand";
import { ChatMessageSchema, type ChatMessage, type ChatRecord, type ChatRequest, type StreamData } from "@/schema/chat.schema";
type Settings = Omit<ChatRequest, "chat_id" | "messages">;
type ChatState = {
  id: string; messages: ChatMessage[]; settings: Settings; streaming: boolean; drawer: boolean;
  status: StreamData; error: string; initialized: boolean;
  configure: (settings: Partial<Settings>) => void;
  initialize: (folder: string, provider_id: string, model: string) => void;
  load: (chat: ChatRecord) => void; reset: () => void;
  begin: (text: string, attachments?: { id: string; mime: string }[]) => void; event: (event: string, data: StreamData) => void;
  finish: (error?: string) => void; setDrawer: (open: boolean) => void;
};
const defaults: Settings = { provider_id: "", model: "", folder: "", permission: "write", effort: "default", file_tools: "none", dev_chat: false };
export const useChatStore = create<ChatState>((set) => ({
  id: "", messages: [], settings: defaults, streaming: false, drawer: false, status: {}, error: "", initialized: false,
  configure: (settings) => set((state) => ({ settings: { ...state.settings, ...settings } })),
  initialize: (folder, provider_id, model) => set((state) => state.initialized ? {} : ({
    initialized: true, settings: { ...state.settings, folder, provider_id, model },
  })),
  // Chats created by a plan or debate have no source of their own: keep the one currently selected.
  load: (chat) => set((state) => ({ id: chat.id, messages: chat.messages, settings: {
    provider_id: chat.provider_id || state.settings.provider_id, model: chat.provider_id ? chat.model : state.settings.model,
    folder: chat.folder, permission: chat.permission,
    effort: chat.effort, file_tools: chat.file_tools, dev_chat: chat.dev_chat,
  }, status: {}, error: "", drawer: false, initialized: true })),
  reset: () => set((state) => ({ id: "", messages: [], status: {}, error: "", drawer: false,
    settings: { ...state.settings, dev_chat: false } })),
  begin: (text, attachments = []) => set((state) => ({ id: state.id || crypto.randomUUID(), streaming: true, status: {}, error: "",
    messages: [...state.messages, ChatMessageSchema.parse({ role: "user", content: text, attachments }),
      ChatMessageSchema.parse({ role: "assistant", content: "" })] })),
  event: (event, data) => set((state) => {
    if (event === "status" || event === "heartbeat") return { status: { ...state.status, ...data } };
    const messages = [...state.messages];
    const last = messages[messages.length - 1];
    if (!last) return {};
    const message = { ...last };
    if (event === "delta") message.content += data.text || "";
    if (event === "reasoning") message.reasoning += data.text || "";
    if (event === "log") message.logs = [...message.logs, data.text || ""];
    if (event === "memory") message.logs = [...message.logs, [data.scope, data.status, data.action].filter(Boolean).join(" · ")];
    if (event === "diff") message.diff = data.text || "";
    if (event === "error") message.error = data.text;
    messages[messages.length - 1] = message;
    return { messages };
  }),
  finish: (error = "") => set((state) => ({ streaming: false, error,
    messages: state.messages.map((message, index) => error && index === state.messages.length - 1 ? { ...message, error } : message) })),
  setDrawer: (drawer) => set({ drawer }),
}));
