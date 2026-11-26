import { useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { useChatStore } from "../chat-store";
import styles from "./turn-navigator.module.css";

export function TurnNavigator({ activeIndex, onSelect }: {
  activeIndex: number;
  onSelect: (messageIndex: number) => void;
}) {
  const userMessages = useChatStore(useShallow(state => {
    const messages: Record<number, string> = {};
    state.messages.forEach((message, index) => {
      if (message.role === "user") messages[index] = message.content;
    });
    return messages;
  }));
  const [hovered, setHovered] = useState<number>();
  const [focused, setFocused] = useState<number>();
  const turns = useMemo(() => Object.entries(userMessages).map(([index, content], turn) => {
    const text = content.trim().replace(/\s+/g, " ");
    const label = text.length > 100 ? `${text.slice(0, 100)}…` : text;
    return { index: Number(index), label: label || `Lượt ${turn + 1}` };
  }), [userMessages]);
  if (turns.length < 2) return null;

  const activeTurn = turns.reduce((current, { index }) => index <= activeIndex ? index : current, turns[0].index);
  const previewIndex = hovered ?? focused;
  const preview = previewIndex === undefined ? undefined : userMessages[previewIndex]?.trim();

  return <nav className={styles.nav} aria-label="Điều hướng lượt trò chuyện">
    <div className={styles.track}>
      {turns.map(({ index: messageIndex, label }) => (
        <button key={messageIndex} type="button" className={styles.marker}
          aria-label={label} aria-current={messageIndex === activeTurn ? "true" : undefined}
          onPointerEnter={event => {
            if (event.pointerType !== "touch") setHovered(messageIndex);
          }}
          onPointerLeave={() => setHovered(undefined)}
          onFocus={() => setFocused(messageIndex)} onBlur={() => setFocused(undefined)}
          onClick={() => onSelect(messageIndex)} />
      ))}
    </div>
    {preview && <div className={styles.preview} aria-hidden="true"><strong>{preview}</strong></div>}
  </nav>;
}
