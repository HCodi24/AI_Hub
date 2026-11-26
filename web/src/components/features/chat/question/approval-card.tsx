import { useState } from "react";
import { chatApi } from "@/apis/chat.api";
import { Button } from "@/components/button/button";
import { notify } from "@/components/toast/toast";
import { chatText } from "@/text/chat";
import { useChatStore } from "../chat-store";
import styles from "../chat-message.module.css";

const LONG_COMMAND = 80;

/** The run is paused on a command the chat's permission would refuse: the user decides, once, for this command only. */
export function ApprovalCard({ id, command }: { id: string; command: string }) {
  const [sending, setSending] = useState(false);
  const firstLineEnd = command.search(/[\r\n]/);
  const isLongCommand = firstLineEnd !== -1 || command.length > LONG_COMMAND;
  const firstLine = firstLineEnd === -1 ? command : command.slice(0, firstLineEnd);
  async function answer(allow: boolean) {
    setSending(true);
    try { await chatApi.approve(id, allow); useChatStore.setState({ approval: null }); }
    catch (error) { notify(error instanceof Error ? error.message : chatText.requestError); useChatStore.setState({ approval: null }); }
    finally { setSending(false); }
  }
  return <div className={styles.choices} role="group" aria-label={chatText.approveTitle} aria-busy={sending}>
    <p>{chatText.approveTitle}</p>
    {isLongCommand ? (
      <details key={id} className={styles.command}>
        <summary>
          {chatText.approveShowCommand}{" · "}
          <code>{firstLine}</code>
        </summary>
        <pre>{command}</pre>
      </details>
    ) : <pre>{command}</pre>}
    <Button variant="primary" disabled={sending} onClick={() => void answer(true)}>{chatText.approveAllow}</Button>
    <Button disabled={sending} onClick={() => void answer(false)}>{chatText.approveDeny}</Button>
  </div>;
}
