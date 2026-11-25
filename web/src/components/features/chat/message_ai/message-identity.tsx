import { UserRound } from "lucide-react";
import { PetAvatar } from "@/components/features/pets/pet-avatar";
import type { ChatPetState } from "@/lib/chat-pet-state";
import { modelDisplayLabel } from "@/lib/model-display-label";
import { chatText } from "@/text/chat";
import styles from "../chat-message.module.css";

const PROVIDER_LABELS: Record<string, string> = { codex_cli: "Codex", claude_cli: "Claude", antigravity_cli: "Antigravity", anthropic: "Claude", deepseek: "DeepSeek", deepseek_agent: "DeepSeek", openai_compat: "OpenAI" };

type Props = {
  user: boolean;
  provider: string;
  model: string;
  petState: ChatPetState;
};

export function MessageIdentity({ user, provider, model, petState }: Props) {
  const name = user
    ? chatText.you
    : model ? modelDisplayLabel(model, provider) : PROVIDER_LABELS[provider] || chatText.assistant;
  return <div className={styles.identity}>{user ? <UserRound size={18} aria-hidden="true" /> : <PetAvatar key={`${provider}:${model}`} provider={provider} model={model} state={petState} />}
    <strong>{name}</strong>
    {!user && <span className={styles.petStatus} data-state={petState} role="status">{petState !== "idle" && <>· {chatText.petStatus[petState]}</>}</span>}
  </div>;
}
