import { memo, useMemo, type ReactNode } from "react";
import { ChevronRight, MousePointerClick, UserRound } from "lucide-react";
import { PetAvatar } from "@/components/features/pets/pet-avatar";
import type { ChatMessage as Message, StreamData } from "@/schema/chat.schema";
import { MarkdownView } from "@/components/markdown-view/markdown-view";
import { chatText } from "@/text/chat";
import { DebateCard } from "@/components/features/debate/debate-card";
import { splitImportedFileText } from "@/lib/import-file";
import { splitPreviewSelectionText } from "@/lib/preview-selection";
import { stripDevChatPreamble } from "@/lib/dev-chat-preamble";
import { AttachmentCard } from "./attachment-card";
import { ChatActivity } from "./chat-activity";
import { FileChanges } from "./file-changes";
import { TaskBoardView } from "./task-board";
import { chatPetState } from "@/lib/chat-pet-state";
import { parseChatQuestions } from "@/lib/chat-questions";
import { modelDisplayLabel } from "@/lib/model-display-label";
import styles from "./chat-message.module.css";
/** MutiS: the app reads Codex's `mutis-delegate` blocks itself and shows the worker's result, so the raw JSON stays hidden. */
const hideDelegation = (text: string) => text.replace(/```mutis-delegate[\s\S]*?(?:```|$)/g, "").trim();
const mutisRoleLabels = new Map([
  ["lead", "Trưởng nhóm"],
  ["worker", "Thực thi"],
  ["checker", "Kiểm tra"],
]);
const PROVIDER_LABELS: Record<string, string> = { codex_cli: "Codex", claude_cli: "Claude", antigravity_cli: "Antigravity", anthropic: "Claude", deepseek: "DeepSeek" };
export const ChatMessage = memo(function ChatMessage({ message, chatId, streaming = false, latest = false, status, replies, taskBoardEnabled = true }: { message: Message; chatId: string; streaming?: boolean; latest?: boolean; status?: StreamData; replies?: ReactNode; taskBoardEnabled?: boolean }) {
  const user = message.role === "user";
  /** Model identity comes only from this message: one chat can mix worker turns with different models. */
  const sourceLabel = PROVIDER_LABELS[message.provider] || chatText.assistant;
  const model = message.model.trim();
  const petState = useMemo(() => chatPetState(message, streaming, latest), [message, streaming, latest]);
  const imported = useMemo(() => {
    if (user) {
      const split = splitImportedFileText(message.content);
      return { ...split, ...splitPreviewSelectionText(stripDevChatPreamble(split.text)) };
    }
    return {
      text: streaming
        ? hideDelegation(message.content).replace(/^```mutis-questions\s*$[\s\S]*$/m, "")
        : parseChatQuestions(hideDelegation(message.content)).text,
      files: [],
      selections: [],
    };
  }, [user, message.content, streaming]);
  /** DeepSeek token logs already live inside the collapsed activity disclosure, so they are not repeated here. */
  const tokenLine = message.provider === "deepseek" ? undefined : message.logs.filter((line) => /token|usage|chi phí/i.test(line)).at(-1);
  return <article className={styles.message} data-user={user} data-streaming={streaming}>
    <div className={styles.identity}>{user ? <UserRound size={18} aria-hidden="true" /> : <PetAvatar key={`${message.provider}:${model}`} provider={message.provider} model={model} state={petState} />}
      <strong>{user ? chatText.you : model ? modelDisplayLabel(model, message.provider) : sourceLabel}</strong>
      {!user && <span className={styles.petStatus} data-state={petState} role="status">{petState !== "idle" && <>· {chatText.petStatus[petState]}</>}</span>}
    </div>
    {message.debate_id ? <DebateCard id={message.debate_id} /> : <>
      {imported.text && <MarkdownView source={imported.text} />}
      {imported.selections.length > 0 && <section className={styles.selections} aria-label="Phần tử đã chọn">
        {imported.selections.map((item, index) => <details key={index} className={styles.selection}>
          <summary title={item.label}><MousePointerClick size={16} aria-hidden="true" /><span>{item.label.split(" > ").pop()}</span><ChevronRight className={styles.selectionArrow} size={16} aria-hidden="true" /></summary>
          <div className={styles.selectionContent}><p>{item.label}</p><MarkdownView source={item.text} /></div>
        </details>)}
      </section>}
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
      {status?.role && mutisRoleLabels.has(status.role) && ` · ${mutisRoleLabels.get(status.role)}`}
      {status?.elapsed_s !== undefined && ` · ${status.elapsed_s} ${chatText.seconds}`}
      {status?.idle_s !== undefined && ` · ${chatText.idle}: ${status.idle_s} ${chatText.seconds}`}
    </div>}
    {tokenLine && <div className={styles.tokens}>{tokenLine}</div>}
    <div className={styles.fileResults}><FileChanges files={message.files} diff={message.diff} /></div>
    {taskBoardEnabled && !(streaming && latest) && message.tasks && <div className={styles.archivedTaskBoard}><TaskBoardView tasks={message.tasks} /></div>}
    {message.error && <div className={styles.error} role="alert">{message.error}</div>}
  </article>;
});
