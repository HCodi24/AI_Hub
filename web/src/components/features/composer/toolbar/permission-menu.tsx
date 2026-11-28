import type { RefObject } from "react";
import { Check, Hand, ShieldCheck, ShieldAlert, AlertTriangle } from "lucide-react";
import { PERMISSIONS } from "@/constants/chat";
import { chatText } from "@/text/chat";
import styles from "./permission-menu.module.css";

type Permission = (typeof PERMISSIONS)[number];
const ICONS = { read: Hand, write: ShieldCheck, full: ShieldAlert };
type Props = {
  value: Permission; disabled: boolean; cli: boolean;
  menuRef: RefObject<HTMLDetailsElement | null>; onOpen: () => void; onPick: (permission: Permission) => void;
  /** A caveat about the current source: `chatOnly` replaces the chip and locks the choices; otherwise it is only a warning. */
  limit?: { note: string; chatOnly?: boolean };
  /** Extra heading/scope description shown when the chat permission only controls a subset of roles (e.g. MutiS Lead). */
  scope?: { heading: string; note: string };
};

export function PermissionMenu({ value, disabled, cli, menuRef, onOpen, onPick, limit, scope }: Props) {
  const hasWarning = Boolean(limit && !limit.chatOnly);
  const Icon = hasWarning ? AlertTriangle : ICONS[value];
  return <details ref={menuRef} data-glass="permission-picker" className={styles.picker} onToggle={event => {
    if (event.target === event.currentTarget && event.currentTarget.open) onOpen();
  }}>
    <summary aria-label={chatText.permission} title={limit?.note ?? scope?.note ?? chatText.permission} data-full={value === "full"} data-permission={limit ? undefined : value} data-warning={hasWarning || undefined}>
      <Icon size={16} aria-hidden="true" /><span>{limit?.chatOnly ? chatText.chatOnly : chatText[value]}</span>
    </summary>
    <div data-glass="overlay" className={styles.panel} role="group" aria-label={chatText.permission}>
      {hasWarning && limit && <div className={styles.warning} role="note">
        <AlertTriangle size={14} aria-hidden="true" />
        <span>{limit.note}</span>
      </div>}
      {limit?.chatOnly
        ? <p>{limit.note}</p>
        : <>
          <p className={styles.heading}>{scope?.heading ?? chatText.permissionHeading}</p>
          {scope?.note && <p className={styles.scope}>{scope.note}</p>}
        </>}
      {PERMISSIONS.map(permission => {
        const OptionIcon = ICONS[permission];
        return <button key={permission} type="button" aria-pressed={value === permission} data-full={permission === "full"}
          data-permission={permission}
          disabled={disabled || Boolean(limit?.chatOnly) || (permission === "full" && !cli)} onClick={() => {
            onPick(permission);
            if (menuRef.current) { menuRef.current.open = false; menuRef.current.querySelector("summary")?.focus(); }
          }}>
          <OptionIcon size={16} aria-hidden="true" />
          <span><strong>{chatText[permission]}</strong><small>{chatText.permissionDescriptions[permission]}</small></span>
          {value === permission && <Check size={16} aria-hidden="true" />}
        </button>;
      })}
    </div>
  </details>;
}
