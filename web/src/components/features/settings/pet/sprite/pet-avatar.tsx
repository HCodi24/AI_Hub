import { useState } from "react";
import { PawPrint } from "lucide-react";
import type { PetState } from "@/lib/chat-pet-state";
import { usePetId } from "./use-pet-id";
import styles from "./pet-avatar.module.css";
const CELL_HEIGHT = 208;
/** Pet the user chose in Settings for the source that wrote the reply; no choice (or pets switched off) = nothing is drawn.
 *  `/api/pets/<id>` serves its sprite sheet: 8 columns, 6 rows, or 5 when it has no running row. `small` is for task rows. */
export function PetAvatar({ provider, model = "", state = "idle", small = false }: { provider: string; model?: string; state?: PetState; small?: boolean }) {
  const [failed, setFailed] = useState(false);
  const [rows, setRows] = useState(6);
  const id = usePetId(provider, model);
  if (!id || failed) return null;
  const url = `/api/pets/${id}`;
  const style = { backgroundImage: `url(${url})`, "--rows": rows, ...(small ? { "--w": "24px", "--h": "26px" } : {}) } as React.CSSProperties;
  return <span className={styles.pet} data-state={state} data-run={rows >= 6 ? "fox" : undefined} style={style} aria-hidden="true">
    <img src={url} alt="" hidden onError={() => setFailed(true)}
      onLoad={event => setRows(Math.max(5, Math.round(event.currentTarget.naturalHeight / CELL_HEIGHT)))} /></span>;
}

export function PetThumb({ id }: { id: string }) {
  const [failed, setFailed] = useState(false);
  const url = `/api/pets/${encodeURIComponent(id)}`;
  if (failed) return <PawPrint size={16} aria-hidden="true" />;
  return <span className={styles.thumb} style={{ backgroundImage: `url(${url})` }} aria-hidden="true">
    <img src={url} alt="" hidden onError={() => setFailed(true)} />
  </span>;
}
