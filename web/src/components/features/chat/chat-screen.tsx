import { useEffect, useState } from "react";
import Head from "next/head";
import { useShallow } from "zustand/react/shallow";
import { MessageList } from "./message-list";
import { Sidebar } from "@/components/sidebar/sidebar";
import { Spinner } from "@/components/spinner/spinner";
import { Composer } from "@/components/features/composer/composer";
import { ChatHeader } from "@/components/features/composer/chat-toolbar";
import { ACTIVE_CHAT_KEY, useChatStore } from "./chat-store";
import { useChat } from "./use-chat";
import { chatText } from "@/text/chat";
import styles from "./chat-screen.module.css";
import { Toast, useToast } from "@/components/toast/toast";
import { setRunnerMemoryHandler } from "@/components/features/plan/plan-runner";
import { memoryNotice } from "@/components/features/memory/memory-events";
import { SettingsDialog } from "@/components/features/settings/settings-dialog";
import { DevBanner, RESUME_KEY } from "@/components/features/dev/dev-banner";
import { openDevChat } from "@/components/features/dev/dev-chat";
import { useQuery } from "@tanstack/react-query";
import { devApi } from "@/apis/dev.api";
export function ChatScreen() {
  useEffect(() => { setRunnerMemoryHandler((raw, folder) => useToast.getState().show(memoryNotice(raw, folder))); }, []);
  const state = useChatStore(useShallow(s => ({ error: s.error, streaming: s.streaming, status: s.status, folder: s.settings.folder })));
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { config, chats, send, stop, load } = useChat();
  const dev = useQuery({ queryKey: ["dev-status"], queryFn: ({ signal }) => devApi.status(signal) });
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
  return <><Head><title>{chatText.brand}</title><meta name="viewport" content="width=device-width, initial-scale=1" /></Head>
    <a className={styles.skip} href="#chat-main">{chatText.skip}</a>
    <div className={styles.shell}><Sidebar chats={chats.data || []} loading={chats.isPending} load={load} openSettings={() => setSettingsOpen(true)}
      openDevChat={dev.data?.dev ? () => void openDevChat(config.data?.providers || []) : undefined} />
      <main id="chat-main" className={styles.main}>
        <DevBanner />
        <ChatHeader />
        {config.isPending && <Spinner />}
        {queryError && <div role="alert" className={styles.error}>{queryError.message}
          <button onClick={() => { void config.refetch(); void chats.refetch(); }}>{chatText.retry}</button></div>}
        {state.error && <div role="alert" className={styles.error}>{state.error}</div>}
        <section className={styles.messages} aria-label={chatText.history} aria-busy={state.streaming}>
          <MessageList />
        </section>
        <div className={styles.status} role="status" aria-live="polite" aria-atomic="true">
          {state.streaming && <>{chatText.streaming} {state.status.provider_name} {state.status.model}
            {state.status.elapsed_s !== undefined && ` · ${state.status.elapsed_s} ${chatText.seconds}`}
            {state.status.idle_s !== undefined && ` · ${chatText.idle}: ${state.status.idle_s} ${chatText.seconds}`}</>}
        </div>
        <Composer config={config.data} streaming={state.streaming} send={send} stop={stop} />
      </main>
    </div>{settingsOpen && <SettingsDialog folder={state.folder} close={() => setSettingsOpen(false)} />}<Toast /></>;
}
