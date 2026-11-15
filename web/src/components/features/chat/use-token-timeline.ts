import { useEffect, useRef, useState } from "react";
import { chatApi } from "@/apis/chat.api";
import type { ChatTokenUsage } from "@/lib/chat-token-usage";
import type { TokenSample } from "@/lib/chat-token-timeline";

export type TokenTimelineStatus = "loading" | "ready" | "error";
export type TokenTimeline = { samples: TokenSample[]; status: TokenTimelineStatus };
/** Modest polling so reports recorded while the Dashboard is open still appear. */
export const TOKEN_TIMELINE_POLL_MS = 3000;

type View = { id: string; samples: TokenSample[]; status: TokenTimelineStatus };

/** No chat selected: nothing to fetch, so the history is honestly empty instead of loading forever. */
const EMPTY_TIMELINE: TokenTimeline = { samples: [], status: "ready" };

/**
 * Canonical identity of the reported usage: one entry per model with its input/output, sorted by
 * model. Text-only deltas keep the same key, while a redistribution between models changes it and
 * therefore refetches the backend history.
 */
export function tokenUsageKey(usage: ChatTokenUsage): string {
  return usage.models
    .map(row => `${row.model}\u0000${row.input}\u0000${row.output}`)
    .sort()
    .join("\u0001");
}

/**
 * Backend-recorded token history of one chat. No client observation timestamps and no browser
 * storage: the server owns the timeline, this hook only reads it.
 */
export function useTokenTimeline(chatId: string, usage: ChatTokenUsage): TokenTimeline {
  // Only the reported per-model counts matter, so text-only deltas never restart the request.
  const usageKey = tokenUsageKey(usage);
  const [view, setView] = useState<View>({ id: chatId, samples: [], status: chatId ? "loading" : "ready" });
  const latest = useRef<View>({ id: chatId, samples: [], status: chatId ? "loading" : "ready" });
  const inFlight = useRef<AbortController | null>(null);

  useEffect(() => {
    // An empty id would request `/chats//token-history`; skip the request and the polling entirely.
    if (!chatId) {
      latest.current = { id: "", samples: [], status: "ready" };
      setView(latest.current);
      return;
    }
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    // A chat switch must never show the previous chat's samples, not even for one frame.
    if (latest.current.id !== chatId) {
      latest.current = { id: chatId, samples: [], status: "loading" };
      setView(latest.current);
    }
    const load = async () => {
      if (disposed || inFlight.current) return;
      const controller = new AbortController();
      inFlight.current = controller;
      try {
        const samples = await chatApi.tokenHistory(chatId, controller.signal);
        if (disposed || controller.signal.aborted) return;
        latest.current = { id: chatId, samples, status: "ready" };
        setView(latest.current);
      } catch {
        if (disposed || controller.signal.aborted) return;
        // Keep the last successful samples of this chat; the next poll retries automatically.
        latest.current = { id: chatId, samples: latest.current.id === chatId ? latest.current.samples : [], status: "error" };
        setView(latest.current);
      } finally {
        if (inFlight.current === controller) inFlight.current = null;
        if (!disposed) timer = setTimeout(() => { void load(); }, TOKEN_TIMELINE_POLL_MS);
      }
    };
    void load();
    return () => {
      disposed = true;
      if (timer !== undefined) clearTimeout(timer);
      if (inFlight.current) {
        inFlight.current.abort();
        inFlight.current = null;
      }
    };
  }, [chatId, usageKey]);

  if (!chatId) return EMPTY_TIMELINE;
  return view.id === chatId ? { samples: view.samples, status: view.status } : { samples: [], status: "loading" };
}
