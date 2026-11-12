import { useQuery, useQueryClient } from "@tanstack/react-query";
import { fileToolsFor } from "@/lib/agent-source";
import { useRef } from "react";
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
  const deleting = useRef(false);
  const config = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) });
  const chats = useQuery({ queryKey: ["chats"], queryFn: ({ signal }) => chatApi.list(signal) });
  async function send(text: string, direct = false, attachments: { id: string; mime: string }[] = []) {
    const state = useChatStore.getState();
    if (deleting.current || state.streaming || useCardStore.getState().running) return false;
    const mutis = await mutisApi.get();
    let debate = false;
    try { debate = ["1", "true"].includes(localStorage.getItem("debateMode") || ""); } catch { /* Storage unavailable. */ }
    if (!direct && !attachments.length && debate) {
      await startCard(text, "debate"); await client.invalidateQueries({ queryKey: ["chats"] }); return true;
    }
    if (!direct && !attachments.length && state.settings.provider_id === "mutis" && mutis.enabled && mutis.auto_plan && !state.settings.dev_chat) {
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
    // Settings requests can finish after the user switches chats or sends another reply.
    const latest = useChatStore.getState();
    if (deleting.current || latest.id !== state.id || latest.messages !== state.messages || latest.streaming || useCardStore.getState().running) return false;
    state.begin(text, attachments);
    const current = useChatStore.getState();
    const controller = new AbortController(); activeChatAbort = controller;
    try {
      const source = config.data?.providers.find(item => item.id === settings.provider_id);
      await chatApi.send({ ...settings, file_tools: fileToolsFor(source, settings.permission), folder: settings.folder.trim(), model: settings.model.trim(),
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
    if (deleting.current || useChatStore.getState().streaming) return;
    useChatStore.setState({ error: "" });
    try {
      const record = await client.fetchQuery({ queryKey: ["chat", id], queryFn: ({ signal }) => chatApi.get(id, signal), staleTime: 0 });
      if (!deleting.current && !useChatStore.getState().streaming) useChatStore.getState().load(record);
    } catch (error: unknown) { useChatStore.setState({ error: error instanceof Error ? error.message : chatText.requestError }); }
  }
  async function remove(id: string) {
    if (deleting.current || useChatStore.getState().streaming || useCardStore.getState().running) return false;
    deleting.current = true;
    try {
      await client.cancelQueries({ queryKey: ["chat", id] });
      await chatApi.delete(id);
      if (useChatStore.getState().id === id) useChatStore.getState().reset();
      client.removeQueries({ queryKey: ["chat", id] });
      client.setQueryData<NonNullable<typeof chats.data>>(["chats"], current => current?.filter(chat => chat.id !== id));
      try { localStorage.removeItem(`ai-hub-chat-selection:${id}`); } catch { /* Storage unavailable. */ }
      await client.invalidateQueries({ queryKey: ["chats"] });
      return true;
    } catch (error: unknown) {
      useChatStore.setState({ error: error instanceof Error ? error.message : chatText.deleteFailed });
      return false;
    } finally { deleting.current = false; }
  }
  return { config, chats, send, stop, load, remove };
}
