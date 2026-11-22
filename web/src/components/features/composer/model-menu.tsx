import { useEffect, useId, useLayoutEffect, useRef, useState, type CSSProperties, type RefObject } from "react";
import { Check, ChevronRight } from "lucide-react";
import { chatText } from "@/text/chat";
import styles from "./model-menu.module.css";

function formatModelLabel(label: string) {
  return label.replace(/\bgpt\b/gi, "GPT").replace(/\b[a-z]/g, letter => letter.toUpperCase());
}

type Props = {
  models: string[]; value: string; label: string; disabled: boolean;
  open: boolean; setOpen: (open: boolean) => void;
  anchor: RefObject<HTMLElement | null>; onPick: (model: string) => void;
};

export function ModelMenu({ models, value, label, disabled, open, setOpen, anchor, onPick }: Props) {
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const id = useId();
  const [position, setPosition] = useState<CSSProperties>({});
  useLayoutEffect(() => {
    if (!open) return;
    function place() {
      const box = anchor.current?.getBoundingClientRect();
      if (!box) return;
      const width = Math.min(300, window.innerWidth - 24);
      setPosition({ width, left: Math.max(12, Math.min(box.left, window.innerWidth - width - 12)),
        bottom: window.innerHeight - box.top + 8, maxHeight: Math.max(0, box.top - 20) });
    }
    place();
    const selected = menu.current?.querySelector<HTMLButtonElement>('[aria-checked="true"]')
      || menu.current?.querySelector<HTMLButtonElement>("button");
    selected?.focus();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => { window.removeEventListener("resize", place); window.removeEventListener("scroll", place, true); };
  }, [open, anchor]);
  useEffect(() => {
    if (!open) return;
    function closeOutside(event: PointerEvent) {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    }
    document.addEventListener("pointerdown", closeOutside);
    return () => document.removeEventListener("pointerdown", closeOutside);
  }, [open, setOpen]);
  function close() { setOpen(false); trigger.current?.focus(); }
  return <div ref={root}>
    <button ref={trigger} type="button" className={styles.trigger} disabled={disabled} aria-haspopup="menu"
      aria-expanded={open} aria-controls={open ? id : undefined} onClick={() => setOpen(!open)}>
      <span>{formatModelLabel(label)}</span><ChevronRight size={14} aria-hidden="true" />
    </button>
    {open && <div ref={menu} id={id} className={styles.menu} style={position} role="menu" aria-label={chatText.selectModel}
      onKeyDown={event => {
        if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); close(); }
        if (event.key === "Tab") setOpen(false);
        if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
        event.preventDefault();
        const buttons = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>("button"));
        const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
        const next = event.key === "Home" ? 0 : event.key === "End" ? buttons.length - 1
          : (index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length;
        buttons[next]?.focus();
      }}>
      <div className={styles.heading}>{chatText.selectModel}</div>
      {models.map(model => <button key={model} type="button" role="menuitemradio" aria-checked={model === value} disabled={disabled}
        onClick={() => { onPick(model); close(); }}>
        <span>{formatModelLabel(model)}</span>{model === value && <Check size={18} aria-hidden="true" />}
      </button>)}
      {!models.length && <div className={styles.heading}>{chatText.noModels}</div>}
    </div>}
  </div>;
}
