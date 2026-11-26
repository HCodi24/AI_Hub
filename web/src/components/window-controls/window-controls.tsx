import styles from "./window-controls.module.css";
type TauriWindow = { minimize(): void; toggleMaximize(): void; close(): void };
const appWindow = () => (window as unknown as { __TAURI__?: { window: { getCurrentWindow(): TauriWindow } } }).__TAURI__?.window.getCurrentWindow();

function MinimizeIcon() {
  return <svg width="8" height="8" viewBox="0 0 12 12" aria-hidden="true">
    <rect x="2" y="5" width="8" height="2" rx="1" fill="currentColor" />
  </svg>;
}

function MaximizeIcon() {
  return <svg width="8" height="8" viewBox="0 0 12 12" aria-hidden="true">
    <path d="M 2.2 3.3 Q 2.2 2.2 3.3 2.2 L 6.8 2.2 Q 7.8 2.2 7.1 2.9 L 2.9 7.1 Q 2.2 7.8 2.2 6.8 Z" fill="currentColor" />
    <path d="M 9.8 8.7 Q 9.8 9.8 8.7 9.8 L 5.2 9.8 Q 4.2 9.8 4.9 9.1 L 9.1 4.9 Q 9.8 4.2 9.8 5.2 Z" fill="currentColor" />
  </svg>;
}

function CloseIcon() {
  return <svg width="8" height="8" viewBox="0 0 12 12" fill="none" aria-hidden="true">
    <path d="M 3.1 3.1 L 8.9 8.9 M 8.9 3.1 L 3.1 8.9" stroke="currentColor" strokeWidth="2.1" strokeLinecap="round" />
  </svg>;
}

/** Minimize / maximize / close in the top strip. Only exists inside the desktop app, which has no native title bar. */
export function WindowControls() {
  const win = typeof window === "undefined" ? undefined : appWindow();
  if (!win) return null;
  return <>
    <div className={styles.drag} data-tauri-drag-region onDoubleClick={() => win.toggleMaximize()} />
    <div className={styles.controls} data-glass="window-controls">
      <button type="button" className={styles.min} data-action="min" aria-label="Minimize" onClick={() => win.minimize()}><MinimizeIcon /></button>
      <button type="button" className={styles.max} data-action="max" aria-label="Maximize" onClick={() => win.toggleMaximize()}><MaximizeIcon /></button>
      <button type="button" className={styles.close} data-action="close" aria-label="Close" onClick={() => win.close()}><CloseIcon /></button>
    </div>
  </>;
}
