import { useEffect, useRef, useState } from "react";
import { Maximize2, Minimize2, Plus, Terminal as TerminalIcon, X } from "lucide-react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { IconButton } from "@/components/icon-button/icon-button";
import { useChatStore } from "@/components/features/chat/chat-store";
import { terminalText as t } from "@/text/terminal";
import styles from "./terminal-panel.module.css";

const OPEN_KEY = "ai-hub-terminal-open";
const read = (key: string) => { try { return localStorage.getItem(key) ?? ""; } catch { return ""; } };
const write = (key: string, value: string) => { try { localStorage.setItem(key, value); } catch { /* storage unavailable: keep it in memory only */ } };

// The web dev server (port 7880) cannot proxy a WebSocket, so it talks to the app's own port directly.
const socketUrl = (folder: string) => {
  const host = window.location.port === "7880" ? "127.0.0.1:7878" : window.location.host;
  return `ws://${host}/api/terminal?folder=${encodeURIComponent(folder)}`;
};

/** One shell: xterm in the page, a pseudo-terminal in the app, joined by a WebSocket that lives and dies with this component. */
function TerminalView({ folder }: { folder: string }) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const css = getComputedStyle(document.documentElement);
    const rawBg = css.getPropertyValue("--bg").trim();
    const bg = !rawBg || rawBg === "transparent"
      ? (css.getPropertyValue("--glass-terminal-bg").trim() || "#141822")
      : rawBg;
    const term = new Terminal({
      cursorBlink: true, fontSize: 13, fontFamily: '"JetBrains Mono Variable", ui-monospace, monospace',
      theme: { background: bg, foreground: css.getPropertyValue("--text").trim() || "#e5e5e5" },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(element);
    const socket = new WebSocket(socketUrl(folder));
    socket.binaryType = "arraybuffer";
    const encoder = new TextEncoder();
    const sendSize = () => { fit.fit(); if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify({ cols: term.cols, rows: term.rows })); };
    socket.onopen = sendSize;
    socket.onmessage = event => term.write(new Uint8Array(event.data as ArrayBuffer));
    socket.onclose = () => term.write(t.ended);
    const input = term.onData(data => { if (socket.readyState === WebSocket.OPEN) socket.send(encoder.encode(data)); });
    const observer = new ResizeObserver(sendSize);
    observer.observe(element);
    term.focus();
    return () => { observer.disconnect(); input.dispose(); socket.close(); term.dispose(); };
  }, [folder]);
  return <div ref={host} className={styles.view} />;
}

/** A shell at the bottom of the chat, opened in the chat's folder. Ctrl+` toggles it. */
export function TerminalPanel() {
  const folder = useChatStore(s => s.settings.folder);
  const [open, setOpen] = useState(false);
  const [big, setBig] = useState(false);
  const [session, setSession] = useState(0);
  useEffect(() => { setOpen(read(OPEN_KEY) === "1"); }, []);
  const toggle = (next: boolean) => { setOpen(next); write(OPEN_KEY, next ? "1" : "0"); };
  useEffect(() => {
    const listen = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.key === "`") { event.preventDefault(); toggle(!open); }
    };
    window.addEventListener("keydown", listen);
    return () => window.removeEventListener("keydown", listen);
  }, [open]);
  return <>
    <IconButton data-glass="terminal-toggle" className={styles.toggle} aria-label={open ? t.close : t.toggle} aria-expanded={open} onClick={() => toggle(!open)}><TerminalIcon size={20} /></IconButton>
    {open && <section data-glass="terminal-panel" className={styles.panel} data-big={big || undefined} aria-label={t.title}>
      <header className={styles.bar}>
        <span className={styles.title}>{t.title}</span>
        <IconButton size="sm" aria-label={t.restart} onClick={() => setSession(s => s + 1)}><Plus size={18} /></IconButton>
        <span className={styles.spacer} />
        <IconButton size="sm" aria-label={big ? t.restore : t.maximize} onClick={() => setBig(!big)}>{big ? <Minimize2 size={16} /> : <Maximize2 size={16} />}</IconButton>
        <IconButton size="sm" aria-label={t.close} onClick={() => toggle(false)}><X size={18} /></IconButton>
      </header>
      <TerminalView key={`${folder}|${session}`} folder={folder} />
    </section>}
  </>;
}
