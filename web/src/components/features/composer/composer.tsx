import { useEffect, useRef, useState } from "react";
import { ArrowUp, Square } from "lucide-react";
import { Button } from "@/components/button/button";
import { chatText } from "@/text/chat";
import { cardText as t } from "@/text/cards";
import { startCard } from "@/components/features/chat/card-actions";
import { notify } from "@/components/toast/toast";
import { useCardStore } from "@/components/features/chat/card-store";
import styles from "./composer.module.css";
export function Composer({ streaming, send, stop }: { streaming: boolean; send: (text: string) => Promise<boolean>; stop: () => Promise<void> }) {
  const [debate, setDebate] = useState(false);
  const running = useCardStore(s => s.running);
  useEffect(() => { try { setDebate(["1", "true"].includes(localStorage.getItem("debateMode") || "")); } catch { /* Storage unavailable. */ } }, []);
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
    catch (error) { setValue(draft); notify(error instanceof Error ? error.message : t.error); }
    finally { pending.current = false; input.current?.focus(); }
  }
  return <form className={styles.composer} onSubmit={(event) => { event.preventDefault(); void submit(); }}>
    <label className="sr-only" htmlFor="chat-input">{chatText.placeholder}</label>
    <div><button type="button" aria-pressed={debate} disabled={streaming || Boolean(running)} onClick={() => { setDebate(!debate); try { localStorage.setItem("debateMode", !debate ? "1" : "0"); } catch { /* Storage unavailable. */ } }}>{t.debate}</button>
      <button type="button" disabled={streaming || Boolean(running) || !value.trim()} onClick={() => { const draft = value; setValue(""); void startCard(draft, "plan").catch(error => { setValue(draft); notify(String(error)); }); }}>{t.plan}</button></div>
    <div className={styles.box}><textarea id="chat-input" ref={input} value={value} rows={1} disabled={streaming}
      placeholder={chatText.placeholder} onChange={(event) => setValue(event.target.value)}
      onKeyDown={(event) => { if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); void submit(); } }} />
      {streaming ? <Button onClick={() => void stop()} aria-label={chatText.stop}><Square size={16} />{chatText.stop}</Button> :
        <Button type="submit" disabled={!value.trim() || Boolean(running)} aria-label={chatText.send}><ArrowUp size={18} /></Button>}
    </div><p>{chatText.hint}</p>
  </form>;
}
