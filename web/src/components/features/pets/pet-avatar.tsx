import { useState } from "react";
import { Bot } from "lucide-react";
import { ClaudePet } from "./claude-pet";
import styles from "./pet-avatar.module.css";
/**
 * Provider kind → Codex-format pet, served by `/api/pets/<id>`: built-in Codex pets come from the Codex pet CDN,
 * others must be installed into Codex by the user (`~/.codex/pets/<id>`). Neither is bundled with the app.
 * `idleMs` is the pet's full idle loop; both pets use the same frame proportions.
 */
const PETS: Record<string, { id: string; idleMs: number }> = {
  codex_cli: { id: "codex", idleMs: 6600 },
  claude_cli: { id: "clawd-svg", idleMs: 1100 },
  anthropic: { id: "clawd-svg", idleMs: 1100 },
};
/** Claude kinds fall back to the app's own pet when Clawd is not installed. */
const CLAUDE = new Set(["claude_cli", "anthropic"]);
const CELL_HEIGHT = 208;
export function PetAvatar({ provider, busy }: { provider: string; busy: boolean }) {
  const [failed, setFailed] = useState(false);
  const [rows, setRows] = useState(9);
  const pet = PETS[provider];
  if (!pet || failed) return CLAUDE.has(provider) ? <ClaudePet busy={busy} /> : <Bot size={18} aria-hidden="true" />;
  const url = `/api/pets/${pet.id}`;
  const style = { backgroundImage: `url(${url})`, "--rows": rows, "--idle": `${pet.idleMs}ms` } as React.CSSProperties;
  return <span className={styles.pet} data-busy={busy} style={style} aria-hidden="true">
    <img src={url} alt="" hidden onError={() => setFailed(true)}
      onLoad={event => setRows(Math.max(9, Math.round(event.currentTarget.naturalHeight / CELL_HEIGHT)))} /></span>;
}
