import { useEffect, useRef, useState } from "react";
import { PenSquare, Home, History, Search, X, PanelLeftClose, PanelLeftOpen, Settings, Hammer, ChevronDown, ChevronRight, Folder } from "lucide-react";
import { IconButton } from "@/components/icon-button/icon-button";
import { useChatStore } from "@/components/features/chat/chat-store";
import { Skeleton } from "@/components/skeleton/skeleton";
import { WindowControls } from "@/components/window-controls/window-controls";
import { ThemeControl } from "@/components/theme-control/theme-control";
import { useShallow } from "zustand/react/shallow";
import type { ChatSummary } from "@/schema/chat.schema";
import { chatText } from "@/text/chat";
import { settingsText } from "@/text/settings";
import { cn, folderName } from "@/lib/utils";
import styles from "./sidebar.module.css";
// Served from web/public/brand; the app lives under basePath "/app" (next.config.js).
// Chats arrive newest first, so groups keep first-seen order: the most recently used project is on top.
function groupByFolder(chats: ChatSummary[]) {
  const groups = new Map<string, ChatSummary[]>();
  for (const chat of chats) groups.set(chat.folder, [...(groups.get(chat.folder) || []), chat]);
  return [...groups];
}
const LOGO = "/app/brand/mutis-logo.svg";
const LOGO_DARK = "/app/brand/mutis-logo-dark.svg";
export function Sidebar({ chats, loading, load, openSettings, openDevChat }: { chats: ChatSummary[]; loading: boolean; load: (id: string) => Promise<void>; openSettings: () => void; openDevChat?: () => void }) {
  const { id, devChat, drawer, streaming, reset, setDrawer } = useChatStore(useShallow(s => ({ id: s.id, devChat: s.settings.dev_chat, drawer: s.drawer, streaming: s.streaming, reset: s.reset, setDrawer: s.setDrawer })));
  const [collapsed, setCollapsed] = useState(false);
  const [searching, setSearching] = useState(false);
  const [query, setQuery] = useState("");
  const [closed, setClosed] = useState<Set<string>>(new Set());
  const panel = useRef<HTMLElement>(null);
  const preferences = useRef<HTMLDetailsElement>(null);
  const history = useRef<HTMLElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const filteredChats = chats.filter(chat => chat.title.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));
  useEffect(() => {
    function closePreferences(event: PointerEvent | KeyboardEvent) {
      if (!preferences.current?.open) return;
      if (event instanceof KeyboardEvent) {
        if (event.key !== "Escape") return;
        event.preventDefault();
        event.stopImmediatePropagation();
        preferences.current.querySelector("summary")?.focus();
      } else if (event.target instanceof Node && preferences.current.contains(event.target)) return;
      preferences.current.open = false;
    }
    document.addEventListener("pointerdown", closePreferences);
    document.addEventListener("keydown", closePreferences, true);
    return () => {
      document.removeEventListener("pointerdown", closePreferences);
      document.removeEventListener("keydown", closePreferences, true);
    };
  }, []);
  useEffect(() => { if (searching) search.current?.focus(); }, [searching]);
  useEffect(() => {
    const desktop = matchMedia("(min-width: 768px)");
    const close = () => { if (desktop.matches) setDrawer(false); };
    desktop.addEventListener("change", close);
    return () => desktop.removeEventListener("change", close);
  }, [setDrawer]);
  useEffect(() => {
    if (!drawer) return;
    const previous = document.activeElement;
    const controls = () => [...(panel.current?.querySelectorAll<HTMLElement>("button:not(:disabled), a[href], summary, input") || [])]
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
      <div className={styles.menuLabels}><span>File</span><span>Edit</span><span>View</span></div>
      <WindowControls />
      <div className={styles.rail}>
        <div className={styles.collapse}><IconButton className={styles.collapseButton} aria-label={collapsed ? chatText.expandSidebar : chatText.collapseSidebar} aria-expanded={!collapsed} aria-controls="sidebar-content" onClick={() => setCollapsed(!collapsed)}>{collapsed ? <PanelLeftOpen size={16} /> : <PanelLeftClose size={16} />}</IconButton></div>
        <nav className={styles.railNav} aria-label={chatText.navigation}>
          <IconButton className={cn(styles.railButton, !id && !devChat && styles.active)} aria-label={chatText.home} disabled={streaming} onClick={reset}><Home size={23} /></IconButton>
          <IconButton className={styles.railButton} aria-label={chatText.history} aria-controls="sidebar-history" onClick={() => { setCollapsed(false); requestAnimationFrame(() => history.current?.focus()); }}><History size={23} /></IconButton>
          {openDevChat && <IconButton className={cn(styles.railButton, devChat && styles.active)} aria-label={settingsText.dev.devChat} aria-pressed={devChat} disabled={streaming} onClick={() => { setDrawer(false); openDevChat(); }}><Hammer size={23} /></IconButton>}
        </nav>
        <div className={styles.railBottom}>
          <details ref={preferences} className={styles.preferences}>
            <summary className={styles.profile} aria-label={chatText.workspaceMenu} title={chatText.workspaceMenu}><img className={styles.avatar} src={LOGO_DARK} alt="" aria-hidden="true" /></summary>
            <div className={styles.preferencesPanel}>
              <div className={styles.identity}><img className={styles.avatar} src={LOGO_DARK} alt="" aria-hidden="true" /><div><strong>{chatText.brand}</strong><span>{chatText.local}</span></div></div>
              <button className={styles.settings} onClick={() => { if (preferences.current) preferences.current.open = false; setDrawer(false); openSettings(); }}><Settings size={20} aria-hidden="true" />{settingsText.open}</button>
              <div className={styles.appearance}><span>{chatText.appearance}</span><ThemeControl /></div>
            </div>
          </details>
        </div>
      </div>
      <div id="sidebar-content" className={styles.content}>
        <div className={styles.brand}><img className={styles.logo} src={LOGO} alt="" aria-hidden="true" /><strong>{chatText.brand}</strong>
          <IconButton aria-label={chatText.searchChats} aria-expanded={searching} aria-controls="sidebar-search" onClick={() => { setSearching(!searching); setQuery(""); }}><Search size={20} /></IconButton>
          <div className={styles.close}><IconButton aria-label={chatText.close} onClick={() => setDrawer(false)}><X size={20} /></IconButton></div>
        </div>
        <button className={styles.newChat} disabled={streaming} onClick={reset}><PenSquare size={21} aria-hidden="true" />{chatText.newChat}</button>
        {searching && <input ref={search} id="sidebar-search" className={styles.search} type="search" aria-label={chatText.searchChats} placeholder={chatText.searchChats} value={query} onChange={event => setQuery(event.target.value)} onKeyDown={event => { if (event.key === "Escape") { event.stopPropagation(); setQuery(""); setSearching(false); panel.current?.querySelector<HTMLButtonElement>('[aria-controls="sidebar-search"]')?.focus(); } }} />}
        <h2>{chatText.recents}</h2>
        <nav ref={history} id="sidebar-history" tabIndex={-1} className={styles.history} aria-label={chatText.history}>
          {loading && <Skeleton />}{!loading && !filteredChats.length && <p>{query.trim() ? chatText.noChatsFound : chatText.empty}</p>}
          {groupByFolder(filteredChats).map(([folder, items]) => {
            const open = Boolean(query.trim()) || !closed.has(folder);
            return <section key={folder} className={styles.group}>
              <button className={styles.groupHead} aria-expanded={open} title={folder || chatText.noFolder}
                onClick={() => setClosed(prev => { const next = new Set(prev); if (!next.delete(folder)) next.add(folder); return next; })}>
                {open ? <ChevronDown size={16} aria-hidden="true" /> : <ChevronRight size={16} aria-hidden="true" />}
                <Folder size={16} aria-hidden="true" /><span>{folder ? folderName(folder) : chatText.noFolder}</span></button>
              {open && items.map((chat) => <button key={chat.id} className={styles.chatItem} disabled={streaming} aria-current={chat.id === id ? "page" : undefined} title={chat.title || chatText.newChat}
                onClick={() => void load(chat.id)}><span>{chat.title || chatText.newChat}</span></button>)}
            </section>;
          })}
        </nav>
      </div>
    </aside>
  </>;
}
