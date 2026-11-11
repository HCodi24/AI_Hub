import { useEffect, useRef, type ReactNode } from "react";
import { cardText as t } from "@/text/cards";
import styles from "./dialog.module.css";
export function Dialog({ title, close, children, wide }: { title: string; close: () => void; children: ReactNode; wide?: boolean }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => { const dialog = ref.current; const previous = document.activeElement;
    dialog?.showModal(); return () => { dialog?.close(); if (previous instanceof HTMLElement) previous.focus(); };
  }, []);
  return <dialog ref={ref} className={wide ? `${styles.dialog} ${styles.wide}` : styles.dialog} aria-label={title} onCancel={event => { event.preventDefault(); close(); }}>
    <header><h2>{title}</h2><button onClick={close}>{t.close}</button></header>{children}</dialog>;
}
