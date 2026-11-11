import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useRef } from "react";
import { chatApi } from "@/apis/chat.api";
import { configApi } from "@/apis/config.api";
import { useChatStore } from "./chat-store";
import { chatText } from "@/text/chat";
export function useChat() {
  const client = useQueryClient();
  const abort = useRef<AbortController | null>(null);
  const config = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) });
  const chats = useQuery({ queryKey: ["chats"], queryFn: ({ signal }) => chatApi.list(signal) });
  async function send(text: string) {
    const state = useChatStore.getState();
    if (state.streaming) return false;
    if (!state.settings.provider_id || !state.settings.model.trim() || !state.settings.folder.trim()) {
      useChatStore.setState({ error: chatText.selectRequired }); return false;
    }
    state.begin(text);
    const current = useChatStore.getState();
    const controller = new AbortController(); abort.current = controller;
    try {
      await chatApi.send({ ...current.settings, folder: current.settings.folder.trim(), model: current.settings.model.trim(),
        chat_id: current.id, messages: current.messages.slice(0, -1) }, controller.signal, current.event);
      current.finish();
    } catch (error: unknown) {
      current.finish(controller.signal.aborted ? chatText.stopped : error instanceof Error ? error.message : chatText.requestError);
    } finally { abort.current = null; await client.invalidateQueries({ queryKey: ["chats"] }); }
    return true;
  }
  async function stop() {
    try { await chatApi.stop(useChatStore.getState().id); abort.current?.abort(); }
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
