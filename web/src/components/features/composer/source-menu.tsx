import { useState, type CSSProperties, type RefObject } from "react";
import { Bot } from "lucide-react";
import { useProviderConnections } from "@/lib/provider-connections";
import type { ProviderSummary } from "@/schema/config.schema";
import { chatText } from "@/text/chat";
import { cardText as t } from "@/text/cards";
import styles from "./source-menu.module.css";

type Props = {
  providers: ProviderSummary[]; mutisEnabled: boolean; mutisName?: string; value: string; title: string; disabled: boolean;
  open: boolean; setOpen: (open: boolean) => void; anchor: RefObject<HTMLElement | null>; onPick: (id: string) => void;
};

/** Button that opens a floating card above the popup listing only sources that pass the connection test. */
export function SourceMenu({ providers, mutisEnabled, mutisName, value, title, disabled, open, setOpen, anchor, onPick }: Props) {
  const [position, setPosition] = useState<CSSProperties>({});
  const statuses = useProviderConnections(providers);
  const connected = providers.filter(provider => statuses[provider.id]?.ok);
  const items = [...(mutisEnabled ? [{ id: "mutis", name: mutisName || t.mutis }] : []), ...connected];
  function toggle() {
    const box = anchor.current?.getBoundingClientRect();
    // shortcut: position is measured on open only; the popup closes on outside clicks, so scroll/resize drift is rare.
    if (box) setPosition({ left: box.left, bottom: window.innerHeight - box.top + 8, maxHeight: Math.max(120, box.top - 16) });
    setOpen(!open);
  }
  return <>
    <button type="button" className={styles.trigger} disabled={disabled} aria-label={chatText.source} title={title}
      aria-haspopup="listbox" aria-expanded={open} onClick={toggle}><Bot size={18} /></button>
    {open && <div className={styles.menu} style={position} role="listbox" aria-label={chatText.source}>
      {items.map((entry) => <button key={entry.id} type="button" role="option" aria-selected={value === entry.id} disabled={disabled}
        onClick={() => { onPick(entry.id); setOpen(false); }}>{entry.name}</button>)}
      {!items.length && <span className={styles.note}>{chatText.noConnectedSource}</span>}
    </div>}
  </>;
}
