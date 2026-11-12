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
import { chatPetState } from "@/lib/chat-pet-state";
import { parseChatQuestions } from "@/lib/chat-questions";
import styles from "./chat-message.module.css";
const PROVIDER_LABELS: Record<string, string> = { codex_cli: "Codex", claude_cli: "Claude", anthropic: "Claude", deepseek: "DeepSeek" };
export const ChatMessage = memo(function ChatMessage({ message, chatId, streaming = false, latest = false, status, replies }: { message: Message; chatId: string; streaming?: boolean; latest?: boolean; status?: StreamData; replies?: ReactNode }) {
  const user = message.role === "user";
  const petState = useMemo(() => chatPetState(message, streaming, latest), [message, streaming, latest]);
  const imported = useMemo(() => user ? splitImportedFileText(message.content) : {
    text: streaming
      ? message.content.replace(/^```mutis-questions\s*$[\s\S]*$/m, "")
      : parseChatQuestions(message.content).text,
    files: [],
  }, [user, message.content, streaming]);
  const tokenLine = message.logs.filter((line) => /token|usage|chi phí/i.test(line)).at(-1);
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
    {!user && streaming && <div className={styles.streamStatus} role="status" aria-live="polite" aria-atomic="true">
      {chatText.streaming} {status?.provider_name} {status?.model}
      {status?.elapsed_s !== undefined && ` · ${status.elapsed_s} ${chatText.seconds}`}
      {status?.idle_s !== undefined && ` · ${chatText.idle}: ${status.idle_s} ${chatText.seconds}`}
    </div>}
    {replies}
    {message.attachments.map((attachment) => <a key={attachment.id} href={`/api/attachments/${encodeURIComponent(chatId)}/${encodeURIComponent(`${attachment.id}.${attachment.mime === "image/jpeg" ? "jpg" : attachment.mime.split("/")[1]}`)}`} target="_blank" rel="noreferrer"><img className={styles.thumb} alt={chatText.attachments} src={`/api/attachments/${encodeURIComponent(chatId)}/${encodeURIComponent(`${attachment.id}.${attachment.mime === "image/jpeg" ? "jpg" : attachment.mime.split("/")[1]}`)}`} /></a>)}
    {message.reasoning && <details><summary>{chatText.reasoning}</summary><MarkdownView source={message.reasoning} /></details>}
    {message.logs.length > 0 && <details><summary>{chatText.activity}</summary><pre className={styles.logs}>{message.logs.join("\n")}</pre></details>}
    {tokenLine && <div className={styles.tokens}>{tokenLine}</div>}
    {message.files.length > 0 && <section className={styles.files} aria-label={chatText.diff}>
      <header><strong>{chatText.edited(message.files.length)}</strong>
        <span className={styles.add}>+{message.files.reduce((sum, file) => sum + file.added, 0)}</span>{" "}
        <span className={styles.remove}>−{message.files.reduce((sum, file) => sum + file.removed, 0)}</span></header>
      <ul>{message.files.map((file) => <li key={file.path}><span className={styles.fileName}>
        <span className={styles.fileStatus} data-status={file.status || "modified"} role="img" aria-label={chatText.fileStatus[file.status || "modified"]} title={chatText.fileStatus[file.status || "modified"]}>{file.status === "added" ? "+" : file.status === "deleted" ? "\u2212" : "\u2022"}</span><span className={styles.path}>{file.path}</span></span>
        <span><span className={styles.add}>+{file.added}</span> <span className={styles.remove}>−{file.removed}</span></span></li>)}</ul>
    </section>}
    {message.diff && <details open><summary>{chatText.diff}</summary><pre className={styles.diff}>{message.diff.split("\n").map((line, index) =>
      <span key={index} className={line.startsWith("+") ? styles.add : line.startsWith("-") ? styles.remove : undefined}>{line}{"\n"}</span>)}</pre></details>}
    {message.error && <div className={styles.error} role="alert">{message.error}</div>}
  </article>;
});
