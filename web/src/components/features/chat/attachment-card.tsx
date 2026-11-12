import { FileText, Folder, X } from "lucide-react";
import { chatText } from "@/text/chat";
import styles from "./attachment-card.module.css";

export function AttachmentCard({ name, remove, disabled = false }: { name: string; remove?: () => void; disabled?: boolean }) {
  const folder = name.match(/^(.*)\/ \(\d+ file\)$/);
  const label = folder ? folder[1] : name;
  const Icon = folder ? Folder : FileText;
  return <div className={styles.card} title={label}>
    <span className={styles.icon}><Icon size={30} strokeWidth={1.6} aria-hidden="true" /></span>
    <div className={styles.label}><strong>{label}</strong><span>{folder ? "Folder" : "File"}</span></div>
    {remove && <button className={styles.remove} type="button" disabled={disabled} aria-label={`${chatText.removeFile}: ${label}`} onClick={remove}><X size={14} aria-hidden="true" /></button>}
  </div>;
}
