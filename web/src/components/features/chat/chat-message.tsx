import { memo } from "react";
import { Bot, UserRound } from "lucide-react";
import type { ChatMessage as Message } from "@/schema/chat.schema";
import { MarkdownView } from "@/components/markdown-view/markdown-view";
import { chatText } from "@/text/chat";
import { DebateCard } from "@/components/features/debate/debate-card";
import { PlanCard } from "@/components/features/plan/plan-card";
import styles from "./chat-message.module.css";
export const ChatMessage = memo(function ChatMessage({ message, chatId, streaming = false }: { message: Message; chatId: string; streaming?: boolean }) {
  const user = message.role === "user";
  const tokenLine = message.logs.filter((line) => /token|usage|chi phí/i.test(line)).at(-1);
  return <article className={styles.message} data-user={user} data-streaming={streaming}>
    <div className={styles.identity}>{user ? <UserRound size={18} aria-hidden="true" /> : <Bot size={18} aria-hidden="true" />}
      <strong>{user ? chatText.you : chatText.assistant}</strong></div>
    {message.debate_id ? <DebateCard id={message.debate_id} /> : message.plan_id ? <PlanCard id={message.plan_id} /> : <MarkdownView source={message.content} />}
    {message.attachments.map((attachment) => <a key={attachment.id} href={`/api/attachments/${encodeURIComponent(chatId)}/${encodeURIComponent(`${attachment.id}.${attachment.mime === "image/jpeg" ? "jpg" : attachment.mime.split("/")[1]}`)}`} target="_blank" rel="noreferrer">{chatText.attachments}</a>)}
    {message.reasoning && <details><summary>{chatText.reasoning}</summary><MarkdownView source={message.reasoning} /></details>}
    {message.logs.length > 0 && <details><summary>{chatText.activity}</summary><pre className={styles.logs}>{message.logs.join("\n")}</pre></details>}
    {tokenLine && <div className={styles.tokens}>{tokenLine}</div>}
    {message.diff && <details open><summary>{chatText.diff}</summary><pre className={styles.diff}>{message.diff.split("\n").map((line, index) =>
      <span key={index} className={line.startsWith("+") ? styles.add : line.startsWith("-") ? styles.remove : undefined}>{line}{"\n"}</span>)}</pre></details>}
    {message.error && <div className={styles.error} role="alert">{message.error}</div>}
  </article>;
});
