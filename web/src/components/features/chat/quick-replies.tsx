import { useRef, useState } from "react";
import { Button } from "@/components/button/button";
import { notify } from "@/components/toast/toast";
import { chatText } from "@/text/chat";
import { chatChoices } from "@/lib/chat-choices";
import { useChatStore } from "./chat-store";
import { useCardStore } from "./card-store";
import styles from "./chat-message.module.css";

export type SendReply = (text: string, direct?: boolean) => Promise<boolean>;
export function QuickReplies({ content, send }: { content: string; send: SendReply }) {
  const streaming = useChatStore(s => s.streaming);
  const running = useCardStore(s => s.running);
  const pending = useRef(false);
  const [sending, setSending] = useState(false);
  const choices = chatChoices(content);
  async function reply(choice: string) {
    if (pending.current || useChatStore.getState().streaming || useCardStore.getState().running) return;
    pending.current = true; setSending(true);
    try { await send(choice, true); }
    catch (error) { notify(error instanceof Error ? error.message : chatText.requestError); }
    finally { pending.current = false; setSending(false); }
  }
  if (streaming || !choices.length) return null;
  return <div className={styles.choices} role="group" aria-label={chatText.quickReply} aria-busy={sending}>
    {choices.map(choice => <Button key={choice} disabled={sending || Boolean(running)} onClick={() => void reply(choice)}>{choice}</Button>)}
  </div>;
}
