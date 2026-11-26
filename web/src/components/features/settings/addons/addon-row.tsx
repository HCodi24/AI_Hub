import type { ReactNode } from "react";
import { Puzzle, Server, Sparkles } from "lucide-react";
import type { AddonKind } from "@/schema/addons.schema";
import styles from "../settings.module.css";
const ICONS = { plugin: Puzzle, skill: Sparkles, mcp: Server } as const;
/** "name@marketplace" -> the name, with the marketplace as a quiet tag beside it. */
export function splitId(id: string) {
  const at = id.lastIndexOf("@");
  return at > 0 ? { name: id.slice(0, at), tag: id.slice(at + 1) } : { name: id, tag: "" };
}
type AddonRowProps = { kind: AddonKind; name: string; tag?: string; detail?: string; muted?: boolean; children?: ReactNode };
/** One row of the add-on card: icon tile, name with tag, a one-line detail, then the row's controls. */
export function AddonRow({ kind, name, tag, detail, muted, children }: AddonRowProps) {
  const Icon = ICONS[kind];
  return <li className={styles.addonRow} data-muted={muted || undefined}>
    <span className={styles.addonIcon} aria-hidden="true"><Icon size={16} /></span>
    <span className={styles.addonText}>
      <span className={styles.addonName}><strong>{name}</strong>{tag && <span>{tag}</span>}</span>
      {detail && <small title={detail}>{detail}</small>}
    </span>
    {children && <span className={styles.addonControls}>{children}</span>}
  </li>;
}
