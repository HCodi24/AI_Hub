import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { configApi } from "@/apis/config.api";
import { petKind } from "@/lib/pet-choice";
import type { ChatPetState } from "@/lib/chat-pet-state";
import styles from "./pet-avatar.module.css";
/** Pet the user chose in Settings for the source that wrote the reply; no choice (or pets switched off) = nothing is drawn. `/api/pets/<id>` serves its 8 x 5 sprite sheet. */
export function PetAvatar({ provider, model = "", state = "idle" }: { provider: string; model?: string; state?: ChatPetState }) {
  const [failed, setFailed] = useState(false);
  const config = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) }).data;
  const id = config?.pets[petKind(provider, model)];
  if (!id || failed || config?.pets_enabled === false) return null;
  const url = `/api/pets/${id}`;
  return <span className={styles.pet} data-state={state} style={{ backgroundImage: `url(${url})` }} aria-hidden="true">
    <img src={url} alt="" hidden onError={() => setFailed(true)} /></span>;
}
