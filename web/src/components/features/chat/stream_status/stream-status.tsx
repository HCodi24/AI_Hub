import type { ReactNode } from "react";
import styles from "./stream-status.module.css";

/** Compact elapsed time: 45s, 1m 05s, 1h 02m. */
export function formatElapsed(total: number): string {
  const s = Math.max(0, Math.floor(total));
  if (s < 60) return `${s}s`;
  const pad = (n: number) => String(n).padStart(2, "0");
  if (s < 3600) return `${Math.floor(s / 60)}m ${pad(s % 60)}s`;
  return `${Math.floor(s / 3600)}h ${pad(Math.floor(s / 60) % 60)}m`;
}

/** Live "Đang phản hồi…" line under a running reply or Tester card, with a blinking caret. */
export function StreamStatus({ children }: { children: ReactNode }) {
  return <div className={styles.streamStatus} role="status" aria-live="polite" aria-atomic="true">{children}</div>;
}
