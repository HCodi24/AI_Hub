import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Hammer, X } from "lucide-react";
import { devApi } from "@/apis/dev.api";
import { Button } from "@/components/button/button";
import { IconButton } from "@/components/icon-button/icon-button";
import { useChatStore } from "@/components/features/chat/chat-store";
import { settingsText as t } from "@/text/settings";
import styles from "./dev-banner.module.css";
export const RESUME_KEY = "ai-hub-resume-chat";
type Notice = { text: string; action?: "rebuild" | "web-build" };
const parse = (raw: string): Record<string, unknown> => { try { return JSON.parse(raw) as Record<string, unknown>; } catch { return {}; } };
// Remember the open chat so the page comes back to it after a dev reload instead of starting a new chat.
function reloadPage() {
  try { const id = useChatStore.getState().id; if (id) sessionStorage.setItem(RESUME_KEY, id); } catch { /* storage unavailable */ }
  location.reload();
}
/** Dev mode only: follows /api/dev/events, offers the builds, and reloads the page when the UI changed and no chat is running
 * (reloading mid-stream would cancel the AI's own run). */
export function DevBanner() {
  const status = useQuery({ queryKey: ["dev-status"], queryFn: ({ signal }) => devApi.status(signal) });
  const dev = status.data?.dev === true;
  const [notice, setNotice] = useState<Notice | null>(null); const [log, setLog] = useState(""); const [busy, setBusy] = useState(false);
  const reloadTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  useEffect(() => {
    if (!dev) return;
    function reloadWhenIdle() {
      if (!useChatStore.getState().streaming) { reloadPage(); return; }
      setNotice({ text: t.dev.reloadPending });
      if (reloadTimer.current) return;
      reloadTimer.current = setInterval(() => {
        if (useChatStore.getState().streaming) return;
        if (reloadTimer.current) clearInterval(reloadTimer.current);
        reloadTimer.current = null;
        setTimeout(reloadPage, 800);
      }, 1000);
    }
    const source = new EventSource("/api/dev/events");
    source.addEventListener("ui_changed", reloadWhenIdle);
    source.addEventListener("web_changed", () => setNotice({ text: t.dev.webChanged, action: "web-build" }));
    source.addEventListener("rust_changed", event => {
      const files = parse((event as MessageEvent<string>).data).files;
      setNotice({ text: t.dev.rustChanged(Array.isArray(files) ? files.length : 0), action: "rebuild" });
    });
    source.addEventListener("rebuild", event => {
      const result = parse((event as MessageEvent<string>).data);
      setNotice({ text: result.ok ? t.dev.restarting : `${t.dev.buildFailed}:\n${String(result.errors_tail ?? "")}` });
    });
    source.addEventListener("build_log", event => {
      const text = String(parse((event as MessageEvent<string>).data).text ?? "");
      setLog(current => (current + text).slice(-12000));
    });
    let stopped = false;
    source.onerror = () => {
      // The server is restarting after a rebuild: wait until it answers again, then reload.
      source.close(); setNotice({ text: t.dev.restarting });
      void (async () => {
        while (!stopped) {
          await new Promise(resolve => setTimeout(resolve, 1000));
          try { if ((await devApi.status()).dev) { reloadWhenIdle(); return; } } catch { /* still down */ }
        }
      })();
    };
    return () => { stopped = true; source.close(); if (reloadTimer.current) clearInterval(reloadTimer.current); };
  }, [dev]);
  if (!dev || !notice) return null;
  async function run(kind: "rebuild" | "web-build") {
    setBusy(true); setLog(""); setNotice({ text: kind === "rebuild" ? t.dev.running : `${t.dev.webBuild}…` });
    try {
      await devApi.run(kind, new AbortController().signal, (event, data) => {
        const value = typeof data === "object" && data !== null ? data as Record<string, unknown> : {};
        if (event === "log") setLog(current => `${current}${String(value.text ?? "")}\n`.slice(-12000));
        if (event === "rebuild") setNotice({ text: value.ok ? t.dev.restarting : `${t.dev.buildFailed}:\n${String(value.errors_tail ?? "")}` });
        if (event === "summary" && "web_ok" in value) setNotice({ text: value.web_ok ? t.dev.webOk : t.dev.webFail });
      });
    } catch (error) { setNotice({ text: error instanceof Error ? error.message : t.error }); } finally { setBusy(false); }
  }
  const action = notice.action;
  return <div className={styles.banner} role="status" aria-live="polite">
    <Hammer size={16} aria-hidden="true" /><span className={styles.text}>{notice.text}</span>
    {action && <Button size="sm" variant="primary" disabled={busy} onClick={() => void run(action)}>
      {action === "rebuild" ? t.dev.rebuild : t.dev.webBuild}</Button>}
    <IconButton aria-label={t.dev.dismiss} onClick={() => setNotice(null)}><X size={16} /></IconButton>
    {log && <pre className={styles.log}>{log}</pre>}
  </div>;
}
