import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type RefObject } from "react";
import { createPortal } from "react-dom";
import { useProviderConnections } from "@/lib/provider-connections";
import type { ProviderSummary } from "@/schema/config.schema";
import { chatText } from "@/text/chat";
import { cardText as t } from "@/text/cards";
import styles from "./source-menu.module.css";

type Props = {
  providers: ProviderSummary[]; mutisEnabled: boolean; mutisName?: string; value: string; title: string; disabled: boolean;
  open: boolean; setOpen: (open: boolean) => void; anchor: RefObject<HTMLElement | null>; menuRef?: RefObject<HTMLDivElement | null>; onPick: (id: string) => void;
};

/** Button that opens a floating card above the popup listing only sources that pass the connection test. */
export function SourceMenu({ providers, mutisEnabled, mutisName, value, title, disabled, open, setOpen, anchor, menuRef, onPick }: Props) {
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<CSSProperties>({});
  const statuses = useProviderConnections(providers);
  const connected = providers.filter(provider => statuses[provider.id]?.ok);
  const items = [...(mutisEnabled ? [{ id: "mutis", name: mutisName || t.mutis }] : []), ...connected];
  useLayoutEffect(() => {
    if (!open) return;
    function place() {
      const box = anchor.current?.getBoundingClientRect();
      if (!box) return;
      const width = Math.min(240, window.innerWidth * 0.9);
      setPosition({
        left: Math.max(12, Math.min(box.left, window.innerWidth - width - 12)),
        bottom: window.innerHeight - box.top + 8,
        maxHeight: Math.max(120, box.top - 16),
      });
    }
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => { window.removeEventListener("resize", place); window.removeEventListener("scroll", place, true); };
  }, [open, anchor]);
  useEffect(() => {
    if (!open) return;
    function closeOutside(event: PointerEvent) {
      const target = event.target as Node;
      if (!trigger.current?.contains(target) && !menu.current?.contains(target)) setOpen(false);
    }
    document.addEventListener("pointerdown", closeOutside);
    return () => document.removeEventListener("pointerdown", closeOutside);
  }, [open, setOpen]);
  function setMenuNode(node: HTMLDivElement | null) {
    menu.current = node;
    if (menuRef) menuRef.current = node;
  }
  return <>
    <button ref={trigger} type="button" className={styles.trigger} disabled={disabled} aria-label={chatText.source} title={title}
      aria-haspopup="listbox" aria-expanded={open} onClick={() => setOpen(!open)}>
      <img src="/app/brand/mutis-logo.svg" width={24} height={24} alt="" />
    </button>
    {open && typeof document !== "undefined" && createPortal(
      <div ref={setMenuNode} data-glass="overlay" className={styles.menu} style={position} role="listbox" aria-label={chatText.source}>
        {items.map((entry) => <button key={entry.id} type="button" role="option" aria-selected={value === entry.id} disabled={disabled}
          onClick={() => { onPick(entry.id); setOpen(false); }}>{entry.name}</button>)}
        {!items.length && <span className={styles.note}>{chatText.noConnectedSource}</span>}
      </div>,
      document.body,
    )}
  </>;
}
