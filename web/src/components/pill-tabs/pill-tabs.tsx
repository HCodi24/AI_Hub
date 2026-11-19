import type { KeyboardEvent } from "react";
import { cn } from "@/lib/utils";
import styles from "./pill-tabs.module.css";
export type PillTab<T extends string> = { id: T; label: string; count?: number };
type PillTabsProps<T extends string> = {
  tabs: PillTab<T>[];
  value: T;
  onChange: (id: T) => void;
  /** Prefix for tab/panel ids: tabs get `${idPrefix}-tab-${id}`, and point at `${idPrefix}-panel-${id}`. */
  idPrefix: string;
  label: string;
  /** id of a shared tabpanel; defaults to one panel per tab (`${idPrefix}-panel-${id}`). */
  controls?: string;
  className?: string;
};
/** Borderless pill tabs: the selected tab gets a soft filled background, with an optional muted count after the label. */
export function PillTabs<T extends string>({ tabs, value, onChange, idPrefix, label, controls, className }: PillTabsProps<T>) {
  function onKey(event: KeyboardEvent<HTMLButtonElement>) {
    const at = tabs.findIndex(tab => tab.id === value);
    const next = event.key === "ArrowRight" ? at + 1 : event.key === "ArrowLeft" ? at - 1
      : event.key === "Home" ? 0 : event.key === "End" ? tabs.length - 1 : null;
    if (next === null) return;
    event.preventDefault();
    const id = tabs[(next + tabs.length) % tabs.length].id;
    onChange(id);
    document.getElementById(`${idPrefix}-tab-${id}`)?.focus();
  }
  return <div className={cn(styles.tabs, className)} role="tablist" aria-label={label}>
    {tabs.map(tab => <button key={tab.id} type="button" role="tab" id={`${idPrefix}-tab-${tab.id}`} aria-controls={controls ?? `${idPrefix}-panel-${tab.id}`}
      aria-selected={tab.id === value} tabIndex={tab.id === value ? 0 : -1} className={styles.tab}
      onClick={() => onChange(tab.id)} onKeyDown={onKey}>
      <span>{tab.label}</span>
      {tab.count !== undefined && <span className={styles.count}>{tab.count}</span>}
    </button>)}
  </div>;
}
