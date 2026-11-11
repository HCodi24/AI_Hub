import { useEffect, useRef, useState } from "react";
import { Plus, MessageSquare, X, Layers, PanelLeftClose, PanelLeftOpen, Settings, Hammer } from "lucide-react";
import { Button } from "@/components/button/button";
import { IconButton } from "@/components/icon-button/icon-button";
import { useChatStore } from "@/components/features/chat/chat-store";
import { Skeleton } from "@/components/skeleton/skeleton";
import { ThemeControl } from "@/components/theme-control/theme-control";
import { useShallow } from "zustand/react/shallow";
import type { ChatSummary } from "@/schema/chat.schema";
import { chatText } from "@/text/chat";
import { settingsText } from "@/text/settings";
import { cn } from "@/lib/utils";
import styles from "./sidebar.module.css";
export function Sidebar({ chats, loading, load, openSettings, openDevChat }: { chats: ChatSummary[]; loading: boolean; load: (id: string) => Promise<void>; openSettings: () => void; openDevChat?: () => void }) {
  const { id, drawer, streaming, reset, setDrawer } = useChatStore(useShallow(s => ({ id: s.id, drawer: s.drawer, streaming: s.streaming, reset: s.reset, setDrawer: s.setDrawer })));
  const [collapsed, setCollapsed] = useState(false);
  const panel = useRef<HTMLElement>(null);
  useEffect(() => {
    const desktop = matchMedia("(min-width: 768px)");
    const close = () => { if (desktop.matches) setDrawer(false); };
    desktop.addEventListener("change", close);
    return () => desktop.removeEventListener("change", close);
  }, [setDrawer]);
  useEffect(() => {
    if (!drawer) return;
    const previous = document.activeElement;
    const controls = () => [...(panel.current?.querySelectorAll<HTMLElement>("button:not(:disabled), a[href]") || [])]
      .filter(element => element.getClientRects().length > 0);
    const main = document.querySelector("main");
    if (main) main.inert = true;
    controls()[0]?.focus();
    function key(event: KeyboardEvent) {
      if (event.key === "Escape") setDrawer(false);
      if (event.key !== "Tab") return;
      const items = controls(); const first = items[0]; const last = items.at(-1);
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
      if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    }
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("keydown", key);
      if (main) main.inert = false;
      if (previous instanceof HTMLElement) previous.focus();
    };
  }, [drawer, setDrawer]);
  return <>
    {drawer && <button className={styles.backdrop} aria-label={chatText.close} onClick={() => setDrawer(false)} />}
    <aside ref={panel} className={cn(styles.sidebar, drawer && styles.open, collapsed && styles.collapsed)}
      role={drawer ? "dialog" : undefined} aria-modal={drawer || undefined} aria-label={chatText.history}>
      <div className={styles.collapse}><IconButton aria-label={collapsed ? chatText.expandSidebar : chatText.collapseSidebar} onClick={() => setCollapsed(!collapsed)}>{collapsed ? <PanelLeftOpen size={18} /> : <PanelLeftClose size={18} />}</IconButton></div>
      <div className={styles.brand}><Layers size={24} aria-hidden="true" /><strong>{chatText.brand}</strong><span>{chatText.beta}</span>
        <div className={styles.close}><IconButton aria-label={chatText.close} onClick={() => setDrawer(false)}><X size={18} /></IconButton></div></div>
      <Button disabled={streaming} onClick={reset}><Plus size={18} aria-hidden="true" />{chatText.newChat}</Button>
      {openDevChat && <Button disabled={streaming} onClick={() => { setDrawer(false); openDevChat(); }}><Hammer size={18} aria-hidden="true" />{settingsText.dev.devChat}</Button>}
      <h2>{chatText.history}</h2>
      <nav className={styles.history} aria-label={chatText.history}>
        {loading && <Skeleton />}{!loading && !chats.length && <p>{chatText.empty}</p>}
        {chats.map((chat) => <button key={chat.id} disabled={streaming} aria-current={chat.id === id ? "page" : undefined}
          onClick={() => void load(chat.id)}><MessageSquare size={15} aria-hidden="true" /><span>{chat.title || chatText.newChat}</span></button>)}
      </nav>
      <Button variant="ghost" onClick={() => { setDrawer(false); openSettings(); }}><Settings size={18} aria-hidden="true" />{settingsText.open}</Button>
      <ThemeControl />
    </aside>
  </>;
}
