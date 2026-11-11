import { CheckCircle2, Circle, Loader2, XCircle } from "lucide-react";
import styles from "./status-badge.module.css";
export function StatusBadge({ state, label }: { state: "running" | "done" | "failed" | "draft"; label: string }) {
  const Icon = state === "running" ? Loader2 : state === "done" ? CheckCircle2 : state === "failed" ? XCircle : Circle;
  return <span className={styles.badge} data-state={state}><Icon size={14} aria-hidden="true" className={state === "running" ? "spin" : undefined} />{label}</span>;
}
