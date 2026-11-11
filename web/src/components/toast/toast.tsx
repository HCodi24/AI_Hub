import { X } from "lucide-react";
import { create } from "zustand";
import { cardText as t } from "@/text/cards";
import styles from "./toast.module.css";
type Notice = { text: string; action?: () => Promise<void> };
export const useToast = create<{ notice: Notice | null; show: (notice: Notice) => void }>(set => ({ notice: null, show: notice => set({ notice }) }));
export function notify(text: string) { useToast.getState().show({ text }); }
export function Toast() {
  const { notice } = useToast();
  if (!notice) return null;
  return <aside className={styles.toast} role="status"><span>{notice.text}</span>
    {notice.action && <button onClick={() => { void notice.action?.().catch(e => notify(String(e))); }}>{t.undo}</button>}
    <button aria-label={t.close} onClick={() => useToast.setState({ notice: null })}><X size={16} aria-hidden="true" /></button></aside>;
}
