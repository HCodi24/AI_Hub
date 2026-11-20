import { useEffect, useRef, useState, type CSSProperties, type FormEvent } from "react";
import { PanelRight, RotateCw, X } from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { devApi } from "@/apis/dev.api";
import { Button } from "@/components/button/button";
import { IconButton } from "@/components/icon-button/icon-button";
import { notify } from "@/components/toast/toast";
import { useChatStore } from "@/components/features/chat/chat-store";
import { previewText as t } from "@/text/preview";
import styles from "./preview-panel.module.css";

const OPEN_KEY = "ai-hub-preview-open";
const WIDTH_KEY = "ai-hub-preview-width";
const MIN_WIDTH = 320;
const DEFAULT_WIDTH = 560;
const KEY_STEP = 24;
const urlKey = (folder: string) => `ai-hub-preview-url:${folder}`;
// The assistant ends a turn with a line `preview: <url>` to point the panel at the screen it just edited.
const PREVIEW_LINE = /^preview:\s*(\S+)\s*$/im;

const read = (key: string) => { try { return localStorage.getItem(key) ?? ""; } catch { return ""; } };
const write = (key: string, value: string) => { try { localStorage.setItem(key, value); } catch { /* storage unavailable: keep it in memory only */ } };

/** An address the iframe may open: http(s) only, `host:port` and `/path` accepted as shorthand. */
export function safePreviewUrl(value: string): string | null {
  const text = value.trim();
  if (!text) return null;
  try {
    const url = new URL(/^[\w.-]+:\d+/.test(text) ? `http://${text}` : text, window.location.origin);
    return url.protocol === "http:" || url.protocol === "https:" ? url.href : null;
  } catch { return null; }
}

/** Dev mode only: whether the live preview server listens, with a button to start it or (if this app started it) stop it. */
function ServerStatus({ onUp }: { onUp: () => void }) {
  const client = useQueryClient();
  const dev = useQuery({ queryKey: ["dev-status"], queryFn: ({ signal }) => devApi.status(signal) });
  const server = useQuery({ queryKey: ["preview-server"], enabled: dev.data?.dev === true, queryFn: ({ signal }) => devApi.previewServer(signal), refetchInterval: 3000 });
  const run = useMutation({
    mutationFn: (action: "start" | "stop") => devApi.previewServerAction(action),
    onSuccess: data => client.setQueryData(["preview-server"], data),
    onError: error => notify(error instanceof Error ? error.message : t.invalid),
  });
  const running = server.data?.running === true;
  const seen = useRef(false);
  // The iframe was blank while the server was down: reload it the moment the server comes up.
  useEffect(() => { if (running && seen.current) onUp(); seen.current = true; }, [running]); // eslint-disable-line react-hooks/exhaustive-deps -- onUp is a stable reload
  if (!server.data) return null;
  const { port, owned } = server.data;
  return <div className={styles.server}>
    <span className={styles.dot} data-on={running || undefined} aria-hidden="true" />
    <span>{owned && !running ? t.starting : t.server(port, running)}</span>
    {!owned && running && <small>{t.leftover}</small>}
    <Button size="sm" disabled={run.isPending} onClick={() => run.mutate(owned || running ? "stop" : "start")}>{owned || running ? t.stop : t.start}</Button>
  </div>;
}

/** Live view of the page being edited: the dev server's own hot reload updates the iframe as the code changes. */
export function PreviewPanel() {
  const folder = useChatStore(s => s.settings.folder);
  const reply = useChatStore(s => [...s.messages].reverse().find(m => m.role === "assistant")?.content ?? "");
  const streaming = useChatStore(s => s.streaming);
  const ran = useRef(false);
  const [open, setOpen] = useState(false);
  const [url, setUrl] = useState("");
  const [draft, setDraft] = useState("");
  const [version, setVersion] = useState(0);
  const [width, setWidth] = useState(DEFAULT_WIDTH);
  const [containerWidth, setContainerWidth] = useState(1200);
  const [resizing, setResizing] = useState(false);
  const panel = useRef<HTMLElement>(null);
  const drag = useRef<{ x: number; width: number } | null>(null);
  // Same rule as the history sidebar, mirrored: at most two thirds of the window, never narrower than MIN_WIDTH.
  const maxWidth = Math.max(MIN_WIDTH, Math.floor(containerWidth * 2 / 3));
  const panelWidth = Math.max(MIN_WIDTH, Math.min(width, maxWidth));
  useEffect(() => {
    setOpen(read(OPEN_KEY) === "1");
    const saved = Number(read(WIDTH_KEY));
    if (saved > 0) setWidth(saved);
  }, []);
  useEffect(() => {
    const container = panel.current?.parentElement;
    if (!open || !container) return;
    const observer = new ResizeObserver(() => setContainerWidth(container.clientWidth));
    observer.observe(container);
    setContainerWidth(container.clientWidth);
    return () => observer.disconnect();
  }, [open]);
  useEffect(() => { const saved = read(urlKey(folder)); setUrl(saved); setDraft(saved); }, [folder]);
  // A reply that has just finished streaming and names a screen opens it. Replies loaded from history never do,
  // so a reload keeps the address the user typed.
  useEffect(() => {
    if (streaming) { ran.current = true; return; }
    if (!ran.current) return;
    ran.current = false;
    const target = PREVIEW_LINE.exec(reply)?.[1];
    if (!target || !safePreviewUrl(target)) return;
    setUrl(target); setDraft(target); setOpen(true); write(urlKey(folder), target); write(OPEN_KEY, "1");
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only the end of a run may move the panel
  }, [streaming]);
  const toggle = (next: boolean) => { setOpen(next); write(OPEN_KEY, next ? "1" : "0"); };
  const resize = (next: number) => { const value = Math.max(MIN_WIDTH, Math.min(maxWidth, next)); setWidth(value); write(WIDTH_KEY, String(value)); };
  const submit = (event: FormEvent) => { event.preventDefault(); setUrl(draft.trim()); write(urlKey(folder), draft.trim()); setVersion(v => v + 1); };
  const safe = safePreviewUrl(url);
  return <>
    {!open && <IconButton className={styles.toggle} aria-label={t.toggle} onClick={() => toggle(true)}><PanelRight size={20} /></IconButton>}
    {open && <aside ref={panel} className={styles.panel} aria-label={t.toggle} data-preview data-resizing={resizing || undefined}
      style={{ "--preview-width": `${panelWidth}px` } as CSSProperties}>
      <div className={styles.handle} role="separator" aria-orientation="vertical" aria-label={t.resize}
        aria-valuemin={MIN_WIDTH} aria-valuemax={maxWidth} aria-valuenow={panelWidth} tabIndex={0}
        onPointerDown={event => {
          if (event.button !== 0) return;
          event.preventDefault();
          event.currentTarget.setPointerCapture(event.pointerId);
          drag.current = { x: event.clientX, width: panelWidth };
          setResizing(true);
        }}
        onPointerMove={event => { if (drag.current) resize(drag.current.width - (event.clientX - drag.current.x)); }}
        onPointerUp={event => { if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId); }}
        onLostPointerCapture={() => { drag.current = null; setResizing(false); }}
        onKeyDown={event => {
          if (event.key === "ArrowLeft") { event.preventDefault(); resize(panelWidth + KEY_STEP); }
          else if (event.key === "ArrowRight") { event.preventDefault(); resize(panelWidth - KEY_STEP); }
        }} />
      <form className={styles.bar} onSubmit={submit}>
        <input aria-label={t.url} value={draft} placeholder={t.placeholder} onChange={event => setDraft(event.target.value)} spellCheck={false} />
        <IconButton type="submit" aria-label={t.reload}><RotateCw size={18} /></IconButton>
        <IconButton aria-label={t.close} onClick={() => toggle(false)}><X size={18} /></IconButton>
      </form>
      <ServerStatus onUp={() => setVersion(v => v + 1)} />
      {safe ? <iframe key={`${safe}|${version}`} className={styles.frame} src={safe} title={t.toggle}
        sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-modals" />
        : <p className={styles.note}>{url ? t.invalid : t.empty}</p>}
    </aside>}
  </>;
}
