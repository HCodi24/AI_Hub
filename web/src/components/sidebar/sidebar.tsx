import { useEffect, useRef } from "react";
import { Plus, MessageSquare, ArrowUpRight, X, Layers } from "lucide-react";
import { Button } from "@/components/button/button";
import { IconButton } from "@/components/icon-button/icon-button";
import { useChatStore } from "@/components/features/chat/chat-store";
import { Spinner } from "@/components/spinner/spinner";
import type { ChatSummary } from "@/schema/chat.schema";
import { chatText } from "@/text/chat";
import { routes } from "@/routes/routes";
import { cn } from "@/lib/utils";
import styles from "./sidebar.module.css";
export function Sidebar({ chats, loading, load }: { chats: ChatSummary[]; loading: boolean; load: (id: string) => Promise<void> }) {
  const { id, drawer, streaming, reset, setDrawer } = useChatStore();
  const panel = useRef<HTMLElement>(null);
  useEffect(() => {
    if (!drawer) return;
    const previous = document.activeElement;
    const controls = () => [...(panel.current?.querySelectorAll<HTMLElement>("button:not(:disabled), a[href]") || [])];
    controls()[0]?.focus();
    function key(event: KeyboardEvent) {
      if (event.key === "Escape") setDrawer(false);
      if (event.key !== "Tab") return;
      const items = controls(); const first = items[0]; const last = items.at(-1);
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
      if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    }
    document.addEventListener("keydown", key);
    return () => { document.removeEventListener("keydown", key); if (previous instanceof HTMLElement) previous.focus(); };
  }, [drawer, setDrawer]);
  return <>
    {drawer && <button className={styles.backdrop} aria-label={chatText.close} onClick={() => setDrawer(false)} />}
    <aside ref={panel} className={cn(styles.sidebar, drawer && styles.open)} aria-label={chatText.history}>
      <div className={styles.brand}><Layers size={24} aria-hidden="true" /><strong>{chatText.brand}</strong><span>{chatText.beta}</span>
        <div className={styles.close}><IconButton aria-label={chatText.close} onClick={() => setDrawer(false)}><X size={18} /></IconButton></div></div>
      <Button disabled={streaming} onClick={reset}><Plus size={18} />{chatText.newChat}</Button>
      <h2>{chatText.history}</h2>
      <nav className={styles.history} aria-label={chatText.history}>
        {loading && <Spinner />}{!loading && !chats.length && <p>{chatText.empty}</p>}
        {chats.map((chat) => <button key={chat.id} disabled={streaming} aria-current={chat.id === id ? "page" : undefined}
          onClick={() => void load(chat.id)}><MessageSquare size={15} aria-hidden="true" /><span>{chat.title || chatText.newChat}</span></button>)}
      </nav>
      <a className={styles.legacy} href={routes.legacy}>{chatText.legacy}<ArrowUpRight size={16} aria-hidden="true" /></a>
    </aside>
  </>;
}
