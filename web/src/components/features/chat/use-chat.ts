import { useQuery, useQueryClient } from "@tanstack/react-query";
import { chatApi } from "@/apis/chat.api";
import { configApi } from "@/apis/config.api";
import { useChatStore } from "./chat-store";
import { chatText } from "@/text/chat";
import { mutisApi } from "@/apis/mutis.api";
import { startCard, cancelActiveCard } from "./card-actions";
import { useCardStore } from "./card-store";
import { memoryNotice } from "@/components/features/memory/memory-events";
import { useToast } from "@/components/toast/toast";
let activeChatAbort: AbortController | null = null;
export function useChat() {
  const client = useQueryClient();
  const config = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) });
  const chats = useQuery({ queryKey: ["chats"], queryFn: ({ signal }) => chatApi.list(signal) });
  async function send(text: string, direct = false) {
    const state = useChatStore.getState();
    if (state.streaming || useCardStore.getState().running) return false;
    const mutis = await mutisApi.get();
    let debate = false;
    try { debate = ["1", "true"].includes(localStorage.getItem("debateMode") || ""); } catch { /* Storage unavailable. */ }
    if (!direct && debate) {
      await startCard(text, "debate"); await client.invalidateQueries({ queryKey: ["chats"] }); return true;
    }
    if (!direct && state.settings.provider_id === "mutis" && mutis.enabled && mutis.auto_plan && !state.settings.dev_chat) {
      await startCard(text, "plan"); await client.invalidateQueries({ queryKey: ["chats"] }); return true;
    }
    let settings = state.settings;
    if (settings.provider_id === "mutis") {
      const resolved = await mutisApi.resolve(); const lead = resolved.lead.used_provider;
      if (!lead) { useChatStore.setState({ error: chatText.selectRequired }); return false; }
      const role = mutis.roles.lead;
      settings = { ...settings, provider_id: lead.id, model: role.provider_id === lead.id ? role.model || lead.models[0] || "" : lead.models[0] || "", effort: role.effort };
    }
    if (!settings.provider_id || !settings.model.trim() || !settings.folder.trim()) {
      useChatStore.setState({ error: chatText.selectRequired }); return false;
    }
    state.begin(text);
    const current = useChatStore.getState();
    const controller = new AbortController(); activeChatAbort = controller;
    try {
      await chatApi.send({ ...settings, folder: settings.folder.trim(), model: settings.model.trim(),
        chat_id: current.id, messages: current.messages.slice(0, -1) }, controller.signal, (event, data) => {
        current.event(event, data);
        if (event === "memory") useToast.getState().show(memoryNotice(data, settings.folder));
      });
      current.finish();
    } catch (error: unknown) {
      current.finish(controller.signal.aborted ? chatText.stopped : error instanceof Error ? error.message : chatText.requestError);
    } finally { activeChatAbort = null; await client.invalidateQueries({ queryKey: ["chats"] }); }
    return true;
  }
  async function stop() {
    if (cancelActiveCard()) return;
    try { await chatApi.stop(useChatStore.getState().id); activeChatAbort?.abort(); }
    catch (error: unknown) { useChatStore.setState({ error: error instanceof Error ? error.message : chatText.requestError }); }
  }
  async function load(id: string) {
    if (useChatStore.getState().streaming) return;
    useChatStore.setState({ error: "" });
    try {
      const record = await client.fetchQuery({ queryKey: ["chat", id], queryFn: ({ signal }) => chatApi.get(id, signal), staleTime: 0 });
      if (!useChatStore.getState().streaming) useChatStore.getState().load(record);
    } catch (error: unknown) { useChatStore.setState({ error: error instanceof Error ? error.message : chatText.requestError }); }
  }
  return { config, chats, send, stop, load };
}
