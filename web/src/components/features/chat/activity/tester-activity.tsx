import { PetAvatar } from "@/components/features/pets/pet-avatar";
import { chatText } from "@/text/chat";
import { modelDisplayLabel } from "@/lib/model-display-label";
import { ChatActivity } from "./chat-activity";
import { StreamStatus } from "../stream_status/stream-status";
import { useChatStore, type LiveChecker } from "../chat-store";
import styles from "../chat-message.module.css";

/** Vietnamese wording of the live Tester cards (kept local: the shared chat text has no live-checker strings). */
const text = {
  title: "Đang kiểm tra",
  stopped: "Đã dừng",
  attempt: (attempt: number) => `lần ${attempt}`,
} as const;

/** One live check drawn exactly like a running chat message: pet + name + status, status line, collapsed Activity. */
function CheckerCard({ card }: { card: LiveChecker }) {
  const state = card.state === "running" ? "working" : card.state === "failed" ? "failed" : "idle";
  return <article className={styles.message} data-streaming={card.state === "running"} data-checker-id={card.id}>
    <div className={styles.identity}>
      {/* Tester is a Codex source: it wears the pet chosen for its model. */}
      <PetAvatar provider="codex_cli" model={card.model} state={state} />
      <strong>{card.provider_name || "Người kiểm tra"}</strong>
      <span className={styles.petStatus} data-state={state} role="status">
        {state !== "idle" ? <>· {chatText.petStatus[state]}</> : <>· {text.stopped}</>}
      </span>
    </div>
    {card.state === "running" && <StreamStatus>
      {card.title}{card.attempt > 0 && ` · ${text.attempt(card.attempt)}`}{card.model && ` · ${modelDisplayLabel(card.model, "codex_cli")}`}
    </StreamStatus>}
    <ChatActivity logs={card.progress} running={card.state === "running"} />
  </article>;
}

/** Live Tester checks shown beside the conversation, outside the collapsed Activity list of any message. */
export function TesterActivityView({ checkers }: { checkers: LiveChecker[] }) {
  if (!checkers.length) return null;
  return <section aria-label={text.title} aria-live="polite">
    {checkers.map(card => <CheckerCard key={card.id} card={card} />)}
  </section>;
}

export function TesterActivity() {
  return <TesterActivityView checkers={useChatStore(state => state.checkers)} />;
}
