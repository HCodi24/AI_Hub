import { useState } from "react";
import { Loader2, Shrink } from "lucide-react";
import { chatApi } from "@/apis/chat.api";
import { useChatStore } from "@/components/features/chat/chat-store";
import { notify } from "@/components/toast/toast";
import { chatText } from "@/text/chat";
import styles from "./composer.module.css";

/** Compacts the chat's Codex threads on demand (the same compaction the app runs by itself past the token budget). */
export function CompactButton({ disabled }: { disabled?: boolean }) {
  const chatId = useChatStore(s => s.id);
  const hasMessages = useChatStore(s => s.messages.length > 0);
  const [busy, setBusy] = useState(false);
  const t = chatText.compact;
  async function run() {
    setBusy(true);
    try {
      const result = await chatApi.compact(chatId);
      notify(result.failed.length ? t.failed(result.failed[0]) : result.compacted ? t.done(result.compacted) : t.none);
    } catch (error) { notify(t.failed(error instanceof Error ? error.message : String(error))); }
    finally { setBusy(false); }
  }
  return <button className={styles.compactButton} type="button" title={t.title} disabled={disabled || busy || !hasMessages} onClick={() => void run()}>
    {busy ? <Loader2 size={16} className="spin" aria-hidden="true" /> : <Shrink size={16} aria-hidden="true" />}{busy ? t.busy : t.label}
  </button>;
}
