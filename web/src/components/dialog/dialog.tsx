import { useEffect, useRef, type ReactNode } from "react";
import { X } from "lucide-react";
import { cardText as t } from "@/text/cards";
import styles from "./dialog.module.css";
export function Dialog({ title, close, children, wide, className = "", iconClose = false }: { title: string; close: () => void; children: ReactNode; wide?: boolean; className?: string; iconClose?: boolean }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => { const dialog = ref.current; const previous = document.activeElement;
    dialog?.showModal(); return () => { dialog?.close(); if (previous instanceof HTMLElement) previous.focus(); };
  }, []);
  return <dialog ref={ref} className={`${styles.dialog} ${wide ? styles.wide : ""} ${className}`} aria-label={title} onCancel={event => { event.preventDefault(); close(); }}>
    <header><h2>{title}</h2><button onClick={close} aria-label={t.close}>{iconClose ? <X size={22} aria-hidden="true" /> : t.close}</button></header>{children}</dialog>;
}
