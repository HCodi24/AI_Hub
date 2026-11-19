import { useQuery, useQueryClient } from "@tanstack/react-query";
import { fileToolsFor } from "@/lib/agent-source";
import { useRef } from "react";
import { chatApi } from "@/apis/chat.api";
import { configApi } from "@/apis/config.api";
import { useChatStore } from "./chat-store";
import { chatText } from "@/text/chat";
import { mutisApi } from "@/apis/mutis.api";
import { rolesFor } from "@/lib/mutis-level";
import { startDebate, cancelActiveCard } from "./card-actions";
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
    if (deleting.current || useCardStore.getState().running) return false;
    // A run is in progress: the message waits in line and is sent when the run ends (all sources, same rule).
    if (state.streaming) { state.enqueue({ text, attachments }); return true; }
    let debate = false;
    try { debate = ["1", "true"].includes(localStorage.getItem("debateMode") || ""); } catch { /* Storage unavailable. */ }
    if (state.settings.folder.trim() && !direct && !attachments.length && debate) {
      await startDebate(text); await client.invalidateQueries({ queryKey: ["chats"] }); return true;
    }
    // Only the MutiS source needs its settings before the message is shown.
    const mutis = state.settings.provider_id === "mutis" ? await mutisApi.get() : undefined;
    const settings = state.settings;
    if (!mutis && (!settings.provider_id || !settings.model.trim())) {
      useChatStore.setState({ error: chatText.selectRequired }); return false;
    }
    // MutiS shows its lead first; a single model shows itself. The identity (and so the pet) is on screen at once,
    // before the server says anything; backend events may replace this prediction.
    const lead = mutis ? rolesFor(mutis, settings.effort)?.lead || mutis.roles.lead : undefined;
    const initialSource = config.data?.providers.find(item => item.id === (lead ? lead.provider_id : settings.provider_id));
    const initialModel = (lead ? lead.model || initialSource?.models[0] || "" : settings.model).trim();
    // The message goes on screen at once; everything that waits for the server happens after it, while "streaming" blocks other actions.
    state.begin(text, attachments, initialSource?.kind || "", initialModel);
    const current = useChatStore.getState();
    const controller = new AbortController(); activeChatAbort = controller;
    try {
      // At the MutiS source the server picks the lead (its model, effort and file tools) from `settings.effort`, the level the user chose.
      const source = config.data?.providers.find(item => item.id === settings.provider_id);
      await chatApi.send({ ...settings, file_tools: mutis || !settings.folder.trim() ? "none" : fileToolsFor(source, settings.permission), folder: settings.folder.trim(), model: settings.model.trim(),
        chat_id: current.id, messages: current.messages.slice(0, -1) }, controller.signal, (event, data) => {
        current.event(event, data);
        if (event === "memory") useToast.getState().show(memoryNotice(data, settings.folder));
      });
      current.finish();
    } catch (error: unknown) {
      current.finish(controller.signal.aborted ? chatText.stopped : error instanceof Error ? error.message : chatText.requestError);
    } finally { activeChatAbort = null; await client.invalidateQueries({ queryKey: ["chats"] }); }
    // Next waiting message, unless the run failed or was stopped (stop empties the line).
    const next = useChatStore.getState().error ? undefined : useChatStore.getState().dequeue();
    if (next) await send(next.text, false, next.attachments);
    return true;
  }
  async function stop() {
    if (cancelActiveCard()) return;
    useChatStore.getState().clearQueue();
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
