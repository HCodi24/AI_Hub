import { useCallback, useEffect, useRef, useState, type CSSProperties, type FormEvent } from "react";
import { Minus, MousePointerClick, PanelRight, Play, Plus, RotateCw } from "lucide-react";
import { z } from "zod";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { devApi } from "@/apis/dev.api";
import { demoApi } from "@/apis/demo.api";
import { Button } from "@/components/button/button";
import { IconButton } from "@/components/icon-button/icon-button";
import { notify } from "@/components/toast/toast";
import { useChatStore } from "@/components/features/chat/chat-store";
import { http, parseResponse } from "@/lib/http";
import { previewText as t } from "@/text/preview";
import styles from "./preview-panel.module.css";

const OPEN_KEY = "ai-hub-preview-open";
const WIDTH_KEY = "ai-hub-preview-width";
const MIN_WIDTH = 320;
const CHAT_MIN_WIDTH = 360;
const DEFAULT_WIDTH = 560;
const KEY_STEP = 24;
const ZOOM_KEY = "ai-hub-preview-zoom";
const ZOOM_STEPS = [0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2];
const urlKey = (folder: string) => `ai-hub-preview-url:${folder}`;
// The assistant ends a turn with a line `preview: <url>` to point the panel at the screen it just edited.
const PREVIEW_LINE = /^preview:\s*(\S+)\s*$/im;

const PickedSchema = z.object({ url: z.string(), selector: z.string(), text: z.string(), css: z.record(z.string(), z.string()), html: z.string() });
const PreviewProxySchema = z.object({ port: z.number().int().min(1).max(65535) });
const RegionSchema = z.object({
  kind: z.literal("region"), url: z.string(), width: z.number(), height: z.number(), total: z.number(),
  items: z.array(z.object({
    selector: z.string(), text: z.string(), css: z.record(z.string(), z.string()), html: z.string(),
    at: z.object({ left: z.number(), top: z.number(), width: z.number(), height: z.number() }),
  })),
});

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

function demoSize(bytes: number) {
  const kb = bytes / 1024;
  if (kb >= 1024) return `${(kb / 1024).toFixed(1)} MB`;
  if (kb >= 1) return `${kb.toFixed(1)} KB`;
  return `${bytes} B`;
}

function ServerStatus({ address, folder, dev, onUp }: { address: URL | null; folder: string; dev: boolean; onUp: () => void }) {
  const client = useQueryClient();
  const hostname = address?.hostname;
  const host = hostname === "localhost" ? "localhost" : hostname === "127.0.0.1" ? "ipv4" : hostname === "[::1]" ? "ipv6" : null;
  const port = Number(address?.port || (address?.protocol === "https:" ? 443 : 80));
  const own = !address || (port === 7880 && (host === "localhost" || host === "ipv4"));
  const managed = own ? dev : host !== null && folder !== "";
  const server = useQuery({
    queryKey: own && managed ? ["preview-server"] : ["preview-server", host, port],
    enabled: managed || host !== null,
    queryFn: ({ signal }) => own && managed ? devApi.previewServer(signal) : devApi.localPreviewStatus(port, host ?? "localhost", signal),
    refetchInterval: 3000,
  });
  const run = useMutation({
    mutationFn: (action: "start" | "stop") => devApi.previewServerAction(action, own ? undefined : { port, folder }),
    onSuccess: data => client.setQueryData(own ? ["preview-server"] : ["preview-server", host, port], data),
    onError: error => notify(error instanceof Error ? error.message : t.invalid),
  });
  const running = server.isError ? undefined : server.data?.running;
  const previous = useRef<boolean | undefined>(undefined);
  useEffect(() => {
    if (running === true && previous.current === false) onUp();
    previous.current = running;
  }, [running, onUp]);
  if (!managed && !host) return null;
  const owned = server.data?.owned === true;
  const canStop = owned || running === true;
  const label = server.isError ? t.serverUnknown : running === undefined ? t.serverChecking
    : managed && owned && !running ? t.starting : running ? t.serverRunning : t.serverStopped;
  return <div className={styles.server}>
    <span className={styles.dot} data-on={running || undefined} aria-hidden="true" />
    <span role="status">{address ? `${hostname}:${port}` : t.server(7880)}
      {running ? <span className="sr-only"> · {label}</span> : ` · ${label}`}
    </span>
    <IconButton size="sm" aria-label={canStop ? t.stop : t.start} title={managed ? (canStop ? t.stop : t.start) : t.serverControlUnavailable}
      disabled={!managed || !server.data || run.isPending}
      onClick={() => run.mutate(canStop ? "stop" : "start")}>
      {canStop ? <span className={styles.stopIcon} data-running={running || undefined} /> : <Play size={16} fill="currentColor" />}
    </IconButton>
  </div>;
}

/** Preview of the page being edited. */
export function PreviewPanel() {
  const { data: devStatus } = useQuery({
    queryKey: ["dev-status"],
    queryFn: ({ signal }) => devApi.status(signal),
  });
  const dev = devStatus?.dev === true;
  const folder = useChatStore(s => s.settings.folder);
  const reply = useChatStore(s => [...s.messages].reverse().find(m => m.role === "assistant")?.content ?? "");
  const streaming = useChatStore(s => s.streaming);
  const client = useQueryClient();
  const { data: demoFiles } = useQuery({
    queryKey: ["demo-files", folder],
    queryFn: ({ signal }) => demoApi.list(folder, signal),
    enabled: Boolean(folder),
    refetchOnWindowFocus: false,
  });
  useEffect(() => {
    if (!folder) return;
    const refreshDemoFiles = () => {
      void client.invalidateQueries({ queryKey: ["demo-files", folder], exact: true });
    };
    window.addEventListener("focus", refreshDemoFiles);
    return () => window.removeEventListener("focus", refreshDemoFiles);
  }, [client, folder]);
  useEffect(() => useChatStore.subscribe((state, previous) => {
    const finishedFolder = previous.settings.folder;
    if (previous.streaming && !state.streaming && finishedFolder) {
      void client.invalidateQueries({ queryKey: ["demo-files", finishedFolder], exact: true });
    }
  }), [client]);
  const removeDemo = useMutation({
    mutationFn: ({ folder }: { folder: string; files: number }) => demoApi.remove(folder),
    onSuccess: async ({ skipped, ...remaining }, { folder, files }) => {
      const queryKey = ["demo-files", folder];
      await client.cancelQueries({ queryKey, exact: true });
      client.setQueryData(queryKey, remaining);
      const removed = t.demoRemoved(Math.max(0, files - remaining.files));
      notify(skipped.length ? `${removed}. ${t.demoSkipped(skipped)}` : removed);
    },
    onError: error => notify(error instanceof Error ? error.message : t.demoRemoveError),
  });
  const ran = useRef(false);
  const [open, setOpen] = useState(false);
  const [url, setUrl] = useState("");
  const [draft, setDraft] = useState("");
  const [version, setVersion] = useState(0);
  const [width, setWidth] = useState(DEFAULT_WIDTH);
  const [availableWidth, setAvailableWidth] = useState(DEFAULT_WIDTH + CHAT_MIN_WIDTH);
  const [resizing, setResizing] = useState(false);
  const panel = useRef<HTMLElement>(null);
  const frame = useRef<HTMLIFrameElement>(null);
  const [ready, setReady] = useState(false);
  const attachFrame = useCallback((element: HTMLIFrameElement | null) => {
    frame.current = element;
    setReady(false);
  }, []);
  const [picking, setPicking] = useState(false);
  const [zoom, setZoom] = useState(1);
  const drag = useRef<{ x: number; width: number } | null>(null);
  // Let the preview grow until the chat reaches its minimum width, while preserving the preview minimum.
  const maxWidth = Math.max(MIN_WIDTH, availableWidth - CHAT_MIN_WIDTH);
  const panelWidth = Math.max(MIN_WIDTH, Math.min(width, maxWidth));
  useEffect(() => {
    setOpen(read(OPEN_KEY) === "1");
    const saved = Number(read(WIDTH_KEY));
    if (saved > 0) setWidth(saved);
    const savedZoom = Number(read(ZOOM_KEY));
    if (ZOOM_STEPS.includes(savedZoom)) setZoom(savedZoom);
  }, []);
  useEffect(() => {
    const aside = panel.current;
    const container = aside?.parentElement;
    if (!open || !aside || !container) return;
    const main = container.querySelector<HTMLElement>("#chat-main") ?? container.querySelector<HTMLElement>(":scope > main");
    if (!main) return;
    const measure = () => setAvailableWidth(main.offsetWidth + aside.offsetWidth);
    const observer = new ResizeObserver(measure);
    observer.observe(container);
    observer.observe(main);
    observer.observe(aside);
    measure();
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
  const safe = safePreviewUrl(url);
  const address = safe ? new URL(safe) : null;
  const targetPort = address && ["localhost", "127.0.0.1", "[::1]"].includes(address.hostname)
    ? address.port || (address.protocol === "https:" ? "443" : "80")
    : null;
  const proxy = useQuery({
    queryKey: ["preview-proxy", address?.origin],
    enabled: open && targetPort !== null,
    retry: false,
    queryFn: async ({ signal }) => parseResponse(PreviewProxySchema, await http.get(`/preview-proxy/open/${targetPort}`, signal)),
  });
  const proxyPort = targetPort !== null && proxy.isSuccess ? proxy.data.port : null;
  const src = proxyPort !== null && address
    ? `${window.location.protocol}//${window.location.hostname}:${proxyPort}${address.pathname}${address.search}${address.hash}`
    : safe;
  const pickerOrigin = proxyPort !== null && src ? new URL(src).origin : null;
  const pickerReady = ready && pickerOrigin !== null;
  // Picker messages come from the proxied preview frame.
  useEffect(() => {
    const listen = (event: MessageEvent) => {
      if (event.source !== frame.current?.contentWindow || event.origin !== pickerOrigin || !event.data) return;
      if (event.data.type === "mutis-picker-ready") setReady(true);
      else if (event.data.type === "mutis-pick-end") setPicking(false);
      else if (event.data.type === "mutis-picked") {
        setPicking(false);
        const region = RegionSchema.safeParse(event.data.data);
        const parsed = PickedSchema.safeParse(event.data.data);
        const detail = region.success ? { label: t.regionLabel(region.data.items.length, region.data.width, region.data.height), text: t.pickedRegion(region.data) }
          : parsed.success ? { label: parsed.data.selector, text: t.picked(parsed.data) } : null;
        if (detail) window.dispatchEvent(new CustomEvent("mutis-preview-selection", { detail }));
      }
    };
    window.addEventListener("message", listen);
    return () => window.removeEventListener("message", listen);
  }, [pickerOrigin]);
  const pick = () => {
    const target = frame.current?.contentWindow;
    if (!target || !pickerOrigin) return;
    target.postMessage({ type: picking ? "mutis-pick-stop" : "mutis-pick-start" }, pickerOrigin);
    setPicking(!picking);
  };
  const zoomTo = (next: number) => { setZoom(next); write(ZOOM_KEY, String(next)); };
  const zoomBy = (direction: 1 | -1) => zoomTo(ZOOM_STEPS[Math.max(0, Math.min(ZOOM_STEPS.length - 1, ZOOM_STEPS.indexOf(zoom) + direction))]);
  const toggle = (next: boolean) => {
    setOpen(next);
    if (!next) setPicking(false);
    write(OPEN_KEY, next ? "1" : "0");
  };
  const resize = (next: number) => { const value = Math.max(MIN_WIDTH, Math.min(maxWidth, next)); setWidth(value); write(WIDTH_KEY, String(value)); };
  const submit = (event: FormEvent) => { event.preventDefault(); setUrl(draft.trim()); write(urlKey(folder), draft.trim()); setVersion(v => v + 1); };
  return <>
    <IconButton className={styles.toggle} aria-label={open ? t.close : t.toggle} aria-expanded={open} onClick={() => toggle(!open)}><PanelRight size={20} /></IconButton>
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
        <div className={styles.address}>
          <input aria-label={t.url} value={draft} placeholder={t.placeholder} onChange={event => setDraft(event.target.value)} spellCheck={false} />
          <IconButton type="submit" size="sm" className={styles.reload} aria-label={t.reload}><RotateCw size={18} /></IconButton>
        </div>
        <div className={styles.zoom}>
          <IconButton type="button" size="sm" aria-label={t.zoomOut} title={t.zoomOut} disabled={zoom <= ZOOM_STEPS[0]} onClick={() => zoomBy(-1)}><Minus size={16} /></IconButton>
          <button type="button" className={styles.zoomValue} aria-label={t.zoomReset} title={t.zoomReset} onClick={() => zoomTo(1)}>{Math.round(zoom * 100)}%</button>
          <IconButton type="button" size="sm" aria-label={t.zoomIn} title={t.zoomIn} disabled={zoom >= ZOOM_STEPS[ZOOM_STEPS.length - 1]} onClick={() => zoomBy(1)}><Plus size={16} /></IconButton>
        </div>
        <IconButton type="button" aria-label={t.pick} aria-pressed={picking} disabled={!pickerReady} title={pickerReady ? (picking ? t.picking : t.pick) : t.noPicker} onClick={pick}><MousePointerClick size={18} /></IconButton>
      </form>
      <ServerStatus key={address?.origin} address={address} folder={folder} dev={dev} onUp={() => setVersion(v => v + 1)} />
      {folder && demoFiles && demoFiles.files > 0 && <div className={styles.server}>
        <span title={demoFiles.dirs.map(dir => dir.path).join("\n")}>
          {t.demoFiles(demoFiles.files, demoSize(demoFiles.bytes))}
        </span>
        <Button size="sm" disabled={removeDemo.isPending}
          onClick={() => removeDemo.mutate({ folder, files: demoFiles.files })}>{t.removeDemo}</Button>
      </div>}
      {safe && src ? <div className={styles.frameViewport}>
        <iframe ref={attachFrame} key={`${src}|${version}`} onLoad={() => {
          setPicking(false);
          if (pickerOrigin) frame.current?.contentWindow?.postMessage({ type: "mutis-picker-ping" }, pickerOrigin);
        }} className={styles.frame} style={{ "--preview-zoom": zoom } as CSSProperties} src={src} title={t.toggle}
          sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-modals" />
      </div>
        : <p className={styles.note}>{url ? t.invalid : t.empty}</p>}
    </aside>}
  </>;
}
