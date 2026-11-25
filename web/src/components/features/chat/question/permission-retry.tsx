import { useRef, useState } from "react";
import { Button } from "@/components/button/button";
import { notify } from "@/components/toast/toast";
import { chatText } from "@/text/chat";
import { useChatStore } from "../chat-store";
import { useCardStore } from "../card-store";
import type { SendReply } from "./quick-replies";
import styles from "../chat-message.module.css";

/** The run was blocked by the chat's permission: offer the same message again one level up (the user's click is the approval). */
export function PermissionRetry({ need, text, send }: { need: "write" | "full"; text: string; send: SendReply }) {
  const pending = useRef(false);
  const [sending, setSending] = useState(false);
  const running = useCardStore(s => s.running);
  async function rerun() {
    if (pending.current || useChatStore.getState().streaming || useCardStore.getState().running) return;
    pending.current = true; setSending(true);
    try {
      useChatStore.getState().configure({ permission: need });
      await send(text, true);
    } catch (error) { notify(error instanceof Error ? error.message : chatText.requestError); }
    finally { pending.current = false; setSending(false); }
  }
  return <div className={styles.choices} role="group" aria-label={chatText.denied} aria-busy={sending}>
    <p>{chatText.denied}</p>
    <Button variant="primary" disabled={sending || Boolean(running)} onClick={() => void rerun()}>{chatText.rerun(chatText[need])}</Button>
  </div>;
}
