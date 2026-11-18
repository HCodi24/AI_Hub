import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type MouseEvent } from "react";
import Head from "next/head";
import { useShallow } from "zustand/react/shallow";
import { MessageList } from "./message-list";
import { TaskBoard } from "./task-board";
import { TokenDashboard } from "./token-dashboard";
import { Sidebar } from "@/components/sidebar/sidebar";
import { Spinner } from "@/components/spinner/spinner";
import { Composer } from "@/components/features/composer/composer";
import { ChatHeader } from "@/components/features/composer/chat-toolbar";
import { ACTIVE_CHAT_KEY, useChatStore } from "./chat-store";
import { useChat } from "./use-chat";
import { useDashboardPreference } from "./use-dashboard-preference";
import { useTaskBoardPreference } from "./use-task-board-preference";
import { chatText } from "@/text/chat";
import styles from "./chat-screen.module.css";
import { Toast, useToast } from "@/components/toast/toast";
import { SettingsDialog } from "@/components/features/settings/settings-dialog";
import { DashboardDialog } from "./dashboard-dialog";
import { DevBanner, RESUME_KEY } from "@/components/features/dev/dev-banner";
import { openDevChat } from "@/components/features/dev/dev-chat";
import { useQuery } from "@tanstack/react-query";
import { devApi } from "@/apis/dev.api";
// Descendants that own their own behaviour: clicks inside them never open the large Dashboard dialog.
const INTERACTIVE = "a, button, input, select, textarea, summary, details, label, [role='button'], [role='link'], [contenteditable='true'], [tabindex]:not([tabindex='-1'])";
export function ChatScreen() {
  const dropTarget = useRef<HTMLElement>(null);
  const frameRef = useRef<HTMLDivElement>(null);
  const dockRef = useRef<HTMLDivElement>(null);
  const composerSlotRef = useRef<HTMLDivElement>(null);
  const state = useChatStore(useShallow(s => ({ error: s.error, streaming: s.streaming, folder: s.settings.folder })));
  const [settingsOpen, setSettingsOpen] = useState(false);
  // Local only: opening the large Dashboard dialog never touches chat state or the dashboard preference.
  const [dashboardOpen, setDashboardOpen] = useState(false);
  // Persisted in localStorage (shared across chats and tabs); the frame shows the cumulative chart of
  // the current chat, while the dialog adds the totals, tables, notes and backend history.
  const [dashboardEnabled, setDashboardEnabled] = useDashboardPreference();
  const [taskBoardEnabled, setTaskBoardEnabled] = useTaskBoardPreference();
  const { config, chats, send, stop, load, remove } = useChat();
  const dev = useQuery({ queryKey: ["dev-status"], queryFn: ({ signal }) => devApi.status(signal) });
  useLayoutEffect(() => {
    const frame = frameRef.current;
    const dock = dockRef.current;
    const composerSlot = composerSlotRef.current;
    const composer = composerSlot?.querySelector("form");
    if (!frame || !dock || !composerSlot || !composer) return;

    let overflowFrame = 0;
    const updateDock = () => {
      frame.style.setProperty("--chat-dock-height", `${dock.getBoundingClientRect().height}px`);
      cancelAnimationFrame(overflowFrame);
      overflowFrame = requestAnimationFrame(() => {
        composerSlot.toggleAttribute("data-overflow", composer.offsetHeight > composerSlot.clientHeight);
      });
    };
    updateDock();
    const observer = new ResizeObserver(updateDock);
    observer.observe(dock, { box: "border-box" });
    observer.observe(composerSlot);
    observer.observe(composer, { box: "border-box" });
    return () => {
      observer.disconnect();
      cancelAnimationFrame(overflowFrame);
    };
  }, []);
  // Restore the chat that was open before the page reload.
  useEffect(() => {
    let id: string | null = null;
    try {
      id = sessionStorage.getItem(RESUME_KEY) || sessionStorage.getItem(ACTIVE_CHAT_KEY);
      sessionStorage.removeItem(RESUME_KEY);
    } catch { /* storage unavailable */ }
    if (id) void load(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- run once on mount
  }, []);
  const queryError = config.error || chats.error;
  // Clicks on the frame itself (chart, legend, padding) open the large Dashboard dialog. Nested
  // interactive descendants keep their own behaviour: details/summary, buttons, links and inputs never open it.
  const onDashboardClick = (event: MouseEvent<HTMLElement>) => {
    // Text nodes and other non-element targets resolve through their parent element, so the chart title,
    // SVG paths/circles, the legend text and the frame padding all reach the frame itself.
    const target = event.target;
    const node = target instanceof Element ? target : target instanceof Node ? target.parentElement : null;
    // Only descendants of the frame can exempt the click; the frame itself (text, SVG, padding) opens it.
    const hit = node && event.currentTarget.contains(node) ? node.closest(INTERACTIVE) : null;
    if (hit && hit !== event.currentTarget) return;
    setDashboardOpen(true);
  };
  // Enter/Space only when the frame itself is the event target, so nested controls keep their keys.
  const onDashboardKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.target !== event.currentTarget) return;
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    setDashboardOpen(true);
  };
  return <><Head><title>{chatText.brand}</title><meta name="viewport" content="width=device-width, initial-scale=1" /></Head>
    <a className={styles.skip} href="#chat-main">{chatText.skip}</a>
    <div className={styles.shell}><Sidebar chats={chats.data || []} loading={chats.isPending} load={load} remove={remove} openSettings={() => setSettingsOpen(true)}
      openDevChat={dev.data?.dev ? () => void openDevChat(config.data?.providers || []) : undefined} />
      <main ref={dropTarget} id="chat-main" className={styles.main}>
        <DevBanner />
        <ChatHeader />
        {config.isPending && <Spinner />}
        {queryError && <div role="alert" className={styles.error}>{queryError.message}
          <button onClick={() => { void config.refetch(); void chats.refetch(); }}>{chatText.retry}</button></div>}
        {state.error && <div role="alert" className={styles.error}>{state.error}</div>}
        <div ref={frameRef} className={styles.chatFrame}>
          <div className={styles.workspace} data-dashboard={dashboardEnabled ? "on" : undefined}>
            {dashboardEnabled && <aside className={styles.dashboard} aria-label={chatText.dashboard.open} tabIndex={0} aria-haspopup="dialog" onClick={onDashboardClick} onKeyDown={onDashboardKeyDown}><TokenDashboard compact /></aside>}
            <section className={styles.messages} aria-label={chatText.history} aria-busy={state.streaming}>
              <MessageList send={send} taskBoardEnabled={taskBoardEnabled} />
            </section>
          </div>
          <div ref={dockRef} className={styles.dock}>
            <div className={styles.dockBoard}><TaskBoard enabled={taskBoardEnabled} /></div>
            <div ref={composerSlotRef} className={styles.dockComposer}>
              <Composer dropTarget={dropTarget} config={config.data} streaming={state.streaming} send={send} stop={stop} />
            </div>
          </div>
        </div>
      </main>
    </div>{settingsOpen && <SettingsDialog folder={state.folder} close={() => setSettingsOpen(false)}
      dashboardEnabled={dashboardEnabled} onDashboardEnabledChange={setDashboardEnabled}
      taskBoardEnabled={taskBoardEnabled} onTaskBoardEnabledChange={setTaskBoardEnabled} />}
    {dashboardOpen && <DashboardDialog close={() => setDashboardOpen(false)} />}<Toast /></>;
}
