import { useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { useChatStore } from "../chat-store";
import styles from "./turn-navigator.module.css";

export function TurnNavigator({ activeIndex, onSelect }: {
  activeIndex: number;
  onSelect: (messageIndex: number) => void;
}) {
  const id = useChatStore(state => state.id);
  const storageKey = id ? `ai-hub-turn-marks:${id}` : "";
  const [marks, setMarks] = useState<Set<number>>(() => {
    if (!storageKey) return new Set();
    try {
      const stored: unknown = JSON.parse(localStorage.getItem(storageKey) ?? "[]");
      if (Array.isArray(stored)) return new Set(stored.filter((index): index is number => typeof index === "number"));
    } catch { /* Storage unavailable or invalid. */ }
    return new Set();
  });
  useEffect(() => {
    if (!storageKey) return;
    try {
      if (marks.size) localStorage.setItem(storageKey, JSON.stringify([...marks]));
      else localStorage.removeItem(storageKey);
    } catch { /* Storage unavailable. */ }
  }, [marks, storageKey]);

  function toggle(messageIndex: number) {
    setMarks(current => {
      const next = new Set(current);
      if (!next.delete(messageIndex)) next.add(messageIndex);
      return next;
    });
  }

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
  const previewMarked = previewIndex !== undefined && marks.has(previewIndex);

  return <nav className={styles.nav} aria-label="Điều hướng lượt trò chuyện">
    <div className={styles.track}>
      {turns.map(({ index: messageIndex, label }) => {
        const marked = marks.has(messageIndex);
        return <button key={messageIndex} type="button" className={styles.marker}
          data-marked={marked ? "true" : undefined}
          aria-label={marked ? `Đã đánh dấu: ${label}` : label}
          aria-current={messageIndex === activeTurn ? "true" : undefined}
          title={marked ? "Chuột phải để bỏ đánh dấu" : "Chuột phải để đánh dấu"}
          onPointerEnter={event => {
            if (event.pointerType !== "touch") setHovered(messageIndex);
          }}
          onPointerLeave={() => setHovered(undefined)}
          onFocus={() => setFocused(messageIndex)} onBlur={() => setFocused(undefined)}
          onContextMenu={event => { event.preventDefault(); toggle(messageIndex); }}
          onClick={() => onSelect(messageIndex)} />;
      })}
    </div>
    {preview && <div className={styles.preview} aria-hidden="true">
      {previewMarked && <span className={styles.markedTag}>★ Đã đánh dấu</span>}
      <strong>{preview}</strong>
    </div>}
  </nav>;
}
