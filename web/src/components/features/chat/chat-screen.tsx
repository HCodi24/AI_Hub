import { useEffect } from "react";
import Head from "next/head";
import { useShallow } from "zustand/react/shallow";
import { MessageList } from "./message-list";
import { Sidebar } from "@/components/sidebar/sidebar";
import { Spinner } from "@/components/spinner/spinner";
import { Composer } from "@/components/features/composer/composer";
import { ChatToolbar } from "./chat-toolbar";
import { useChatStore } from "./chat-store";
import { useChat } from "./use-chat";
import { chatText } from "@/text/chat";
import styles from "./chat-screen.module.css";
import { Toast, useToast } from "@/components/toast/toast";
import { setRunnerMemoryHandler } from "@/components/features/plan/plan-runner";
import { memoryNotice } from "@/components/features/memory/memory-events";
export function ChatScreen() {
  useEffect(() => { setRunnerMemoryHandler((raw, folder) => useToast.getState().show(memoryNotice(raw, folder))); }, []);
  const state = useChatStore(useShallow(s => ({ error: s.error, streaming: s.streaming, status: s.status })));
  const { config, chats, send, stop, load } = useChat();
  const queryError = config.error || chats.error;
  return <><Head><title>{chatText.brand}</title><meta name="viewport" content="width=device-width, initial-scale=1" /></Head>
    <a className={styles.skip} href="#chat-main">{chatText.skip}</a>
    <div className={styles.shell}><Sidebar chats={chats.data || []} loading={chats.isPending} load={load} />
      <main id="chat-main" className={styles.main}>
        <ChatToolbar config={config.data} />
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
        <Composer streaming={state.streaming} send={send} stop={stop} />
      </main>
    </div><Toast /></>;
}
