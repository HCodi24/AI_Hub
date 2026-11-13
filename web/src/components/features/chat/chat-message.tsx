import { memo, useMemo, type ReactNode } from "react";
import { UserRound } from "lucide-react";
import { PetAvatar } from "@/components/features/pets/pet-avatar";
import type { ChatMessage as Message, StreamData } from "@/schema/chat.schema";
import { MarkdownView } from "@/components/markdown-view/markdown-view";
import { chatText } from "@/text/chat";
import { DebateCard } from "@/components/features/debate/debate-card";
import { PlanCard } from "@/components/features/plan/plan-card";
import { splitImportedFileText } from "@/lib/import-file";
import { AttachmentCard } from "./attachment-card";
import { ChatActivity } from "./chat-activity";
import { FileChanges } from "./file-changes";
import { chatPetState } from "@/lib/chat-pet-state";
import { parseChatQuestions } from "@/lib/chat-questions";
import styles from "./chat-message.module.css";
/** MutiS High: the app reads Codex's `mutis-delegate` blocks itself and shows the worker's result, so the raw JSON stays hidden. */
const hideDelegation = (text: string) => text.replace(/```mutis-delegate[\s\S]*?(?:```|$)/g, "").trim();
const PROVIDER_LABELS: Record<string, string> = { codex_cli: "Codex", claude_cli: "Claude", anthropic: "Claude", deepseek: "DeepSeek" };
export const ChatMessage = memo(function ChatMessage({ message, chatId, streaming = false, latest = false, status, replies }: { message: Message; chatId: string; streaming?: boolean; latest?: boolean; status?: StreamData; replies?: ReactNode }) {
  const user = message.role === "user";
  const petState = useMemo(() => chatPetState(message, streaming, latest), [message, streaming, latest]);
  const imported = useMemo(() => user ? splitImportedFileText(message.content) : {
    text: streaming
      ? hideDelegation(message.content).replace(/^```mutis-questions\s*$[\s\S]*$/m, "")
      : parseChatQuestions(hideDelegation(message.content)).text,
    files: [],
  }, [user, message.content, streaming]);
  /** DeepSeek token logs already live inside the collapsed activity disclosure, so they are not repeated here. */
  const tokenLine = message.provider === "deepseek" ? undefined : message.logs.filter((line) => /token|usage|chi phí/i.test(line)).at(-1);
  return <article className={styles.message} data-user={user} data-streaming={streaming}>
    <div className={styles.identity}>{user ? <UserRound size={18} aria-hidden="true" /> : <PetAvatar key={message.provider} provider={message.provider} state={petState} />}
      <strong>{user ? chatText.you : PROVIDER_LABELS[message.provider] || chatText.assistant}</strong>
      {!user && <span className={styles.petStatus} data-state={petState} role="status">{petState !== "idle" && <>· {chatText.petStatus[petState]}</>}</span>}
    </div>
    {message.debate_id ? <DebateCard id={message.debate_id} /> : message.plan_id ? <PlanCard id={message.plan_id} /> : <>
      {(!imported.files.length || imported.text) && <MarkdownView source={imported.text} />}
      {imported.files.length > 0 && <section className={styles.attachments} aria-label={chatText.attachedFiles}>
        {imported.files.map((file, index) => <AttachmentCard key={`${index}:${file.name}`} name={file.name} />)}
      </section>}
    </>}
    {replies}
    {message.attachments.map((attachment) => <a key={attachment.id} href={`/api/attachments/${encodeURIComponent(chatId)}/${encodeURIComponent(`${attachment.id}.${attachment.mime === "image/jpeg" ? "jpg" : attachment.mime.split("/")[1]}`)}`} target="_blank" rel="noreferrer"><img className={styles.thumb} alt={chatText.attachments} src={`/api/attachments/${encodeURIComponent(chatId)}/${encodeURIComponent(`${attachment.id}.${attachment.mime === "image/jpeg" ? "jpg" : attachment.mime.split("/")[1]}`)}`} /></a>)}
    {message.reasoning && <details><summary>{chatText.reasoning}</summary><MarkdownView source={message.reasoning} /></details>}
    {!user && <ChatActivity logs={message.logs} provider={message.provider} />}
    {!user && streaming && <div className={styles.streamStatus} role="status" aria-live="polite" aria-atomic="true">
      {chatText.streaming} {status?.provider_name} {status?.model}
      {status?.elapsed_s !== undefined && ` · ${status.elapsed_s} ${chatText.seconds}`}
      {status?.idle_s !== undefined && ` · ${chatText.idle}: ${status.idle_s} ${chatText.seconds}`}
    </div>}
    {tokenLine && <div className={styles.tokens}>{tokenLine}</div>}
    <FileChanges files={message.files} diff={message.diff} />
    {message.error && <div className={styles.error} role="alert">{message.error}</div>}
  </article>;
});
