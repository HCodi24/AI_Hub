import { useEffect, useRef, useState } from "react";
import { ArrowUp, Square } from "lucide-react";
import { Button } from "@/components/button/button";
import { chatText } from "@/text/chat";
import styles from "./composer.module.css";
export function Composer({ streaming, send, stop }: { streaming: boolean; send: (text: string) => Promise<boolean>; stop: () => Promise<void> }) {
  const [value, setValue] = useState("");
  const input = useRef<HTMLTextAreaElement>(null);
  const pending = useRef(false);
  useEffect(() => {
    if (input.current) { input.current.style.height = "auto"; input.current.style.height = `${Math.min(input.current.scrollHeight, 180)}px`; }
  }, [value]);
  async function submit() {
    if (pending.current || streaming || !value.trim()) return;
    pending.current = true;
    const draft = value; setValue("");
    try { if (!(await send(draft.trim()))) setValue(draft); }
    finally { pending.current = false; input.current?.focus(); }
  }
  return <form className={styles.composer} onSubmit={(event) => { event.preventDefault(); void submit(); }}>
    <label className="sr-only" htmlFor="chat-input">{chatText.placeholder}</label>
    <div className={styles.box}><textarea id="chat-input" ref={input} value={value} rows={1} disabled={streaming}
      placeholder={chatText.placeholder} onChange={(event) => setValue(event.target.value)}
      onKeyDown={(event) => { if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); void submit(); } }} />
      {streaming ? <Button onClick={() => void stop()} aria-label={chatText.stop}><Square size={16} />{chatText.stop}</Button> :
        <Button type="submit" disabled={!value.trim()} aria-label={chatText.send}><ArrowUp size={18} /></Button>}
    </div><p>{chatText.hint}</p>
  </form>;
}
