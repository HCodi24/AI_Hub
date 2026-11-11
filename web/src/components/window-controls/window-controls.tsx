import { Minus, Square, X } from "lucide-react";
import styles from "./window-controls.module.css";
type TauriWindow = { minimize(): void; toggleMaximize(): void; close(): void };
const appWindow = () => (window as unknown as { __TAURI__?: { window: { getCurrentWindow(): TauriWindow } } }).__TAURI__?.window.getCurrentWindow();
/** Minimize / maximize / close in the top strip. Only exists inside the desktop app, which has no native title bar. */
export function WindowControls() {
  const win = typeof window === "undefined" ? undefined : appWindow();
  if (!win) return null;
  return <>
    <div className={styles.drag} data-tauri-drag-region onDoubleClick={() => win.toggleMaximize()} />
    <div className={styles.controls}>
      <button className={styles.min} aria-label="Minimize" onClick={() => win.minimize()}><Minus size={9} strokeWidth={3} /></button>
      <button className={styles.max} aria-label="Maximize" onClick={() => win.toggleMaximize()}><Square size={7} strokeWidth={3} /></button>
      <button className={styles.close} aria-label="Close" onClick={() => win.close()}><X size={9} strokeWidth={3} /></button>
    </div>
  </>;
}
