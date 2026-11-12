import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { configApi } from "@/apis/config.api";
import { BUNDLED_PETS, DEFAULT_PETS, idleMs, petKind, petSheetUrl } from "@/lib/pet-choice";
import { Bot } from "lucide-react";
import { ClaudePet } from "./claude-pet";
import type { ChatPetState } from "@/lib/chat-pet-state";
import styles from "./pet-avatar.module.css";
/** Pet for the source that wrote the reply (the user's choice in Settings, else the default); `/api/pets/<id>` serves its sprite sheet. */
/** Claude kinds fall back to the app's own pet when Clawd is not installed. */
const CLAUDE = new Set(["claude_cli", "anthropic"]);
const CELL_HEIGHT = 208;
export function PetAvatar({ provider, state = "idle" }: { provider: string; state?: ChatPetState }) {
  const [failed, setFailed] = useState(false);
  const [viaBackend, setViaBackend] = useState(false);
  const [rows, setRows] = useState(9);
  const kind = petKind(provider);
  const chosen = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) }).data?.pets[kind];
  const id = chosen || DEFAULT_PETS[kind];
  const pet = id ? { id, idleMs: idleMs(id) } : undefined;
  const busy = state === "working" || state === "replying";
  const icon = id ? BUNDLED_PETS[id]?.icon : undefined;
  if (icon && !failed) return <img className={styles.icon} data-state={state} src={icon} alt="" aria-hidden="true" onError={() => setFailed(true)} />;
  if (!pet || failed) return CLAUDE.has(provider) ? <ClaudePet busy={busy} /> : <Bot size={18} aria-hidden="true" />;
  const url = viaBackend ? `/api/pets/${pet.id}` : petSheetUrl(pet.id);
  const style = { backgroundImage: `url(${url})`, "--rows": rows, "--idle": `${pet.idleMs}ms` } as React.CSSProperties;
  return <span className={styles.pet} data-state={state} style={style} aria-hidden="true">
    <img src={url} alt="" hidden onError={() => pet.id in BUNDLED_PETS && !viaBackend ? setViaBackend(true) : setFailed(true)}
      onLoad={event => setRows(Math.max(9, Math.round(event.currentTarget.naturalHeight / CELL_HEIGHT)))} /></span>;
}
